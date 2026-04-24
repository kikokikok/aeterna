//! B4 §3.4 — [`VaultRefResolver`] **stub** (no `vault` feature).
//!
//! The production Vault resolver lives in `vault.rs` behind
//! `#[cfg(feature = "vault")]` and depends on `vaultrs`. This stub
//! satisfies the module's public surface on default builds so call
//! sites can unconditionally reference `VaultRefResolver`. Every
//! resolve call returns [`ResolveError::BackendUnavailable`] with a
//! diagnostic reason directing the operator to rebuild with the
//! `vault` feature enabled.
//!
//! Rationale: optional features are best added with a compile-time
//! stub rather than `Option<VaultRefResolver>` at every call site.
//! This also keeps the [`crate::secret_resolver::SecretResolverRegistry`]
//! construction code identical across builds.

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;

use crate::secret_resolver::{ResolveError, SecretRefResolver};

/// Stub resolver for [`SecretReference::Vault`] (feature disabled).
#[derive(Debug, Default, Clone, Copy)]
pub struct VaultRefResolver;

impl VaultRefResolver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SecretRefResolver for VaultRefResolver {
    fn kind(&self) -> &'static str {
        "vault"
    }

    async fn resolve(
        &self,
        _tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        if reference.kind() != "vault" {
            return Err(ResolveError::WrongKind {
                expected: "vault",
                actual: reference.kind(),
            });
        }
        Err(ResolveError::BackendUnavailable {
            kind: "vault",
            reason: "server built without the `vault` feature; rebuild with --features vault"
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn vault_ref() -> SecretReference {
        SecretReference::Vault {
            mount: "kv".to_string(),
            path: "apps/a".to_string(),
            field: "api_key".to_string(),
        }
    }

    #[tokio::test]
    async fn reports_kind_vault() {
        assert_eq!(VaultRefResolver::new().kind(), "vault");
    }

    #[tokio::test]
    async fn resolve_says_feature_disabled() {
        let r = VaultRefResolver::new();
        let err = r.resolve(&tid(), &vault_ref()).await.unwrap_err();
        match err {
            ResolveError::BackendUnavailable { kind, reason } => {
                assert_eq!(kind, "vault");
                assert!(reason.contains("vault"), "{reason}");
            }
            other => panic!("expected BackendUnavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected() {
        let r = VaultRefResolver::new();
        let env = SecretReference::Env {
            var: "X".to_string(),
        };
        let err = r.resolve(&tid(), &env).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::WrongKind {
                expected: "vault",
                actual: "env"
            }
        ));
    }
}
