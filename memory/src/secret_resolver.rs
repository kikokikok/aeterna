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
