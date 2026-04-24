//! B4 §3.5 Phase B — [`PostgresRefResolver`].
//!
//! Resolves `SecretReference::Postgres { secret_id }` by delegating
//! to a [`SecretBackend`](storage::secret_backend::SecretBackend).
//! This is the new home of the envelope-encrypted Postgres secret
//! dispatch that previously lived inside
//! `KubernetesTenantConfigProvider::get_secret_bytes`.
//!
//! The resolver holds an `Arc<dyn SecretBackend>` — the same instance
//! constructed in `bootstrap.rs` and shared with other consumers (git
//! token resolution, etc.) so there is exactly one KMS+Postgres
//! client pool in the process.
//!
//! Non-Postgres variants return [`ResolveError::WrongKind`]. The
//! underlying [`SecretBackend::get`] already surfaces
//! `UnsupportedReference` for those, but we short-circuit here to
//! keep error kinds precise and avoid hitting the DB for a call we
//! know will fail.

use std::sync::Arc;

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;
use storage::secret_backend::{SecretBackend, SecretBackendError};

use crate::secret_resolver::{ResolveError, SecretRefResolver};

/// Resolver for [`SecretReference::Postgres`].
#[derive(Clone)]
pub struct PostgresRefResolver {
    backend: Arc<dyn SecretBackend>,
}

impl std::fmt::Debug for PostgresRefResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresRefResolver")
            .field("backend", &"<dyn SecretBackend>")
            .finish()
    }
}

impl PostgresRefResolver {
    pub fn new(backend: Arc<dyn SecretBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl SecretRefResolver for PostgresRefResolver {
    fn kind(&self) -> &'static str {
        "postgres"
    }

    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        if !matches!(reference, SecretReference::Postgres { .. }) {
            return Err(ResolveError::WrongKind {
                expected: "postgres",
                actual: reference.kind(),
            });
        }

        match self.backend.get(reference).await {
            Ok(bytes) => Ok(bytes),
            Err(SecretBackendError::NotFound(_)) => Err(ResolveError::NotFound {
                tenant: tenant.clone(),
                kind: "postgres",
            }),
            Err(SecretBackendError::UnsupportedReference(k)) => {
                // Should be unreachable given the matches! above, but
                // surface it clearly rather than panicking.
                Err(ResolveError::MalformedReference {
                    kind: "postgres",
                    reason: format!(
                        "SecretBackend reports variant {k} unsupported; expected postgres"
                    ),
                })
            }
            Err(e) => Err(ResolveError::BackendUnavailable {
                kind: "postgres",
                reason: format!("{e}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use uuid::Uuid;

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    /// Minimal in-memory SecretBackend used to drive the resolver's
    /// error-mapping logic without spinning up Postgres.
    #[derive(Debug, Default)]
    struct StubBackend {
        // Single pre-programmed response, reused across every call.
        // Wrapped in Mutex so we can construct via Default.
        response: Mutex<Option<Result<Vec<u8>, SecretBackendError>>>,
    }

    impl StubBackend {
        fn ok(bytes: &[u8]) -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Some(Ok(bytes.to_vec()))),
            })
        }
        fn not_found() -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Some(Err(SecretBackendError::NotFound("stub".to_string())))),
            })
        }
        fn db_error() -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Some(Err(SecretBackendError::Aead("stub".to_string())))),
            })
        }
    }

    #[async_trait]
    impl SecretBackend for StubBackend {
        async fn put(
            &self,
            _tenant: Uuid,
            _logical_name: &str,
            _value: SecretBytes,
        ) -> Result<SecretReference, SecretBackendError> {
            unimplemented!("not used in these tests")
        }
        async fn list(
            &self,
            _tenant: Uuid,
        ) -> Result<Vec<(String, SecretReference)>, SecretBackendError> {
            Ok(Vec::new())
        }
        async fn get(
            &self,
            _reference: &SecretReference,
        ) -> Result<SecretBytes, SecretBackendError> {
            match self.response.lock().unwrap().as_ref() {
                Some(Ok(v)) => Ok(SecretBytes::from(v.clone())),
                Some(Err(SecretBackendError::NotFound(s))) => {
                    Err(SecretBackendError::NotFound(s.clone()))
                }
                Some(Err(SecretBackendError::Aead(s))) => Err(SecretBackendError::Aead(s.clone())),
                Some(Err(_)) | None => Err(SecretBackendError::Aead("stub".to_string())),
            }
        }
        async fn delete(&self, _reference: &SecretReference) -> Result<(), SecretBackendError> {
            Ok(())
        }
    }

    fn pg_ref() -> SecretReference {
        SecretReference::Postgres {
            secret_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
        }
    }

    #[tokio::test]
    async fn reports_kind_postgres() {
        let r = PostgresRefResolver::new(StubBackend::ok(b"x"));
        assert_eq!(r.kind(), "postgres");
    }

    #[tokio::test]
    async fn happy_path_returns_bytes() {
        let r = PostgresRefResolver::new(StubBackend::ok(b"plaintext"));
        let out = r.resolve(&tid(), &pg_ref()).await.unwrap();
        assert_eq!(out.expose(), b"plaintext");
    }

    #[tokio::test]
    async fn not_found_is_retagged_with_tenant() {
        let r = PostgresRefResolver::new(StubBackend::not_found());
        let tenant = tid();
        let err = r.resolve(&tenant, &pg_ref()).await.unwrap_err();
        match err {
            ResolveError::NotFound { tenant: t, kind } => {
                assert_eq!(t, tenant);
                assert_eq!(kind, "postgres");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn db_error_surfaces_as_backend_unavailable() {
        let r = PostgresRefResolver::new(StubBackend::db_error());
        let err = r.resolve(&tid(), &pg_ref()).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::BackendUnavailable {
                kind: "postgres",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected_without_backend_call() {
        let r = PostgresRefResolver::new(StubBackend::ok(b"x"));
        let env = SecretReference::Env {
            var: "X".to_string(),
        };
        let err = r.resolve(&tid(), &env).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::WrongKind {
                expected: "postgres",
                actual: "env"
            }
        ));
    }
}
