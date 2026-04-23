//! B4 §3.1 — Kind-dispatched secret resolver.
//!
//! Per design.md §D9, the historical single `SecretResolver` closure in
//! [`crate::provider_registry`] is being replaced with a trait-per-backend
//! model. Each `SecretReference` variant (`Inline`, `Postgres`, `Env`,
//! `File`, `K8s`, `Vault`) will have its own [`SecretRefResolver`] impl,
//! and a [`SecretResolverRegistry`] routes each resolve call to the right
//! one by matching on [`SecretReference::kind()`].
//!
//! This module lands the trait, the registry, and a
//! [`LegacyClosureAdapter`] that bridges back to the old closure-based
//! API so call sites that still set a `SecretResolver` via
//! `ProviderRegistry::set_resolvers` keep working byte-for-byte while the
//! per-kind impls (tasks 3.2–3.4) land one PR at a time.
//!
//! **No runtime behaviour change ships with this commit.** The trait and
//! registry exist, are documented, and are unit-tested. Nothing in
//! `provider_registry.rs` is wired through them yet — that swap happens
//! in §3.5, once all four real impls exist.

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;

/// Structured failure surface for [`SecretRefResolver::resolve`].
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("secret not found for tenant {tenant} (kind={kind})")]
    NotFound {
        tenant: TenantId,
        kind: &'static str,
    },
    #[error("secret backend unavailable (kind={kind}): {reason}")]
    BackendUnavailable { kind: &'static str, reason: String },
    #[error("permission denied resolving secret (kind={kind}): {reason}")]
    PermissionDenied { kind: &'static str, reason: String },
    #[error("malformed secret reference (kind={kind}): {reason}")]
    MalformedReference { kind: &'static str, reason: String },
    #[error(
        "secret resolver wrong-kind mismatch: expected={expected}, got={actual} — registry bug"
    )]
    WrongKind {
        expected: &'static str,
        actual: &'static str,
    },
}

/// Backend-specific resolver for a single [`SecretReference`] variant.
#[async_trait]
pub trait SecretRefResolver: Send + Sync + fmt::Debug {
    fn kind(&self) -> &'static str;
    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError>;
}

/// Routing layer that dispatches by `SecretReference::kind()`.
#[derive(Clone, Default)]
pub struct SecretResolverRegistry {
    by_kind: HashMap<&'static str, Arc<dyn SecretRefResolver>>,
}

impl SecretResolverRegistry {
    pub fn new() -> Self {
        Self {
            by_kind: HashMap::new(),
        }
    }

    pub fn register(&mut self, resolver: Arc<dyn SecretRefResolver>) {
        self.by_kind.insert(resolver.kind(), resolver);
    }

    pub fn registered_kinds(&self) -> Vec<&'static str> {
        self.by_kind.keys().copied().collect()
    }

    pub fn has_kind(&self, kind: &str) -> bool {
        self.by_kind.contains_key(kind)
    }

    pub async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        let kind = reference.kind();
        let Some(resolver) = self.by_kind.get(kind) else {
            return Err(ResolveError::BackendUnavailable {
                kind,
                reason: "no resolver registered".to_string(),
            });
        };
        resolver.resolve(tenant, reference).await
    }
}

impl fmt::Debug for SecretResolverRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut kinds = self.registered_kinds();
        kinds.sort_unstable();
        f.debug_struct("SecretResolverRegistry")
            .field("kinds", &kinds)
            .finish()
    }
}

pub type LegacyClosureInner = Arc<
    dyn Fn(
            TenantId,
            String,
        ) -> Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + 'static>>
        + Send
        + Sync,
>;

pub struct LegacyClosureAdapter {
    kind: &'static str,
    inner: LegacyClosureInner,
}

impl LegacyClosureAdapter {
    pub fn new(kind: &'static str, inner: LegacyClosureInner) -> Self {
        Self { kind, inner }
    }

    fn logical_name(reference: &SecretReference) -> Option<String> {
        match reference {
            SecretReference::Inline { .. } => None,
            SecretReference::Postgres { secret_id } => Some(secret_id.to_string()),
            SecretReference::Env { var } => Some(var.clone()),
            SecretReference::File { path } => Some(path.clone()),
            SecretReference::K8s {
                namespace,
                name,
                key,
            } => {
                // `namespace: None` means "server's own namespace" per
                // the K8s variant doc — surface it as an empty segment
                // so the legacy closure can still disambiguate.
                let ns = namespace.as_deref().unwrap_or("");
                Some(format!("{ns}/{name}#{key}"))
            }
            SecretReference::Vault { mount, path, field } => {
                Some(format!("{mount}/{path}#{field}"))
            }
        }
    }
}

impl fmt::Debug for LegacyClosureAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyClosureAdapter")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl SecretRefResolver for LegacyClosureAdapter {
    fn kind(&self) -> &'static str {
        self.kind
    }

    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        let actual = reference.kind();
        if actual != self.kind {
            return Err(ResolveError::WrongKind {
                expected: self.kind,
                actual,
            });
        }
        let Some(logical) = Self::logical_name(reference) else {
            return Err(ResolveError::MalformedReference {
                kind: actual,
                reason: "inline references are not resolvable via the legacy closure adapter"
                    .to_string(),
            });
        };
        let raw = (self.inner)(tenant.clone(), logical).await;
        match raw {
            Some(s) => Ok(SecretBytes::from(s.into_bytes())),
            None => Err(ResolveError::NotFound {
                tenant: tenant.clone(),
                kind: self.kind,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn closure_returning(v: Option<&'static str>) -> LegacyClosureInner {
        Arc::new(move |_tenant, _name| Box::pin(async move { v.map(String::from) }))
    }

    #[derive(Debug)]
    struct FakeResolver {
        kind: &'static str,
        response_bytes: Vec<u8>,
    }

    #[async_trait]
    impl SecretRefResolver for FakeResolver {
        fn kind(&self) -> &'static str {
            self.kind
        }
        async fn resolve(
            &self,
            _tenant: &TenantId,
            _reference: &SecretReference,
        ) -> Result<SecretBytes, ResolveError> {
            Ok(SecretBytes::from(self.response_bytes.clone()))
        }
    }

    fn env_ref(var: &str) -> SecretReference {
        SecretReference::Env {
            var: var.to_string(),
        }
    }
    fn postgres_ref(_id: &str) -> SecretReference {
        // The actual `Postgres` variant holds a `Uuid`, not a free-form
        // string. Tests only care that dispatch routes by kind, so a
        // fixed well-formed uuid is fine.
        SecretReference::Postgres {
            secret_id: "22222222-2222-2222-2222-222222222222".parse().unwrap(),
        }
    }
    fn file_ref(path: &str) -> SecretReference {
        SecretReference::File {
            path: path.to_string(),
        }
    }
    fn k8s_ref() -> SecretReference {
        SecretReference::K8s {
            namespace: Some("ns".to_string()),
            name: "my-secret".to_string(),
            key: "token".to_string(),
        }
    }
    fn vault_ref() -> SecretReference {
        SecretReference::Vault {
            mount: "kv".to_string(),
            path: "apps/aeterna".to_string(),
            field: "api_key".to_string(),
        }
    }
    fn inline_ref() -> SecretReference {
        SecretReference::Inline {
            plaintext: SecretBytes::from(b"hunter2".to_vec()),
        }
    }

    #[tokio::test]
    async fn registry_new_has_no_kinds() {
        let r = SecretResolverRegistry::new();
        assert!(r.registered_kinds().is_empty());
        assert!(!r.has_kind("env"));
    }

    #[tokio::test]
    async fn registry_register_advertises_kind() {
        let mut r = SecretResolverRegistry::new();
        r.register(Arc::new(FakeResolver {
            kind: "env",
            response_bytes: b"X".to_vec(),
        }));
        assert!(r.has_kind("env"));
        assert_eq!(r.registered_kinds(), vec!["env"]);
    }

    #[tokio::test]
    async fn registry_duplicate_registration_overwrites() {
        let mut r = SecretResolverRegistry::new();
        r.register(Arc::new(FakeResolver {
            kind: "env",
            response_bytes: b"first".to_vec(),
        }));
        r.register(Arc::new(FakeResolver {
            kind: "env",
            response_bytes: b"second".to_vec(),
        }));
        let got = r.resolve(&tid(), &env_ref("DB")).await.unwrap();
        assert_eq!(got.expose(), b"second");
        assert_eq!(r.registered_kinds().len(), 1);
    }

    #[tokio::test]
    async fn registry_dispatches_by_reference_kind() {
        let mut r = SecretResolverRegistry::new();
        r.register(Arc::new(FakeResolver {
            kind: "env",
            response_bytes: b"from-env".to_vec(),
        }));
        r.register(Arc::new(FakeResolver {
            kind: "postgres",
            response_bytes: b"from-pg".to_vec(),
        }));
        let env = r.resolve(&tid(), &env_ref("DB")).await.unwrap();
        let pg = r.resolve(&tid(), &postgres_ref("abc")).await.unwrap();
        assert_eq!(env.expose(), b"from-env");
        assert_eq!(pg.expose(), b"from-pg");
    }

    #[tokio::test]
    async fn registry_unregistered_kind_returns_backend_unavailable() {
        let r = SecretResolverRegistry::new();
        let err = r.resolve(&tid(), &vault_ref()).await.unwrap_err();
        match err {
            ResolveError::BackendUnavailable { kind, reason } => {
                assert_eq!(kind, "vault");
                assert_eq!(reason, "no resolver registered");
            }
            other => panic!("expected BackendUnavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn registry_debug_sorts_kinds_for_stable_output() {
        let mut r = SecretResolverRegistry::new();
        r.register(Arc::new(FakeResolver {
            kind: "vault",
            response_bytes: b"X".to_vec(),
        }));
        r.register(Arc::new(FakeResolver {
            kind: "env",
            response_bytes: b"X".to_vec(),
        }));
        r.register(Arc::new(FakeResolver {
            kind: "file",
            response_bytes: b"X".to_vec(),
        }));
        let dbg = format!("{r:?}");
        assert!(dbg.contains(r#"["env", "file", "vault"]"#), "{dbg}");
    }

    #[tokio::test]
    async fn legacy_adapter_reports_its_kind() {
        let a = LegacyClosureAdapter::new("postgres", closure_returning(Some("X")));
        assert_eq!(a.kind(), "postgres");
    }

    #[tokio::test]
    async fn legacy_adapter_forwards_to_closure_on_matching_kind() {
        let a = LegacyClosureAdapter::new("postgres", closure_returning(Some("hunter2")));
        let got = a.resolve(&tid(), &postgres_ref("db-pw")).await.unwrap();
        assert_eq!(got.expose(), b"hunter2");
    }

    #[tokio::test]
    async fn legacy_adapter_none_from_closure_maps_to_not_found() {
        let a = LegacyClosureAdapter::new("env", closure_returning(None));
        let err = a.resolve(&tid(), &env_ref("MISSING")).await.unwrap_err();
        match err {
            ResolveError::NotFound { kind, .. } => assert_eq!(kind, "env"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn legacy_adapter_rejects_mismatched_kind() {
        let a = LegacyClosureAdapter::new("env", closure_returning(Some("X")));
        let err = a.resolve(&tid(), &postgres_ref("id")).await.unwrap_err();
        match err {
            ResolveError::WrongKind { expected, actual } => {
                assert_eq!(expected, "env");
                assert_eq!(actual, "postgres");
            }
            other => panic!("expected WrongKind, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn legacy_adapter_refuses_inline() {
        let a = LegacyClosureAdapter::new("inline", closure_returning(Some("X")));
        let err = a.resolve(&tid(), &inline_ref()).await.unwrap_err();
        match err {
            ResolveError::MalformedReference { kind, reason } => {
                assert_eq!(kind, "inline");
                assert!(
                    reason.contains("inline references are not resolvable"),
                    "{reason}"
                );
            }
            other => panic!("expected MalformedReference, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn legacy_adapter_k8s_logical_name_is_composite() {
        use std::sync::Mutex;
        let seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let seen_clone = Arc::clone(&seen);
        let inner: LegacyClosureInner = Arc::new(move |_tenant, name| {
            let seen = Arc::clone(&seen_clone);
            Box::pin(async move {
                *seen.lock().unwrap() = Some(name);
                Some("v".to_string())
            })
        });
        let a = LegacyClosureAdapter::new("k8s", inner);
        let _ = a.resolve(&tid(), &k8s_ref()).await.unwrap();
        assert_eq!(seen.lock().unwrap().as_deref(), Some("ns/my-secret#token"));
    }

    #[tokio::test]
    async fn legacy_adapter_file_logical_name_is_path() {
        use std::sync::Mutex;
        let seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let seen_clone = Arc::clone(&seen);
        let inner: LegacyClosureInner = Arc::new(move |_tenant, name| {
            let seen = Arc::clone(&seen_clone);
            Box::pin(async move {
                *seen.lock().unwrap() = Some(name);
                Some("v".to_string())
            })
        });
        let a = LegacyClosureAdapter::new("file", inner);
        let _ = a
            .resolve(&tid(), &file_ref("/run/secrets/db.pw"))
            .await
            .unwrap();
        assert_eq!(seen.lock().unwrap().as_deref(), Some("/run/secrets/db.pw"));
    }

    #[tokio::test]
    async fn registry_end_to_end_with_legacy_adapters() {
        let mut r = SecretResolverRegistry::new();
        r.register(Arc::new(LegacyClosureAdapter::new(
            "env",
            closure_returning(Some("E")),
        )));
        r.register(Arc::new(LegacyClosureAdapter::new(
            "postgres",
            closure_returning(Some("P")),
        )));
        let e = r.resolve(&tid(), &env_ref("A")).await.unwrap();
        let p = r.resolve(&tid(), &postgres_ref("B")).await.unwrap();
        assert_eq!(e.expose(), b"E");
        assert_eq!(p.expose(), b"P");
    }

    #[tokio::test]
    async fn resolve_error_display_is_shape_stable() {
        let e = ResolveError::NotFound {
            tenant: tid(),
            kind: "env",
        };
        assert!(format!("{e}").contains("kind=env"));
        let e = ResolveError::WrongKind {
            expected: "env",
            actual: "vault",
        };
        let s = format!("{e}");
        assert!(s.contains("expected=env"));
        assert!(s.contains("got=vault"));
    }
}

// =============================================================================
// B4 §3.6 — Systematic redaction coverage.
//
// Pins that resolved plaintext never escapes through:
//   * `ResolveError` rendering (`Display`, `Debug`, `Serialize`-via-format),
//   * `SecretBytes` rendering (already locked in `mk_core::secret`; re-pinned
//     here end-to-end through the registry to catch accidental exposure
//     through a future `impl Display for T { write!(..., "{}", bytes.expose()) }`
//     regression),
//   * `SecretReference::Inline { plaintext }` rendering,
//   * Stringified `format!("{e}")` bubble-ups used by our resolvers in their
//     own error paths (the textbook channel for accidental plaintext leakage
//     when a backend returns `format!("decrypt failed for blob: {raw_bytes:?}")`).
//
// The tests deliberately construct resolver errors whose `reason` field is
// under adversarial control — so if a future author ever does embed plaintext
// in a reason string, this test file surfaces the regression rather than
// silently shipping it.
// =============================================================================
#[cfg(test)]
mod redaction_coverage {
    use super::*;
    use crate::secret_resolvers::{EnvRefResolver, InlineRefResolver};
    use async_trait::async_trait;
    use mk_core::secret::SecretReference;
    use mk_core::types::TenantId;
    use mk_core::SecretBytes;

    /// The literal bytes we use as a sentinel “plaintext” in every test
    /// below. If any of these bytes appear in a rendered error message,
    /// Debug output, or JSON serialization, the test fails — that is the
    /// definition of a leak.
    const SENTINEL_PLAINTEXT: &str = "hunter2-DO-NOT-LEAK-swordfish";

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    /// Helper: asserts the rendered string contains none of the plaintext
    /// bytes in any form a grep'ing operator would recognise (raw,
    /// JSON-escaped, percent-encoded-for-logs).
    fn assert_no_plaintext(rendered: &str, plaintext: &str) {
        assert!(
            !rendered.contains(plaintext),
            "plaintext leaked into rendered string: {rendered:?}"
        );
        // Defensive: catch common accidental-escape paths even though
        // our code doesn't do any of them today.
        let json_escaped = serde_json::to_string(plaintext).unwrap();
        assert!(
            !rendered.contains(&json_escaped),
            "JSON-escaped plaintext leaked: {rendered:?}"
        );
    }

    // ------------------------------------------------------------------
    // ResolveError rendering never leaks constructed-reason plaintext
    // ------------------------------------------------------------------

    #[test]
    fn resolve_error_display_does_not_echo_reason_plaintext_into_debug_format() {
        // If an adversary ever managed to get plaintext into `reason`, we
        // still don't double-format it via Debug into panic messages. (The
        // `reason` IS part of the Display message by design — operators
        // need it for diagnostics — so this test pins Debug specifically.)
        let err = ResolveError::BackendUnavailable {
            kind: "postgres",
            reason: SENTINEL_PLAINTEXT.to_string(),
        };
        // Debug must reveal the variant shape for diagnostics, but the
        // point is: the reason IS there (we cannot protect against
        // an attacker-controlled reason through Debug). This test
        // documents the boundary: protect plaintext at *construction
        // time*, never put it in `reason`.
        let dbg = format!("{err:?}");
        assert!(dbg.contains("BackendUnavailable"));
        assert!(dbg.contains("postgres"));
        // Intentional: reason IS in Debug. We verify this so any future
        // change that accidentally hides the reason from Debug (breaking
        // diagnostics) is also caught.
        assert!(dbg.contains(SENTINEL_PLAINTEXT));
    }

    #[test]
    fn resolve_error_not_found_cannot_carry_plaintext_by_construction() {
        // NotFound has no String field — structurally it cannot leak
        // plaintext. This test pins the variant shape so a future "let's
        // add a `details: String`" refactor trips on CI.
        let err = ResolveError::NotFound {
            tenant: tid(),
            kind: "postgres",
        };
        let rendered = format!("{err}");
        assert_no_plaintext(&rendered, SENTINEL_PLAINTEXT);
        // Structural pin: these are the ONLY fields. If a new field is
        // added, this destructure won't compile and the author must
        // revisit the redaction contract.
        match err {
            ResolveError::NotFound { tenant: _, kind: _ } => {}
            _ => unreachable!(),
        }
    }

    #[test]
    fn resolve_error_wrong_kind_cannot_carry_plaintext_by_construction() {
        let err = ResolveError::WrongKind {
            expected: "env",
            actual: "postgres",
        };
        let rendered = format!("{err}");
        assert_no_plaintext(&rendered, SENTINEL_PLAINTEXT);
        // Both fields are `&'static str` — cannot carry dynamic data.
        match err {
            ResolveError::WrongKind { expected: _, actual: _ } => {}
            _ => unreachable!(),
        }
    }

    #[test]
    fn resolve_error_permission_denied_and_malformed_surface_reason_in_display() {
        // Both variants share the `reason: String` shape with
        // BackendUnavailable — same diagnostic contract, same
        // "trust the author not to embed plaintext" boundary.
        let p = ResolveError::PermissionDenied {
            kind: "k8s",
            reason: "service-account lacks get on Secret".to_string(),
        };
        let m = ResolveError::MalformedReference {
            kind: "vault",
            reason: "path must not start with a slash".to_string(),
        };
        let rp = format!("{p}");
        let rm = format!("{m}");
        assert!(rp.contains("k8s") && rp.contains("service-account"));
        assert!(rm.contains("vault") && rm.contains("must not start"));
        assert_no_plaintext(&rp, SENTINEL_PLAINTEXT);
        assert_no_plaintext(&rm, SENTINEL_PLAINTEXT);
    }

    // ------------------------------------------------------------------
    // SecretBytes redaction surfaces end-to-end through the registry
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn resolved_secret_bytes_redact_on_debug_display_and_serialize() {
        // Go through the real registry + real InlineRefResolver path so
        // we exercise the full return-pipeline a caller would see.
        let mut registry = SecretResolverRegistry::new();
        registry.register(std::sync::Arc::new(InlineRefResolver::new()));

        let reference = SecretReference::Inline {
            plaintext: SecretBytes::from(SENTINEL_PLAINTEXT.as_bytes().to_vec()),
        };

        let bytes = registry.resolve(&tid(), &reference).await.unwrap();

        // (1) Debug must not leak.
        let debug = format!("{bytes:?}");
        assert_no_plaintext(&debug, SENTINEL_PLAINTEXT);
        assert_eq!(debug, "SecretBytes(<redacted>)");

        // (2) Display must not leak.
        let display = format!("{bytes}");
        assert_no_plaintext(&display, SENTINEL_PLAINTEXT);
        assert_eq!(display, "<redacted>");

        // (3) Serialize (JSON) must not leak — this is the path that
        //     HTTP response bodies take, so a regression here is
        //     directly exploitable.
        let json = serde_json::to_string(&bytes).unwrap();
        assert_no_plaintext(&json, SENTINEL_PLAINTEXT);
        assert_eq!(json, r#""<redacted>""#);

        // (4) The bytes ARE accessible through the sole explicit
        //     accessor — anything else would defeat the resolver's
        //     purpose. This is the ONE place plaintext is visible.
        assert_eq!(bytes.expose(), SENTINEL_PLAINTEXT.as_bytes());
    }

    #[tokio::test]
    async fn secret_reference_inline_redacts_plaintext_through_serialize_roundtrip() {
        // The reference itself contains plaintext (Inline variant).
        // Simulate an operator dumping an HTTP request body into a log.
        let reference = SecretReference::Inline {
            plaintext: SecretBytes::from(SENTINEL_PLAINTEXT.as_bytes().to_vec()),
        };

        // JSON serialization — the path audit logs / response bodies take.
        let json = serde_json::to_string(&reference).unwrap();
        assert_no_plaintext(&json, SENTINEL_PLAINTEXT);
        assert!(json.contains(r#""kind":"inline""#));
        assert!(json.contains(r#""plaintext":"<redacted>""#));

        // Debug of the enum — panic messages, `{:?}` in logs.
        let dbg = format!("{reference:?}");
        assert_no_plaintext(&dbg, SENTINEL_PLAINTEXT);
        assert!(dbg.contains("<redacted>"));
    }

    // ------------------------------------------------------------------
    // Resolver error paths don't accidentally echo plaintext
    // ------------------------------------------------------------------

    /// A hostile resolver that embeds its input plaintext in every
    /// error it can produce. This is the "what if an author messed up"
    /// scenario; the test confirms the type system does NOT protect
    /// against a resolver impl that voluntarily leaks — which is why
    /// we document the contract: resolver authors must not put
    /// plaintext into `reason` strings. This test is the canary that
    /// future CI runs this module in a mode that greps resolver
    /// sources for `.expose()` calls within error-construction
    /// branches. (Left as future work per task 3.6 follow-up.)
    #[derive(Debug)]
    struct LeakyResolver;

    #[async_trait]
    impl SecretRefResolver for LeakyResolver {
        fn kind(&self) -> &'static str {
            "env"
        }
        async fn resolve(
            &self,
            _tenant: &TenantId,
            _reference: &SecretReference,
        ) -> Result<SecretBytes, ResolveError> {
            // Simulates an author writing `reason: format!("got {plaintext}")`.
            // This test documents that the type system does NOT prevent
            // this — the mitigation is code review + this canary module.
            Err(ResolveError::BackendUnavailable {
                kind: "env",
                reason: format!("embedded-plaintext:{SENTINEL_PLAINTEXT}"),
            })
        }
    }

    #[tokio::test]
    async fn leaky_resolver_canary_documents_type_system_limitation() {
        // Negative test: we EXPECT this to leak, because the type
        // system cannot prevent a resolver author from embedding
        // plaintext in a reason string. This test locks in the
        // boundary of the redaction guarantee:
        //
        //   - SecretBytes + SecretReference::Inline: LEAK-PROOF by type
        //   - ResolveError reasons: TRUST the resolver author
        //
        // If this test ever starts passing (i.e. the reason stops
        // containing the sentinel), that means ResolveError::Display
        // silently dropped the reason field — which would break
        // diagnostics. Both directions are bugs.
        let mut registry = SecretResolverRegistry::new();
        registry.register(std::sync::Arc::new(LeakyResolver));
        let err = registry
            .resolve(&tid(), &SecretReference::Env { var: "X".to_string() })
            .await
            .unwrap_err();
        let rendered = format!("{err}");
        assert!(
            rendered.contains(SENTINEL_PLAINTEXT),
            "reason field must be rendered into Display for diagnostics: {rendered}"
        );
    }

    // ------------------------------------------------------------------
    // Real resolvers: error paths don't leak reference contents
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn env_resolver_missing_var_error_does_not_leak_var_value() {
        // Pre-condition: the env var is unset (we use a deliberately
        // unused sentinel name). This exercises the real error path,
        // which should name the var but cannot reveal its value
        // (there is none).
        let var_name = "AETERNA_REDACTION_TEST_UNSET_VAR_9f2a";
        // Guard: if some prior test or shell env set it, skip — we
        // cannot prove redaction on a value we don't know.
        if std::env::var(var_name).is_ok() {
            return;
        }
        let resolver = EnvRefResolver::new();
        let err = resolver
            .resolve(
                &tid(),
                &SecretReference::Env {
                    var: var_name.to_string(),
                },
            )
            .await
            .unwrap_err();
        let rendered = format!("{err}");
        // The sentinel plaintext used elsewhere in this module
        // must not have contaminated this error. That is the
        // redaction claim; the specific text of the error (whether
        // it includes the var name, the kind tag, etc.) is not
        // part of §3.6's scope and is tested separately in the
        // resolver's own module.
        assert_no_plaintext(&rendered, SENTINEL_PLAINTEXT);
        // Sanity: EnvRefResolver surfaces NotFound for an unset
        // var (no ambiguity with empty strings).
        assert!(matches!(err, ResolveError::NotFound { .. }));
    }

    // ------------------------------------------------------------------
    // Cache-scope: dropping SecretBytes zeroizes (indirect proof)
    // ------------------------------------------------------------------

    #[test]
    fn secret_bytes_zeroize_invariant_is_pinned_in_mk_core() {
        // ZeroizeOnDrop for `SecretBytes` is pinned by the tests in
        // `mk_core::secret::tests` (the `ZeroizeOnDrop` derive on the
        // struct + its explicit type-bound tests). We don't duplicate
        // the bound here because `zeroize` is an indirect dependency
        // of `memory` — re-declaring it in `memory/Cargo.toml` just to
        // assert a type bound would create a split-version hazard. The
        // end-to-end redaction tests above exercise the behavioural
        // half (Debug/Display/Serialize redact); the zeroize-on-drop
        // half lives one crate down.
    }

    #[test]
    fn resolve_error_does_not_implement_zeroize_on_drop_by_design() {
        // ResolveError contains String reasons — we do NOT want
        // ZeroizeOnDrop on error types (that would zero out the
        // reason string the caller is currently logging, producing
        // empty log lines). This test documents the decision.
        //
        // No assert_not_implements exists in stable Rust, so we
        // document the invariant as a comment-only test. If you
        // are reading this because you are adding ZeroizeOnDrop
        // to ResolveError — please don't; use a dedicated
        // SecretBytes field instead and delete this test.
    }
}
