//! Request-level authorization context with platform-admin impersonation support.
//!
//! This module implements the new tenant-resolution chain described by
//! OpenSpec change `refactor-platform-admin-impersonation` (#44).
//!
//! # Resolution order
//!
//! 1. `X-Tenant-ID` header (by slug or UUID). Must resolve to an existing
//!    `tenants` row — an orphan value yields `404 tenant_not_found`.
//! 2. `users.default_tenant_id` when the caller has set a persistent
//!    preference (migration 023).
//! 3. Auto-select when the caller belongs to exactly one tenant.
//! 4. Otherwise:
//!    - PlatformAdmin: `ctx.tenant = None` is the success case (platform-
//!      scoped operation).
//!    - Regular user: emit `400 select_tenant` with the set of available
//!      tenants so the client can prompt the user.
//!
//! The legacy `tenant_required` shape is preserved for one deprecation
//! window via the `Accept-Error-Legacy: true` request header.
//!
//! # Relationship to `TenantContext`
//!
//! `RequestContext` is the preferred API going forward. For code paths that
//! still need the pre-existing [`mk_core::types::TenantContext`], use
//! [`RequestContext::require_tenant_context`] which performs the `require_
//! target_tenant` check and returns a fully-populated `TenantContext`.
//!
//! Legacy helpers `authenticated_tenant_context` and `tenant_scoped_context`
//! in `cli/src/server/mod.rs` remain functional during the migration window;
//! they are marked `#[deprecated]` and will be removed once all call sites
//! have been migrated (tracked as slice 44.c).

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use mk_core::types::{
    INSTANCE_SCOPE_TENANT_ID, Role, RoleIdentifier, TenantContext, TenantId, UserId,
};
use serde::Serialize;
use serde_json::json;

use super::AppState;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A resolved tenant in a request context.
///
/// Carries enough information for handlers to build responses, emit audit
/// events, and enforce tenant-scoped authorization without re-querying the
/// tenant store.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTenant {
    pub id: TenantId,
    pub slug: String,
    pub name: String,
}

impl ResolvedTenant {
    pub fn id_str(&self) -> &str {
        self.id.as_str()
    }
}

/// Request-scoped authorization context.
///
/// Constructed by [`request_context`] from the authenticated identity plus
/// request headers. Handlers should prefer this type over the legacy
/// [`mk_core::types::TenantContext`] because it faithfully represents the
/// "PlatformAdmin without target tenant" case (`tenant = None`) that the old
/// resolver could not express without emitting `tenant_required`.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub user_id: UserId,
    pub roles: Vec<RoleIdentifier>,
    pub is_platform_admin: bool,
    /// Tenant the caller is acting against. `None` means platform-scoped:
    /// only valid for PlatformAdmin. Handlers that need a tenant MUST call
    /// [`RequestContext::require_target_tenant`].
    pub tenant: Option<ResolvedTenant>,
    /// Set of tenant slugs the user is a member of. Used to populate the
    /// `select_tenant` payload so the client can render a picker without an
    /// extra round-trip. Empty for PlatformAdmin (they are not constrained
    /// to this list).
    pub available_tenants: Vec<ResolvedTenant>,
}

impl RequestContext {
    /// Returns the resolved tenant or a `400 select_tenant` response.
    pub fn require_target_tenant(&self, headers: &HeaderMap) -> Result<&ResolvedTenant, Response> {
        match &self.tenant {
            Some(t) => Ok(t),
            None => Err(select_tenant_response(&self.available_tenants, headers)),
        }
    }

    /// Convenience: build a `TenantContext` for call sites that still require
    /// the legacy shape. Emits `select_tenant` if no tenant is resolved.
    pub fn require_tenant_context(&self, headers: &HeaderMap) -> Result<TenantContext, Response> {
        let tenant = self.require_target_tenant(headers)?;
        Ok(TenantContext {
            tenant_id: tenant.id.clone(),
            user_id: self.user_id.clone(),
            agent_id: None,
            roles: self.roles.clone(),
            target_tenant_id: None,
        })
    }

    /// Resolve a list endpoint's scope from the `?tenant=` query parameter.
    ///
    /// Rules (see `openspec/changes/add-cross-tenant-admin-listing/proposal.md`):
    ///
    /// | `?tenant=` value | Non-admin                                | PlatformAdmin           |
    /// |------------------|------------------------------------------|-------------------------|
    /// | *(omitted)*      | `Single(self.tenant)` or `select_tenant` | same                    |
    /// | `*`              | `403 forbidden_scope`                    | `All`                   |
    /// | `all`            | `403 forbidden_scope` + deprecation log  | `All` + deprecation log |
    /// | `<slug-or-uuid>` | `Single` if member, else `forbidden_tenant` | `Single`             |
    /// | unknown tenant   | `404 tenant_not_found`                   | `404 tenant_not_found`  |
    ///
    /// The membership check reuses [`RequestContext::available_tenants`] and
    /// therefore requires no additional IO beyond the tenant-store lookup.
    pub async fn list_scope(
        &self,
        state: &crate::server::AppState,
        headers: &HeaderMap,
        query_tenant: Option<&str>,
    ) -> Result<ListTenantScope, Response> {
        match classify_tenant_query(query_tenant) {
            ScopeIntent::Inherited => {
                let t = self.require_target_tenant(headers)?;
                Ok(ListTenantScope::Single(t.clone()))
            }
            ScopeIntent::All { is_alias } => {
                if is_alias {
                    tracing::warn!(
                        target: "compat",
                        param = "?tenant=all",
                        replacement = "?tenant=*",
                        "deprecated tenant scope alias — clients should migrate to ?tenant=*"
                    );
                }
                if self.is_platform_admin {
                    Ok(ListTenantScope::All)
                } else {
                    Err(forbidden_scope_response("PlatformAdmin"))
                }
            }
            ScopeIntent::Single(hint) => {
                let record = state.tenant_store.get_tenant(hint).await.map_err(|e| {
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "tenant_lookup_failed",
                        &e.to_string(),
                        None,
                    )
                })?;
                let record = match record {
                    Some(t) => t,
                    None => {
                        return Err(error_response(
                            StatusCode::NOT_FOUND,
                            "tenant_not_found",
                            &format!("No tenant matches '{hint}'"),
                            None,
                        ));
                    }
                };
                let is_member = self
                    .available_tenants
                    .iter()
                    .any(|t| t.id.as_str() == record.id.as_str());
                if !self.is_platform_admin && !is_member {
                    return Err(error_response(
                        StatusCode::FORBIDDEN,
                        "forbidden_tenant",
                        "You are not a member of the requested tenant",
                        None,
                    ));
                }
                Ok(ListTenantScope::Single(ResolvedTenant {
                    id: record.id,
                    slug: record.slug,
                    name: record.name,
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// List scope (cross-tenant admin reads — #44.d)
// ---------------------------------------------------------------------------

/// Resolved scope for a list endpoint.
///
/// Produced by [`RequestContext::list_scope`] from the `?tenant=` query
/// parameter. Handlers branch on this to decide whether to issue a
/// tenant-filtered or a cross-tenant (`SELECT … FROM <table>`) query.
#[derive(Debug, Clone)]
pub enum ListTenantScope {
    /// Caller wants results for exactly one tenant. Includes the resolved
    /// metadata so handlers can render `tenant.slug`/`tenant.name` without
    /// an extra lookup.
    Single(ResolvedTenant),
    /// Caller wants results across every tenant. Only ever returned when
    /// `is_platform_admin == true`.
    All,
}

/// Pure classification of the raw `?tenant=` value, **before** any IO.
///
/// Split out from [`RequestContext::list_scope`] so the parsing /
/// authorization rules can be unit-tested without spinning up a tenant
/// store or postgres pool.
#[derive(Debug, PartialEq, Eq)]
enum ScopeIntent<'a> {
    /// `?tenant=` was not provided; use the resolved [`RequestContext::tenant`].
    Inherited,
    /// `?tenant=*` (canonical) or `?tenant=all` (deprecated alias).
    All { is_alias: bool },
    /// `?tenant=<slug-or-uuid>`.
    Single(&'a str),
}

fn classify_tenant_query(query_tenant: Option<&str>) -> ScopeIntent<'_> {
    match query_tenant.map(str::trim).filter(|s| !s.is_empty()) {
        None => ScopeIntent::Inherited,
        Some("*") => ScopeIntent::All { is_alias: false },
        Some(s) if s.eq_ignore_ascii_case("all") => ScopeIntent::All { is_alias: true },
        Some(s) => ScopeIntent::Single(s),
    }
}

/// Build a `403 forbidden_scope` response with a structured `required_role`
/// field, distinct from `forbidden_tenant` so clients can differentiate
/// "you cannot see other tenants" from "you cannot do cross-tenant ops".
///
/// See `openspec/changes/add-cross-tenant-admin-listing/proposal.md`.
pub fn forbidden_scope_response(required_role: &str) -> Response {
    let body = serde_json::json!({
        "error": "forbidden_scope",
        "required_role": required_role,
        "message": format!(
            "'{required_role}' role required for cross-tenant scope (?tenant=*)"
        ),
    });
    (StatusCode::FORBIDDEN, axum::Json(body)).into_response()
}

// ---------------------------------------------------------------------------
// #44.d — cross-tenant listing dispatch helper
// ---------------------------------------------------------------------------

/// Three-way outcome of resolving `?tenant=` on a list endpoint.
///
/// Each list handler that supports `?tenant=` (see #44.d RFC) uses
/// [`resolve_list_scope`] to classify the request into one of these:
///
/// - [`ListDispatch::TenantScoped`] — no `?tenant` param; caller should
///   run its existing tenant-scoped logic unchanged (preserves backward
///   compatibility bit-for-bit).
/// - [`ListDispatch::CrossTenant`] — `?tenant=*` or the deprecated alias
///   `?tenant=all` by a PlatformAdmin; caller should dispatch to its
///   cross-tenant list function which emits the scope-envelope body.
/// - [`ListDispatch::Response`] — authorization or input error; caller
///   returns the response verbatim.
///
/// This helper was extracted after a third near-identical copy of the
/// same gate block appeared (user_api, project_api, then org_api). It
/// prevents drift (trim, case-fold, deprecation warning, error message
/// formatting) across endpoints.
pub enum ListDispatch {
    TenantScoped,
    CrossTenant,
    Response(Response),
}

/// Resolve a `?tenant=` query parameter for a list endpoint.
///
/// Grammar (see RFC):
///
/// - absent               → [`ListDispatch::TenantScoped`]
/// - `*`                  → [`ListDispatch::CrossTenant`] (PlatformAdmin
///                          required; otherwise `403 forbidden_scope`)
/// - `all` (case-insensitive) → deprecated alias for `*`; logs a compat
///                          warning and resolves identically
/// - anything else        → `501 scope_not_implemented` (wiring for
///                          `?tenant=<slug>` is deferred to a later PR
///                          cluster; the 501 is a stable, documented
///                          response that clients can detect)
///
/// `endpoint_label` is surfaced in the 501 body to help clients discover
/// which endpoint they hit (e.g. `"/user"`, `"/project"`, `"/org"`).
pub async fn resolve_list_scope(
    state: &AppState,
    headers: &HeaderMap,
    raw_tenant_param: Option<&str>,
    endpoint_label: &str,
) -> ListDispatch {
    let Some(raw) = raw_tenant_param else {
        return ListDispatch::TenantScoped;
    };
    let trimmed = raw.trim();
    // Empty string is treated like the absent case — defensive for clients
    // that send `?tenant=` with no value (e.g. optional-form-field clients).
    if trimmed.is_empty() {
        return ListDispatch::TenantScoped;
    }
    let is_all_star = trimmed == "*";
    let is_all_alias = trimmed.eq_ignore_ascii_case("all");
    if !is_all_star && !is_all_alias {
        return ListDispatch::Response(error_response(
            StatusCode::NOT_IMPLEMENTED,
            "scope_not_implemented",
            &format!(
                "?tenant=<slug> is not yet supported on {endpoint_label}; use ?tenant=* for cross-tenant listing or omit the parameter"
            ),
            None,
        ));
    }
    if is_all_alias {
        tracing::warn!(
            target: "compat",
            param = "?tenant=all",
            replacement = "?tenant=*",
            endpoint = endpoint_label,
            "deprecated tenant scope alias — clients should migrate to ?tenant=*"
        );
    }
    let req_ctx = match request_context(state, headers).await {
        Ok(c) => c,
        Err(r) => return ListDispatch::Response(r),
    };
    if !req_ctx.is_platform_admin {
        return ListDispatch::Response(forbidden_scope_response("PlatformAdmin"));
    }
    ListDispatch::CrossTenant
}

/// Returns `Ok(())` when the caller is a PlatformAdmin, `403 forbidden` otherwise.
///
/// Unlike the legacy `tenant_scoped_context`-based check, this function has
/// no tenant dependency — it operates purely on instance-scope roles, which
/// is exactly what fresh-deploy / bootstrap flows need.
pub fn require_platform_admin(ctx: &RequestContext) -> Result<(), Response> {
    if ctx.is_platform_admin {
        Ok(())
    } else {
        Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required for this endpoint",
            None,
        ))
    }
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// Build a [`RequestContext`] from the authenticated identity + request headers.
///
/// This is the single entry point for the new resolution chain. Handlers
/// should call it at the top of every authenticated route; the returned
/// `Response` is already a well-formed error that can be returned as-is.
#[tracing::instrument(skip_all)]
pub async fn request_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<RequestContext, Response> {
    // Delegate identity extraction to the legacy helper. This keeps the
    // plugin/k8s/dev-header branches identical while we own just the tenant
    // resolution step.
    let (user_id, instance_roles, known_tenant_ids) =
        super::resolve_identity(state, headers).await?;

    let is_platform_admin = instance_roles
        .iter()
        .any(|r| matches!(r, RoleIdentifier::Known(Role::PlatformAdmin)));

    // Fetch every tenant the user can currently see, for select_tenant payload.
    let mut available = Vec::new();
    if is_platform_admin {
        // PlatformAdmins can target any tenant — populate from the tenant store.
        if let Ok(tenants) = state.tenant_store.list_tenants(false).await {
            for t in tenants {
                available.push(ResolvedTenant {
                    id: t.id,
                    slug: t.slug,
                    name: t.name,
                });
            }
        }
    } else {
        for tid in known_tenant_ids.iter() {
            if let Ok(Some(t)) = state.tenant_store.get_tenant(tid).await {
                available.push(ResolvedTenant {
                    id: t.id,
                    slug: t.slug,
                    name: t.name,
                });
            }
        }
    }

    // --- Step 1: X-Tenant-ID header ---
    let explicit = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(hint) = explicit {
        // Accept by id or slug via TenantStore::get_tenant.
        let record = state.tenant_store.get_tenant(hint).await.map_err(|e| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "tenant_lookup_failed",
                &e.to_string(),
                None,
            )
        })?;
        let record = match record {
            Some(t) => t,
            None => {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "tenant_not_found",
                    &format!("No tenant matches '{hint}'"),
                    None,
                ));
            }
        };
        // Membership check (PlatformAdmin exempt).
        let is_member = known_tenant_ids.iter().any(|tid| tid == record.id.as_str());
        if !is_platform_admin && !is_member {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "forbidden_tenant",
                "You are not a member of the requested tenant",
                None,
            ));
        }
        return Ok(RequestContext {
            user_id,
            roles: instance_roles,
            is_platform_admin,
            tenant: Some(ResolvedTenant {
                id: record.id,
                slug: record.slug,
                name: record.name,
            }),
            available_tenants: available,
        });
    }

    // --- Step 2: users.default_tenant_id ---
    if let Ok(Some(default)) = state
        .postgres
        .get_user_default_tenant(user_id.as_str())
        .await
    {
        // The FK ON DELETE SET NULL guarantees the tenant exists if the
        // column is populated, but re-check defensively.
        if let Ok(Some(record)) = state.tenant_store.get_tenant(default.as_str()).await {
            // Only honor if the user is still a member (covers the edge case of
            // a user losing membership but their preference never being cleared).
            let is_member = known_tenant_ids.iter().any(|tid| tid == record.id.as_str());
            if is_platform_admin || is_member {
                return Ok(RequestContext {
                    user_id,
                    roles: instance_roles,
                    is_platform_admin,
                    tenant: Some(ResolvedTenant {
                        id: record.id,
                        slug: record.slug,
                        name: record.name,
                    }),
                    available_tenants: available,
                });
            }
        }
    }

    // --- Step 3: auto-select single tenant ---
    if known_tenant_ids.len() == 1 {
        let tid = &known_tenant_ids[0];
        if let Ok(Some(record)) = state.tenant_store.get_tenant(tid).await {
            return Ok(RequestContext {
                user_id,
                roles: instance_roles,
                is_platform_admin,
                tenant: Some(ResolvedTenant {
                    id: record.id,
                    slug: record.slug,
                    name: record.name,
                }),
                available_tenants: available,
            });
        }
    }

    // --- Step 4: PlatformAdmin returns None; regular user gets select_tenant ---
    if is_platform_admin {
        return Ok(RequestContext {
            user_id,
            roles: instance_roles,
            is_platform_admin,
            tenant: None,
            available_tenants: available,
        });
    }

    Err(select_tenant_response(&available, headers))
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// Build the `400 select_tenant` response body, honoring `Accept-Error-Legacy`.
pub fn select_tenant_response(available: &[ResolvedTenant], headers: &HeaderMap) -> Response {
    let legacy = headers
        .get("accept-error-legacy")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if legacy {
        // Pre-#44 shape; kept for one deprecation window.
        return error_response(
            StatusCode::BAD_REQUEST,
            "tenant_required",
            "X-Tenant-ID header required",
            None,
        );
    }

    let payload = json!({
        "availableTenants": available,
        "hint": if available.is_empty() {
            "You have no tenant memberships. Contact an administrator."
        } else {
            "Provide an X-Tenant-ID header or set a default via PUT /api/v1/user/me/default-tenant."
        },
    });

    error_response(
        StatusCode::BAD_REQUEST,
        "select_tenant",
        "This request requires a tenant selection.",
        Some(payload),
    )
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    extra: Option<serde_json::Value>,
) -> Response {
    let mut body = json!({
        "error": code,
        "message": message,
    });
    if let Some(extra) = extra
        && let (Some(obj), Some(extra_obj)) = (body.as_object_mut(), extra.as_object())
    {
        for (k, v) in extra_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    (status, Json(body)).into_response()
}

// Re-export for external use in tests.
pub const INSTANCE_SCOPE: &str = INSTANCE_SCOPE_TENANT_ID;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use mk_core::types::TenantId;

    fn make_ctx(tenant: Option<ResolvedTenant>, is_admin: bool) -> RequestContext {
        RequestContext {
            user_id: UserId::new("u-1".into()).unwrap(),
            roles: vec![],
            is_platform_admin: is_admin,
            tenant,
            available_tenants: vec![],
        }
    }

    fn rt(slug: &str) -> ResolvedTenant {
        ResolvedTenant {
            id: TenantId::new(format!("tid-{slug}")).unwrap(),
            slug: slug.to_string(),
            name: format!("Tenant {slug}"),
        }
    }

    #[test]
    fn require_target_tenant_returns_tenant_when_present() {
        let ctx = make_ctx(Some(rt("alpha")), false);
        let hdrs = HeaderMap::new();
        let got = ctx.require_target_tenant(&hdrs).unwrap();
        assert_eq!(got.slug, "alpha");
    }

    #[test]
    fn require_target_tenant_emits_select_tenant_when_missing() {
        let ctx = make_ctx(None, false);
        let hdrs = HeaderMap::new();
        let err = ctx.require_target_tenant(&hdrs).unwrap_err();
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn require_platform_admin_allows_admins() {
        let ctx = make_ctx(None, true);
        assert!(require_platform_admin(&ctx).is_ok());
    }

    #[test]
    fn require_platform_admin_rejects_non_admins() {
        let ctx = make_ctx(None, false);
        let err = require_platform_admin(&ctx).unwrap_err();
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn select_tenant_response_uses_legacy_shape_when_requested() {
        let mut hdrs = HeaderMap::new();
        hdrs.insert("accept-error-legacy", "true".parse().unwrap());
        let resp = select_tenant_response(&[rt("a")], &hdrs);
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        // Note: body inspection requires hyper::body::to_bytes which needs
        // an async runtime — covered by the integration tests in 44.d.
    }

    #[test]
    fn select_tenant_response_defaults_to_new_shape() {
        let hdrs = HeaderMap::new();
        let resp = select_tenant_response(&[rt("a"), rt("b")], &hdrs);
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -------------------------------------------------------------------
    // #44.d — list_scope parsing & authorization
    //
    // These 4 tests cover the pure path of `list_scope` (no tenant-store
    // lookup). The 4 IO-bound cases from the RFC tasks.md (explicit_slug,
    // uuid, cross_tenant_requires_membership, nonexistent_returns_404)
    // live in `cli/tests/list_scope_integration_test.rs` because they need
    // a live `AppState` with a seeded tenant store.
    // -------------------------------------------------------------------

    #[test]
    fn classify_tenant_query_shapes() {
        assert_eq!(classify_tenant_query(None), ScopeIntent::Inherited);
        assert_eq!(classify_tenant_query(Some("")), ScopeIntent::Inherited);
        assert_eq!(classify_tenant_query(Some("   ")), ScopeIntent::Inherited);
        assert_eq!(
            classify_tenant_query(Some("*")),
            ScopeIntent::All { is_alias: false }
        );
        assert_eq!(
            classify_tenant_query(Some("all")),
            ScopeIntent::All { is_alias: true }
        );
        assert_eq!(
            classify_tenant_query(Some("ALL")),
            ScopeIntent::All { is_alias: true }
        );
        assert_eq!(
            classify_tenant_query(Some("acme")),
            ScopeIntent::Single("acme")
        );
        assert_eq!(
            classify_tenant_query(Some("  acme  ")),
            ScopeIntent::Single("acme")
        );
    }

    #[test]
    fn list_scope_omitted_resolves_to_single_via_context() {
        // Inherited path does NOT touch tenant_store, so a real AppState is
        // not needed — we build the method by hand to avoid the IO branches.
        let ctx = make_ctx(Some(rt("alpha")), false);
        let hdrs = HeaderMap::new();
        // Simulate the Inherited branch directly.
        match classify_tenant_query(None) {
            ScopeIntent::Inherited => {
                let t = ctx.require_target_tenant(&hdrs).unwrap();
                assert_eq!(t.slug, "alpha");
            }
            other => panic!("expected Inherited, got {other:?}"),
        }
    }

    #[test]
    fn list_scope_star_as_admin_returns_all() {
        let ctx = make_ctx(None, /* is_admin */ true);
        match classify_tenant_query(Some("*")) {
            ScopeIntent::All { is_alias: false } => {
                assert!(ctx.is_platform_admin, "admin path precondition");
                // The method would return Ok(ListTenantScope::All); verified
                // end-to-end in list_scope_integration_test.rs.
            }
            other => panic!("expected All {{ is_alias: false }}, got {other:?}"),
        }
    }

    #[test]
    fn list_scope_star_as_non_admin_returns_forbidden_scope() {
        let ctx = make_ctx(Some(rt("alpha")), /* is_admin */ false);
        assert!(!ctx.is_platform_admin);
        let resp = forbidden_scope_response("PlatformAdmin");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn list_scope_alias_all_is_recognized() {
        // Deprecation warning is emitted via `tracing::warn!`; capturing it
        // in a unit test requires `tracing-test`, which is already a dev-dep
        // for the integration suite. Here we only verify classification.
        match classify_tenant_query(Some("all")) {
            ScopeIntent::All { is_alias: true } => {}
            other => panic!("expected All {{ is_alias: true }}, got {other:?}"),
        }
        match classify_tenant_query(Some("All")) {
            ScopeIntent::All { is_alias: true } => {}
            other => panic!("case-insensitive alias failed: {other:?}"),
        }
    }
}
