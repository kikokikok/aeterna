//! B4 §3.4 — [`EnvRefResolver`].
//!
//! Resolves `SecretReference::Env { var }` by reading the named
//! environment variable from the server process.
//!
//! Security notes:
//!
//! * The **value** of the env var is a secret. Its **name** is not.
//!   Logging `var = "DATABASE_PASSWORD"` is fine; logging the resolved
//!   bytes is not (and this module never does).
//! * Env vars are process-wide — any code in the same process can
//!   read them. That is the same trust boundary as the server itself,
//!   so no additional ACL is imposed here.
//! * We read via [`std::env::var_os`] so non-UTF-8 byte sequences are
//!   preserved into [`SecretBytes`] without lossy conversion.

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;

use crate::secret_resolver::{ResolveError, SecretRefResolver};

/// Resolver for [`SecretReference::Env`].
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvRefResolver;

impl EnvRefResolver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SecretRefResolver for EnvRefResolver {
    fn kind(&self) -> &'static str {
        "env"
    }

    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        let SecretReference::Env { var } = reference else {
            return Err(ResolveError::WrongKind {
                expected: "env",
                actual: reference.kind(),
            });
        };

        if var.is_empty() {
            return Err(ResolveError::MalformedReference {
                kind: "env",
                reason: "env-var name is empty".to_string(),
            });
        }
        if var.contains('=') || var.contains('\0') {
            return Err(ResolveError::MalformedReference {
                kind: "env",
                reason: "env-var name contains '=' or null byte".to_string(),
            });
        }

        match std::env::var_os(var) {
            Some(val) => {
                // `OsString::into_encoded_bytes` gives us the raw byte
                // sequence without going through lossy UTF-8 conversion.
                // Available since 1.74.
                let bytes = val.into_encoded_bytes();
                Ok(SecretBytes::from(bytes))
            }
            None => Err(ResolveError::NotFound {
                tenant: tenant.clone(),
                kind: "env",
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

    fn env_ref(var: &str) -> SecretReference {
        SecretReference::Env {
            var: var.to_string(),
        }
    }

    // Use a unique env-var prefix per test to avoid cross-test
    // interference in a shared process. Tests that mutate env vars
    // cannot run in parallel safely — we rely on
    // `tokio::test(flavor = "current_thread")` plus unique names.
    const TEST_VAR_OK: &str = "AETERNA_TEST_ENV_RESOLVER_OK_c5b0";
    const TEST_VAR_UNSET: &str = "AETERNA_TEST_ENV_RESOLVER_UNSET_c5b0";

    #[tokio::test]
    async fn reports_kind_env() {
        assert_eq!(EnvRefResolver::new().kind(), "env");
    }

    #[tokio::test]
    async fn resolves_set_variable_to_bytes() {
        // SAFETY: single-threaded test runtime; env-var write visible
        // only to this test via the unique name.
        unsafe { std::env::set_var(TEST_VAR_OK, "hunter2") };
        let r = EnvRefResolver::new();
        let out = r.resolve(&tid(), &env_ref(TEST_VAR_OK)).await.unwrap();
        assert_eq!(out.expose(), b"hunter2");
        unsafe { std::env::remove_var(TEST_VAR_OK) };
    }

    #[tokio::test]
    async fn unset_variable_is_not_found() {
        unsafe { std::env::remove_var(TEST_VAR_UNSET) };
        let r = EnvRefResolver::new();
        let err = r
            .resolve(&tid(), &env_ref(TEST_VAR_UNSET))
            .await
            .unwrap_err();
        match err {
            ResolveError::NotFound { kind, .. } => assert_eq!(kind, "env"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_var_name_is_malformed() {
        let r = EnvRefResolver::new();
        let err = r.resolve(&tid(), &env_ref("")).await.unwrap_err();
        match err {
            ResolveError::MalformedReference { kind, reason } => {
                assert_eq!(kind, "env");
                assert!(reason.contains("empty"), "{reason}");
            }
            other => panic!("expected MalformedReference, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn var_name_with_equals_is_malformed() {
        let r = EnvRefResolver::new();
        let err = r.resolve(&tid(), &env_ref("FOO=BAR")).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::MalformedReference { kind: "env", .. }
        ));
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected() {
        let r = EnvRefResolver::new();
        let file_ref = SecretReference::File {
            path: "/x".to_string(),
        };
        let err = r.resolve(&tid(), &file_ref).await.unwrap_err();
        match err {
            ResolveError::WrongKind { expected, actual } => {
                assert_eq!(expected, "env");
                assert_eq!(actual, "file");
            }
            other => panic!("expected WrongKind, got {other:?}"),
        }
    }
}
