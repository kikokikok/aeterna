//! B4 §3.2 — [`K8sRefResolver`] for `SecretReference::K8s`.
//!
//! Design split: the resolver itself is generic over a
//! [`K8sSecretFetcher`] trait, which abstracts the actual HTTP round-
//! trip to the Kubernetes API server. This lets us unit-test the
//! resolver (reference validation, error mapping, base64 decoding) with
//! an in-process mock fetcher, while the production build uses
//! [`PodDownwardApiFetcher`] — a live client reading credentials from
//! the pod's projected service-account volume.
//!
//! Authentication + namespace discovery (production):
//!
//! * Bearer token: `/var/run/secrets/kubernetes.io/serviceaccount/token`
//! * Cluster CA:   `/var/run/secrets/kubernetes.io/serviceaccount/ca.crt`
//! * Namespace fallback (when the reference omits it):
//!                 `/var/run/secrets/kubernetes.io/serviceaccount/namespace`
//! * API server:   `KUBERNETES_SERVICE_HOST` + `KUBERNETES_SERVICE_PORT`
//!
//! The live fetcher is gated behind `feature = "k8s-secrets"`; when the
//! feature is disabled, `PodDownwardApiFetcher::fetch` returns
//! [`ResolveError::BackendUnavailable`] so call sites that accidentally
//! rely on it surface a clear error instead of silently succeeding.
//!
//! Not covered here (out of B4 scope):
//!
//! * Informer-based watch / caching — each resolve is a fresh GET.
//! * Server-side-apply of secret metadata — this module only reads.
//! * TLS pinning beyond the cluster CA — `rustls-native-certs` is not
//!   used; we trust the projected CA bundle exclusively.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;
use mk_core::SecretBytes;

use crate::secret_resolver::{ResolveError, SecretRefResolver};

/// Abstraction over “fetch the `key` field of Secret `namespace/name`
/// from the Kubernetes API”. Implemented by both the live HTTP
/// fetcher and by test mocks.
///
/// Implementations MUST return the **decoded** secret value bytes
/// (not the base64-encoded wire form). The resolver does not try to
/// post-process the returned bytes.
#[async_trait]
pub trait K8sSecretFetcher: Send + Sync + fmt::Debug {
    async fn fetch(
        &self,
        namespace: &str,
        name: &str,
        key: &str,
    ) -> Result<SecretBytes, ResolveError>;
}

/// Resolver for [`SecretReference::K8s`].
///
/// Construct with [`K8sRefResolver::new`] and any
/// [`K8sSecretFetcher`]. The fetcher is [`Arc`]-cloned into every
/// resolve call; use `Arc<DynFetcher>` as the type parameter when you
/// need dynamic dispatch.
#[derive(Debug, Clone)]
pub struct K8sRefResolver<F: K8sSecretFetcher> {
    fetcher: Arc<F>,
    /// Default namespace when the reference omits it.
    /// Typically the pod's own namespace read from the downward API.
    default_namespace: Option<String>,
}

impl<F: K8sSecretFetcher> K8sRefResolver<F> {
    pub fn new(fetcher: F) -> Self {
        Self {
            fetcher: Arc::new(fetcher),
            default_namespace: None,
        }
    }

    pub fn with_default_namespace(mut self, ns: impl Into<String>) -> Self {
        self.default_namespace = Some(ns.into());
        self
    }
}

#[async_trait]
impl<F: K8sSecretFetcher + 'static> SecretRefResolver for K8sRefResolver<F> {
    fn kind(&self) -> &'static str {
        "k8s"
    }

    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        let SecretReference::K8s { namespace, name, key } = reference else {
            return Err(ResolveError::WrongKind {
                expected: "k8s",
                actual: reference.kind(),
            });
        };

        if name.is_empty() || key.is_empty() {
            return Err(ResolveError::MalformedReference {
                kind: "k8s",
                reason: "k8s reference must have non-empty name and key".to_string(),
            });
        }

        // Namespace resolution precedence:
        //   1. explicit namespace on the reference
        //   2. resolver's default_namespace (from downward API)
        //   3. error — we refuse to silently default to `"default"`
        //      because that'd read from an unrelated namespace.
        let ns = match (namespace.as_deref(), self.default_namespace.as_deref()) {
            (Some(n), _) if !n.is_empty() => n,
            (_, Some(d)) if !d.is_empty() => d,
            _ => {
                return Err(ResolveError::MalformedReference {
                    kind: "k8s",
                    reason: "no namespace on reference and resolver has no default".to_string(),
                });
            }
        };

        // DNS-1123 sanity check on the Secret name — we don't want to
        // inject arbitrary strings into the API path. Kubernetes
        // already enforces this, but defense-in-depth.
        if !is_dns_1123_subdomain(name) || !is_dns_1123_subdomain(ns) {
            return Err(ResolveError::MalformedReference {
                kind: "k8s",
                reason: "namespace/name is not a valid DNS-1123 subdomain".to_string(),
            });
        }

        let bytes = self.fetcher.fetch(ns, name, key).await.map_err(|e| {
            // Re-tag BackendUnavailable/NotFound so downstream logs
            // show `kind = "k8s"` even if the fetcher emitted a
            // generic error.
            match e {
                ResolveError::NotFound { .. } => ResolveError::NotFound {
                    tenant: tenant.clone(),
                    kind: "k8s",
                },
                other => other,
            }
        })?;

        Ok(bytes)
    }
}

/// Lightweight DNS-1123 subdomain check per RFC-1123 §2.1.
///
/// We apply this to both namespace names and Secret names. This is a
/// security boundary — we interpolate these into an HTTP path, so we
/// want to reject anything containing slashes, `..`, etc.
fn is_dns_1123_subdomain(s: &str) -> bool {
    if s.is_empty() || s.len() > 253 {
        return false;
    }
    // DNS-1123 subdomain: lowercase alphanumerics, `-`, `.`; must
    // start/end with alphanumeric (lowercase or digit).
    let is_start_end = |b: u8| b.is_ascii_digit() || (b.is_ascii_lowercase());
    let first = s.as_bytes()[0];
    let last = s.as_bytes()[s.len() - 1];
    if !is_start_end(first) || !is_start_end(last) {
        return false;
    }
    s.bytes()
        .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-' || b == b'.')
}

// ---------------------------------------------------------------------------
// PodDownwardApiFetcher — live implementation (feature-gated).
// ---------------------------------------------------------------------------

/// Production fetcher. Reads credentials from the projected service-
/// account volume; issues HTTPS GETs against the in-cluster API
/// server; base64-decodes the returned Secret.
#[derive(Debug, Clone)]
pub struct PodDownwardApiFetcher {
    // Held so the Debug output carries something meaningful even when
    // the feature is disabled.
    note: &'static str,
    #[cfg(feature = "k8s-secrets")]
    inner: Arc<live::LiveK8sFetcher>,
}

impl PodDownwardApiFetcher {
    /// Create a fetcher from the pod's ambient service-account env.
    ///
    /// Returns an error at construction time if the feature is enabled
    /// but the SA volume / env vars are missing. On feature-disabled
    /// builds, always succeeds and every `fetch` returns
    /// [`ResolveError::BackendUnavailable`].
    pub fn from_pod_environment() -> Result<Self, ResolveError> {
        #[cfg(feature = "k8s-secrets")]
        {
            let inner = live::LiveK8sFetcher::from_pod_environment()?;
            Ok(Self {
                note: "live",
                inner: Arc::new(inner),
            })
        }
        #[cfg(not(feature = "k8s-secrets"))]
        {
            Ok(Self {
                note: "stub (build without --features k8s-secrets)",
            })
        }
    }

    /// Read `/var/run/secrets/kubernetes.io/serviceaccount/namespace`
    /// — the pod's own namespace, suitable as a
    /// [`K8sRefResolver::with_default_namespace`] input.
    ///
    /// Returns `None` if the file is absent (non-pod environment).
    pub async fn read_pod_namespace() -> Option<String> {
        let path = "/var/run/secrets/kubernetes.io/serviceaccount/namespace";
        match tokio::fs::read_to_string(path).await {
            Ok(s) => {
                let t = s.trim();
                if t.is_empty() { None } else { Some(t.to_string()) }
            }
            Err(_) => None,
        }
    }
}

#[async_trait]
impl K8sSecretFetcher for PodDownwardApiFetcher {
    async fn fetch(
        &self,
        namespace: &str,
        name: &str,
        key: &str,
    ) -> Result<SecretBytes, ResolveError> {
        #[cfg(feature = "k8s-secrets")]
        {
            let _ = &self.note;
            self.inner.fetch(namespace, name, key).await
        }
        #[cfg(not(feature = "k8s-secrets"))]
        {
            let _ = (namespace, name, key, &self.note);
            Err(ResolveError::BackendUnavailable {
                kind: "k8s",
                reason: "server built without the `k8s-secrets` feature; rebuild with --features k8s-secrets"
                    .to_string(),
            })
        }
    }
}

#[cfg(feature = "k8s-secrets")]
mod live {
    //! Live Kubernetes HTTP client. Private submodule so tests don't
    //! have to compile it without the feature flag.
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    use serde_json::Value as JsonValue;
    use std::time::Duration;

    const SA_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
    const SA_CA_CERT_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";

    #[derive(Debug)]
    pub(super) struct LiveK8sFetcher {
        client: reqwest::Client,
        base_url: String, // e.g. https://10.0.0.1:443
        token: mk_core::SecretBytes, // bearer token; redacted on Debug
    }

    impl LiveK8sFetcher {
        pub(super) fn from_pod_environment() -> Result<Self, ResolveError> {
            let host = std::env::var("KUBERNETES_SERVICE_HOST").map_err(|_| {
                ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: "KUBERNETES_SERVICE_HOST unset".to_string(),
                }
            })?;
            let port = std::env::var("KUBERNETES_SERVICE_PORT").unwrap_or_else(|_| "443".into());
            let base_url = format!("https://{host}:{port}");

            // Read CA cert (PEM) and SA token. Both are blocking reads,
            // but from_pod_environment runs at bootstrap time — fine.
            let ca_pem = std::fs::read(SA_CA_CERT_PATH).map_err(|e| {
                ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: format!("read CA cert {SA_CA_CERT_PATH}: {e}"),
                }
            })?;
            let token_raw = std::fs::read(SA_TOKEN_PATH).map_err(|e| {
                ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: format!("read SA token {SA_TOKEN_PATH}: {e}"),
                }
            })?;
            let token = mk_core::SecretBytes::from(token_raw);

            let ca = reqwest::Certificate::from_pem(&ca_pem).map_err(|e| {
                ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: format!("parse CA cert PEM: {e}"),
                }
            })?;
            let client = reqwest::Client::builder()
                .add_root_certificate(ca)
                .https_only(true)
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: format!("build http client: {e}"),
                })?;

            Ok(Self { client, base_url, token })
        }

        pub(super) async fn fetch(
            &self,
            namespace: &str,
            name: &str,
            key: &str,
        ) -> Result<SecretBytes, ResolveError> {
            let url = format!(
                "{}/api/v1/namespaces/{}/secrets/{}",
                self.base_url, namespace, name
            );
            // Token as UTF-8 is fine — SA tokens are JWTs (ASCII).
            let token_str = std::str::from_utf8(self.token.expose()).map_err(|_| {
                ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: "SA token is not valid UTF-8".to_string(),
                }
            })?;

            let resp = self
                .client
                .get(&url)
                .bearer_auth(token_str)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| ResolveError::BackendUnavailable {
                    kind: "k8s",
                    reason: format!("HTTP GET failed: {e}"),
                })?;

            match resp.status().as_u16() {
                200 => {}
                401 | 403 => {
                    return Err(ResolveError::PermissionDenied {
                        kind: "k8s",
                        reason: format!(
                            "HTTP {} reading {namespace}/{name} — service-account RBAC missing 'get' on secrets",
                            resp.status().as_u16()
                        ),
                    });
                }
                404 => {
                    // Dummy tenant — the outer `K8sRefResolver::resolve`
                    // retags this NotFound with the caller's real
                    // TenantId before it escapes the module.
                    return Err(ResolveError::NotFound {
                        tenant: TenantId::new(
                            "00000000-0000-0000-0000-000000000000".to_string(),
                        )
                        .expect("dummy zero-uuid is a valid TenantId"),
                        kind: "k8s",
                    });
                }
                other => {
                    return Err(ResolveError::BackendUnavailable {
                        kind: "k8s",
                        reason: format!("HTTP {other} from API server"),
                    });
                }
            }

            let body: JsonValue = resp.json().await.map_err(|e| ResolveError::BackendUnavailable {
                kind: "k8s",
                reason: format!("parse Secret JSON: {e}"),
            })?;

            // Secrets look like: { "data": { "<key>": "<base64>" }, ... }
            let b64 = body
                .get("data")
                .and_then(|d| d.get(key))
                .and_then(|v| v.as_str())
                .ok_or_else(|| ResolveError::MalformedReference {
                    kind: "k8s",
                    reason: format!("key '{key}' not present in Secret {namespace}/{name}"),
                })?;

            let raw = B64.decode(b64).map_err(|e| ResolveError::BackendUnavailable {
                kind: "k8s",
                reason: format!("base64 decode of Secret value failed: {e}"),
            })?;
            Ok(SecretBytes::from(raw))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn k8s_ref(ns: Option<&str>, name: &str, key: &str) -> SecretReference {
        SecretReference::K8s {
            namespace: ns.map(str::to_string),
            name: name.to_string(),
            key: key.to_string(),
        }
    }

    #[derive(Debug)]
    struct MockFetcher {
        calls: Mutex<Vec<(String, String, String)>>,
        response: Mutex<Result<Vec<u8>, ResolveError>>,
    }

    impl MockFetcher {
        fn ok(bytes: &[u8]) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                response: Mutex::new(Ok(bytes.to_vec())),
            }
        }
        fn err(e: ResolveError) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                response: Mutex::new(Err(e)),
            }
        }
        fn last_call(&self) -> Option<(String, String, String)> {
            self.calls.lock().unwrap().last().cloned()
        }
    }

    #[async_trait]
    impl K8sSecretFetcher for MockFetcher {
        async fn fetch(
            &self,
            namespace: &str,
            name: &str,
            key: &str,
        ) -> Result<SecretBytes, ResolveError> {
            self.calls
                .lock()
                .unwrap()
                .push((namespace.to_string(), name.to_string(), key.to_string()));
            match &*self.response.lock().unwrap() {
                Ok(v) => Ok(SecretBytes::from(v.clone())),
                Err(e) => Err(clone_err(e)),
            }
        }
    }

    fn clone_err(e: &ResolveError) -> ResolveError {
        match e {
            ResolveError::NotFound { tenant, kind } => {
                ResolveError::NotFound { tenant: tenant.clone(), kind }
            }
            ResolveError::PermissionDenied { kind, reason } => {
                ResolveError::PermissionDenied { kind, reason: reason.clone() }
            }
            ResolveError::BackendUnavailable { kind, reason } => {
                ResolveError::BackendUnavailable { kind, reason: reason.clone() }
            }
            ResolveError::MalformedReference { kind, reason } => {
                ResolveError::MalformedReference { kind, reason: reason.clone() }
            }
            ResolveError::WrongKind { expected, actual } => {
                ResolveError::WrongKind { expected: *expected, actual: *actual }
            }
        }
    }

    #[tokio::test]
    async fn reports_kind_k8s() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"x"));
        assert_eq!(r.kind(), "k8s");
    }

    #[tokio::test]
    async fn explicit_namespace_wins_over_default() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"payload"))
            .with_default_namespace("fallback-ns");
        let out = r
            .resolve(&tid(), &k8s_ref(Some("explicit-ns"), "my-secret", "api-key"))
            .await
            .unwrap();
        assert_eq!(out.expose(), b"payload");
        let last = r.fetcher.last_call().unwrap();
        assert_eq!(last.0, "explicit-ns");
        assert_eq!(last.1, "my-secret");
        assert_eq!(last.2, "api-key");
    }

    #[tokio::test]
    async fn default_namespace_used_when_reference_omits_it() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"payload"))
            .with_default_namespace("pod-ns");
        r.resolve(&tid(), &k8s_ref(None, "my-secret", "api-key")).await.unwrap();
        assert_eq!(r.fetcher.last_call().unwrap().0, "pod-ns");
    }

    #[tokio::test]
    async fn missing_namespace_and_no_default_is_malformed() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"x"));
        let err = r.resolve(&tid(), &k8s_ref(None, "my-secret", "k")).await.unwrap_err();
        match err {
            ResolveError::MalformedReference { kind, reason } => {
                assert_eq!(kind, "k8s");
                assert!(reason.contains("namespace"), "{reason}");
            }
            other => panic!("expected MalformedReference, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_name_or_key_is_malformed() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"x")).with_default_namespace("ns");
        let err = r.resolve(&tid(), &k8s_ref(None, "", "k")).await.unwrap_err();
        assert!(matches!(err, ResolveError::MalformedReference { kind: "k8s", .. }));
        let err = r.resolve(&tid(), &k8s_ref(None, "n", "")).await.unwrap_err();
        assert!(matches!(err, ResolveError::MalformedReference { kind: "k8s", .. }));
    }

    #[tokio::test]
    async fn path_injection_is_rejected() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"x")).with_default_namespace("ns");
        let bad = SecretReference::K8s {
            namespace: Some("../etc".to_string()),
            name: "passwd".to_string(),
            key: "k".to_string(),
        };
        let err = r.resolve(&tid(), &bad).await.unwrap_err();
        assert!(matches!(err, ResolveError::MalformedReference { kind: "k8s", .. }));
    }

    #[tokio::test]
    async fn fetcher_notfound_is_retagged_with_tenant() {
        let r = K8sRefResolver::new(MockFetcher::err(ResolveError::NotFound {
            tenant: TenantId::new("00000000-0000-0000-0000-000000000000".to_string()).unwrap(),
            kind: "k8s",
        }))
        .with_default_namespace("ns");
        let tenant = tid();
        let err = r.resolve(&tenant, &k8s_ref(None, "n", "k")).await.unwrap_err();
        match err {
            ResolveError::NotFound { tenant: t, kind } => {
                assert_eq!(t, tenant);
                assert_eq!(kind, "k8s");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected() {
        let r = K8sRefResolver::new(MockFetcher::ok(b"x")).with_default_namespace("ns");
        let env = SecretReference::Env { var: "X".to_string() };
        let err = r.resolve(&tid(), &env).await.unwrap_err();
        assert!(matches!(err, ResolveError::WrongKind { expected: "k8s", actual: "env" }));
    }

    #[test]
    fn dns1123_accepts_valid_and_rejects_bad() {
        assert!(is_dns_1123_subdomain("my-ns"));
        assert!(is_dns_1123_subdomain("abc.def.ghi"));
        assert!(is_dns_1123_subdomain("a"));
        assert!(!is_dns_1123_subdomain(""));
        assert!(!is_dns_1123_subdomain("-bad"));
        assert!(!is_dns_1123_subdomain("bad-"));
        assert!(!is_dns_1123_subdomain("ABC"));
        assert!(!is_dns_1123_subdomain("has space"));
        assert!(!is_dns_1123_subdomain("has/slash"));
        assert!(!is_dns_1123_subdomain(".."));
    }

    #[tokio::test]
    async fn stub_fetcher_errors_on_feature_disabled_build() {
        // This test always runs; when the feature is on we skip.
        #[cfg(not(feature = "k8s-secrets"))]
        {
            let fetcher = PodDownwardApiFetcher::from_pod_environment().unwrap();
            let err = fetcher.fetch("ns", "n", "k").await.unwrap_err();
            match err {
                ResolveError::BackendUnavailable { kind, reason } => {
                    assert_eq!(kind, "k8s");
                    assert!(reason.contains("k8s-secrets"), "{reason}");
                }
                other => panic!("expected BackendUnavailable, got {other:?}"),
            }
        }
    }
}
