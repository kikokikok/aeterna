//! #44.d §6 — shared CLI args for cross-tenant scoping.
//!
//! Wired into `aeterna user list`, `aeterna org list`, and `aeterna govern
//! audit`. The two flags are mutually exclusive at parse time (clap
//! `conflicts_with`); see `to_query_param` for the mapping to the
//! `?tenant=` grammar consumed by the server.
//!
//! Design choice: one tiny args struct, flattened into each list command,
//! instead of three copy-pasted pairs of flags. Keeps the UX consistent
//! (same flag names, same help text) and gives us a single place to add
//! future scopes (e.g. `--tenants a,b,c`) without touching every site.

use clap::Args;

/// Cross-tenant scoping flags for list commands.
///
/// Maps 1:1 to the `?tenant=` query grammar documented in
/// `docs/api/admin.md`:
///
/// | Flag              | Resolves to             |
/// |-------------------|-------------------------|
/// | (none)            | no query param (legacy) |
/// | `--all-tenants`   | `?tenant=*`             |
/// | `--tenant <slug>` | `?tenant=<slug>`        |
///
/// The `--all-tenants` / `--tenant` pair is mutually exclusive at parse
/// time. Combining them is ALWAYS a client bug (what would it mean?) so
/// rejecting at the clap layer surfaces the mistake before a request is
/// issued.
#[derive(Args, Debug, Clone, Default)]
pub struct TenantScopeArgs {
    /// List across every tenant (PlatformAdmin only).
    ///
    /// Emits the cross-tenant envelope
    /// (`{success, scope: "all", items: [...]}`) with every item decorated
    /// with `tenantId` / `tenantSlug` / `tenantName`. See
    /// `docs/api/admin.md` for the full contract.
    #[arg(long = "all-tenants", conflicts_with = "tenant")]
    pub all_tenants: bool,

    /// List a specific foreign tenant by slug or id (PlatformAdmin only).
    ///
    /// Emits the single-foreign-tenant envelope
    /// (`{success, scope: "tenant", tenant: {...}, items: [...]}`).
    /// Returns `404 tenant_not_found` if the slug/id doesn't exist.
    #[arg(
        long = "tenant",
        value_name = "SLUG_OR_ID",
        conflicts_with = "all_tenants"
    )]
    pub tenant: Option<String>,
}

impl TenantScopeArgs {
    /// Value to send as the `?tenant=` query parameter, if any.
    ///
    /// Returns `None` when no scoping flag was passed — callers should
    /// fall through to the legacy tenant-scoped path. Returns `Some("*")`
    /// for `--all-tenants` and `Some(slug)` for `--tenant <slug>`.
    pub fn to_query_param(&self) -> Option<String> {
        if self.all_tenants {
            Some("*".to_string())
        } else {
            self.tenant.clone()
        }
    }

    /// True when any cross-tenant scope is requested (for callers that
    /// want to branch on display logic, e.g. show an extra "Tenant" column).
    pub fn is_cross_tenant(&self) -> bool {
        self.all_tenants || self.tenant.is_some()
    }
}

/// Description of the listing payload as received from the server.
///
/// CLI list commands (`user list`, `org list`, `govern audit`) can receive
/// one of THREE shapes since #44.d:
///
/// 1. Bare JSON array — pre-#44.d legacy shape, returned when no
///    `?tenant=` is sent. Callers MUST continue to support this shape for
///    backward compatibility.
/// 2. `{success, scope: "all", items: [...]}` — `?tenant=*` / `all` path.
/// 3. `{success, scope: "tenant", tenant: {id, slug, name}, items: [...]}`
///    — `?tenant=<slug>` path (not applicable to `/govern/audit`).
///
/// `ListPayload::from_json` normalizes all three into a single iterable
/// view + an optional banner the CLI can print above the table.
pub struct ListPayload<'a> {
    /// Flattened items array, regardless of envelope shape.
    pub items: Vec<&'a serde_json::Value>,
    /// Human-readable banner line to print above the table, or None for
    /// the legacy path. Examples: `"Cross-tenant view: all tenants"`,
    /// `"Tenant view: acme (01HW...)"`.
    pub banner: Option<String>,
}

impl<'a> ListPayload<'a> {
    /// Normalize any of the three list-response shapes. Falls back to an
    /// empty list + no banner if the payload is malformed (we prefer
    /// "show an empty table" over "crash the CLI"). The server-level
    /// tests guard the contract so this defensive branch should be dead
    /// in practice.
    pub fn from_json(value: &'a serde_json::Value) -> Self {
        // Legacy bare-array shape.
        if let Some(arr) = value.as_array() {
            return Self {
                items: arr.iter().collect(),
                banner: None,
            };
        }
        // Envelope shape — must have an items[] array.
        let items: Vec<&serde_json::Value> = value
            .get("items")
            .and_then(|i| i.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let banner = match value.get("scope").and_then(|s| s.as_str()) {
            Some("all") => Some("Cross-tenant view: all tenants".to_string()),
            Some("tenant") => {
                let slug = value
                    .pointer("/tenant/slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let id = value
                    .pointer("/tenant/id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                Some(format!("Tenant view: {slug} ({id})"))
            }
            _ => None,
        };
        Self { items, banner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_flags_yields_no_query_param() {
        let args = TenantScopeArgs::default();
        assert_eq!(args.to_query_param(), None);
        assert!(!args.is_cross_tenant());
    }

    #[test]
    fn all_tenants_maps_to_star() {
        let args = TenantScopeArgs {
            all_tenants: true,
            tenant: None,
        };
        assert_eq!(args.to_query_param().as_deref(), Some("*"));
        assert!(args.is_cross_tenant());
    }

    #[test]
    fn slug_passes_through_verbatim() {
        let args = TenantScopeArgs {
            all_tenants: false,
            tenant: Some("acme".into()),
        };
        assert_eq!(args.to_query_param().as_deref(), Some("acme"));
        assert!(args.is_cross_tenant());
    }

    #[test]
    fn list_payload_handles_legacy_bare_array() {
        let v = serde_json::json!([{"id": "a"}, {"id": "b"}]);
        let p = ListPayload::from_json(&v);
        assert_eq!(p.items.len(), 2);
        assert!(p.banner.is_none(), "legacy path must NOT show a banner");
    }

    #[test]
    fn list_payload_handles_scope_all_envelope() {
        let v = serde_json::json!({
            "success": true,
            "scope": "all",
            "items": [{"id": "a", "tenantSlug": "t1"}],
        });
        let p = ListPayload::from_json(&v);
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.banner.as_deref(), Some("Cross-tenant view: all tenants"));
    }

    #[test]
    fn list_payload_handles_scope_tenant_envelope() {
        let v = serde_json::json!({
            "success": true,
            "scope": "tenant",
            "tenant": {"id": "01HW0000", "slug": "acme", "name": "Acme"},
            "items": [{"id": "a"}, {"id": "b"}, {"id": "c"}],
        });
        let p = ListPayload::from_json(&v);
        assert_eq!(p.items.len(), 3);
        let banner = p.banner.expect("tenant-scope banner must be set");
        // Assert the banner contains both slug and id so operators can
        // visually confirm WHICH tenant they're viewing — both fields
        // matter (slug for humans, id for log/support correlation).
        assert!(banner.contains("acme"), "banner missing slug: {banner}");
        assert!(banner.contains("01HW0000"), "banner missing id: {banner}");
    }

    #[test]
    fn list_payload_tolerates_malformed_envelope() {
        // Envelope with no items key — defensive fallback path. Must not
        // panic; should produce an empty items[] + no banner.
        let v = serde_json::json!({"success": true, "scope": "unknown"});
        let p = ListPayload::from_json(&v);
        assert!(p.items.is_empty());
        assert!(p.banner.is_none());
    }

    #[test]
    fn clap_rejects_both_flags_together() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct Harness {
            #[command(flatten)]
            scope: TenantScopeArgs,
        }
        // Both flags specified — clap must reject via conflicts_with so we
        // never reach run-time with an ambiguous scope.
        let err = Harness::try_parse_from(["t", "--all-tenants", "--tenant", "acme"])
            .err()
            .expect("clap should reject conflicting scope flags");
        let msg = err.to_string();
        assert!(
            msg.contains("cannot be used with") || msg.contains("conflict"),
            "expected a conflict error, got: {msg}"
        );
    }
}
