//! B4 §3.5 Phase B — [`InlineRefResolver`].
//!
//! Resolves `SecretReference::Inline { plaintext }` by returning the
//! embedded [`SecretBytes`] directly. Inline references are the
//! path taken by tests and by ephemeral manifests that ship the
//! secret material inside the config document itself (the manifest
//! validator forbids Inline in prod deployments, but we still need
//! the resolver for unit tests and for --format inline development
//! flows).
//!
//! No I/O, no configuration. Construction is pure.

use async_trait::async_trait;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;
use mk_core::SecretBytes;

use crate::secret_resolver::{ResolveError, SecretRefResolver};

/// Resolver for [`SecretReference::Inline`].
#[derive(Debug, Default, Clone, Copy)]
pub struct InlineRefResolver;

impl InlineRefResolver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SecretRefResolver for InlineRefResolver {
    fn kind(&self) -> &'static str {
        "inline"
    }

    async fn resolve(
        &self,
        _tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        match reference {
            SecretReference::Inline { plaintext } => {
                // Clone the bytes so the resolver's caller owns an
                // independent SecretBytes. The source SecretBytes in
                // the reference stays intact (the reference is held
                // by the in-memory config document).
                Ok(SecretBytes::from(plaintext.expose().to_vec()))
            }
            other => Err(ResolveError::WrongKind {
                expected: "inline",
                actual: other.kind(),
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

    #[tokio::test]
    async fn reports_kind_inline() {
        assert_eq!(InlineRefResolver::new().kind(), "inline");
    }

    #[tokio::test]
    async fn returns_embedded_plaintext() {
        let r = InlineRefResolver::new();
        let reference = SecretReference::Inline {
            plaintext: SecretBytes::from(b"hunter2".to_vec()),
        };
        let out = r.resolve(&tid(), &reference).await.unwrap();
        assert_eq!(out.expose(), b"hunter2");
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected() {
        let r = InlineRefResolver::new();
        let env = SecretReference::Env { var: "X".to_string() };
        let err = r.resolve(&tid(), &env).await.unwrap_err();
        assert!(matches!(err, ResolveError::WrongKind { expected: "inline", actual: "env" }));
    }
}
