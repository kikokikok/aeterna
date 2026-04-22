//! Tenant config document provider with secret storage delegated to a
//! [`SecretBackend`].
//!
//! # History
//!
//! This module used to be named "Kubernetes" and maintained a second
//! parallel `HashMap<secret_name, HashMap<key, String>>` that *looked* like
//! a `kubectl get secret` payload but was actually just process-local
//! memory. That design lost every secret on pod restart and ran in addition
//! to the git-token-focused `storage::secret_provider`, giving the service
//! *two* incompatible secret stores at once.
//!
//! The public struct is still called [`KubernetesTenantConfigProvider`] for
//! backwards compatibility with construction sites in the CLI crate; the
//! rename to `InMemoryTenantConfigProvider` is a follow-up drive-by. The
//! behaviour change is what matters:
//!
//! - `TenantConfigDocument` values still live in a `HashMap` (config-doc
//!   persistence is out of scope for B1 — tracked separately).
//! - Secret **material** now flows through a [`SecretBackend`] injected
//!   into the constructor (in production:
//!   [`crate::secret_backend::PostgresSecretBackend`] with envelope
//!   encryption; in tests a `NullSecretBackend`).
//! - The returned [`TenantSecretReference`] carries a [`SecretReference`]
//!   enum, not Kubernetes-shaped `secret_name`/`secret_key` strings.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::traits::TenantConfigProvider;
use mk_core::types::{TenantConfigDocument, TenantId, TenantSecretEntry, TenantSecretReference};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::secret_backend::{SecretBackend, SecretBackendError};

const DEFAULT_K8S_NAMESPACE: &str = "default";

#[derive(Debug, Error)]
pub enum TenantConfigProviderError {
    #[error("invalid tenant id for tenant config provider: {0}")]
    InvalidTenantId(String),

    #[error("tenant config validation failed: {0}")]
    Validation(String),

    #[error("secret backend error: {0}")]
    Secret(#[from] SecretBackendError),
}

#[derive(Debug, Default)]
struct InMemoryState {
    config_maps: HashMap<String, TenantConfigDocument>,
}

/// Tenant config document provider.
///
/// Config documents are kept in an in-memory `HashMap` (config-doc
/// persistence is out of scope for B1). Secret material is delegated to
/// the injected [`SecretBackend`].
#[derive(Clone)]
pub struct KubernetesTenantConfigProvider {
    namespace: String,
    state: Arc<RwLock<InMemoryState>>,
    secret_backend: Arc<dyn SecretBackend>,
}

impl KubernetesTenantConfigProvider {
    #[must_use]
    pub fn new(namespace: String, secret_backend: Arc<dyn SecretBackend>) -> Self {
        Self {
            namespace,
            state: Arc::new(RwLock::new(InMemoryState::default())),
            secret_backend,
        }
    }

    /// Test-only constructor that wires up an
    /// [`crate::secret_backend::InMemorySecretBackend`]. Used by the 8
    /// in-file test fixtures across the CLI crate to keep the call sites
    /// one-liners; NOT intended for production wiring.
    #[must_use]
    pub fn new_in_memory_for_tests(namespace: String) -> Self {
        Self::new(
            namespace,
            Arc::new(crate::secret_backend::InMemorySecretBackend::new()),
        )
    }

    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    #[must_use]
    pub fn config_map_name_for_tenant(tenant_id: &TenantId) -> String {
        format!("aeterna-tenant-{}", tenant_id.as_str())
    }

    /// Parse the caller's [`TenantId`] string as a UUID for backend routing.
    ///
    /// Historically this provider required tenant IDs to be the UUID form of
    /// the tenant's primary key (that's what k8s secret naming encoded). The
    /// new `SecretBackend` API takes `Uuid` directly, so the constraint
    /// stays — callers that pass slugs need to resolve to UUID first
    /// (see `TenantStore::get_tenant`).
    fn tenant_uuid(tenant_id: &TenantId) -> Result<Uuid, TenantConfigProviderError> {
        Uuid::parse_str(tenant_id.as_str())
            .map_err(|_| TenantConfigProviderError::InvalidTenantId(tenant_id.as_str().to_string()))
    }
}

#[async_trait]
impl TenantConfigProvider for KubernetesTenantConfigProvider {
    type Error = TenantConfigProviderError;

    async fn get_config(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Option<TenantConfigDocument>, Self::Error> {
        Self::tenant_uuid(tenant_id)?;
        let config_map_name = Self::config_map_name_for_tenant(tenant_id);
        let state = self.state.read().await;
        Ok(state.config_maps.get(&config_map_name).cloned())
    }

    async fn list_configs(&self) -> Result<Vec<TenantConfigDocument>, Self::Error> {
        let state = self.state.read().await;
        let mut values: Vec<_> = state.config_maps.values().cloned().collect();
        values.sort_by(|left, right| left.tenant_id.as_str().cmp(right.tenant_id.as_str()));
        Ok(values)
    }

    async fn upsert_config(
        &self,
        config: TenantConfigDocument,
    ) -> Result<TenantConfigDocument, Self::Error> {
        self.validate(&config).await?;
        let config_map_name = Self::config_map_name_for_tenant(&config.tenant_id);
        let mut state = self.state.write().await;
        state.config_maps.insert(config_map_name, config.clone());
        Ok(config)
    }

    async fn set_secret_entry(
        &self,
        tenant_id: &TenantId,
        secret: TenantSecretEntry,
    ) -> Result<TenantSecretReference, Self::Error> {
        let tenant_uuid = Self::tenant_uuid(tenant_id)?;
        if secret.logical_name.trim().is_empty() {
            return Err(TenantConfigProviderError::Validation(
                "secret logical_name must not be empty".to_string(),
            ));
        }
        if secret.secret_value.expose().is_empty() {
            return Err(TenantConfigProviderError::Validation(
                "secret secret_value must not be empty".to_string(),
            ));
        }

        // Delegate the actual ciphertext storage to the secret backend.
        let reference = self
            .secret_backend
            .put(tenant_uuid, &secret.logical_name, secret.secret_value)
            .await?;

        let tsr = TenantSecretReference {
            logical_name: secret.logical_name.clone(),
            ownership: secret.ownership,
            reference,
        };

        // Persist the reference in the tenant's config document so that
        // subsequent `get_config` calls surface it.
        let config_map_name = Self::config_map_name_for_tenant(tenant_id);
        let mut state = self.state.write().await;
        let entry =
            state
                .config_maps
                .entry(config_map_name)
                .or_insert_with(|| TenantConfigDocument {
                    tenant_id: tenant_id.clone(),
                    fields: BTreeMap::new(),
                    secret_references: BTreeMap::new(),
                });
        entry
            .secret_references
            .insert(tsr.logical_name.clone(), tsr.clone());

        Ok(tsr)
    }

    async fn delete_secret_entry(
        &self,
        tenant_id: &TenantId,
        logical_name: &str,
    ) -> Result<bool, Self::Error> {
        Self::tenant_uuid(tenant_id)?;
        let config_map_name = Self::config_map_name_for_tenant(tenant_id);

        // Pull the reference out of the config doc first so we can delete the
        // backend-side ciphertext even if the config entry is missing for
        // whatever reason.
        let reference_opt = {
            let mut state = self.state.write().await;
            state
                .config_maps
                .get_mut(&config_map_name)
                .and_then(|doc| doc.secret_references.remove(logical_name))
                .map(|tsr| tsr.reference)
        };

        if let Some(reference) = reference_opt {
            self.secret_backend.delete(&reference).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_secret_bytes(
        &self,
        tenant_id: &TenantId,
        logical_name: &str,
    ) -> Result<Option<SecretBytes>, Self::Error> {
        Self::tenant_uuid(tenant_id)?;
        let config_map_name = Self::config_map_name_for_tenant(tenant_id);
        let reference = {
            let state = self.state.read().await;
            state
                .config_maps
                .get(&config_map_name)
                .and_then(|doc| doc.secret_references.get(logical_name).cloned())
        };

        let Some(tsr) = reference else {
            return Ok(None);
        };

        match self.secret_backend.get(&tsr.reference).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(SecretBackendError::NotFound(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn validate(&self, config: &TenantConfigDocument) -> Result<(), Self::Error> {
        Self::tenant_uuid(&config.tenant_id)?;

        if config.contains_raw_secret_material() {
            return Err(TenantConfigProviderError::Validation(
                "config payload contains raw secret material in non-secret fields".to_string(),
            ));
        }

        for (logical_name, reference) in &config.secret_references {
            if logical_name != &reference.logical_name {
                return Err(TenantConfigProviderError::Validation(format!(
                    "secret reference key '{logical_name}' must match logical_name '{}'",
                    reference.logical_name
                )));
            }
            if reference.logical_name.trim().is_empty() {
                return Err(TenantConfigProviderError::Validation(
                    "secret reference logical_name must not be empty".to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mk_core::SecretReference;
    use mk_core::types::{TenantConfigField, TenantConfigOwnership};
    use serde_json::json;
    use std::sync::Mutex;

    /// In-process fake `SecretBackend` — keyed by `(tenant_uuid, logical_name)`.
    /// Adequate for the provider's orchestration tests; the real encryption
    /// path is exercised by `storage::secret_backend` integration tests.
    #[derive(Default)]
    struct FakeSecretBackend {
        store: Mutex<HashMap<Uuid, SecretBytes>>,
        by_key: Mutex<HashMap<(Uuid, String), Uuid>>,
    }

    #[async_trait]
    impl SecretBackend for FakeSecretBackend {
        async fn put(
            &self,
            tenant_db_id: Uuid,
            logical_name: &str,
            value: SecretBytes,
        ) -> Result<SecretReference, SecretBackendError> {
            let key = (tenant_db_id, logical_name.to_string());
            let id = {
                let mut by_key = self.by_key.lock().unwrap();
                *by_key.entry(key).or_insert_with(Uuid::new_v4)
            };
            self.store.lock().unwrap().insert(id, value);
            Ok(SecretReference::Postgres { secret_id: id })
        }

        async fn get(
            &self,
            reference: &SecretReference,
        ) -> Result<SecretBytes, SecretBackendError> {
            let secret_id = match reference {
                SecretReference::Postgres { secret_id } => secret_id,
                other => return Err(SecretBackendError::UnsupportedReference(other.kind())),
            };
            self.store
                .lock()
                .unwrap()
                .get(secret_id)
                .cloned()
                .ok_or_else(|| SecretBackendError::NotFound(secret_id.to_string()))
        }

        async fn delete(&self, reference: &SecretReference) -> Result<(), SecretBackendError> {
            let secret_id = match reference {
                SecretReference::Postgres { secret_id } => secret_id,
                other => return Err(SecretBackendError::UnsupportedReference(other.kind())),
            };
            self.store.lock().unwrap().remove(secret_id);
            Ok(())
        }

        async fn list(
            &self,
            tenant_db_id: Uuid,
        ) -> Result<Vec<(String, SecretReference)>, SecretBackendError> {
            let by_key = self.by_key.lock().unwrap();
            let mut out: Vec<_> = by_key
                .iter()
                .filter(|((t, _), _)| *t == tenant_db_id)
                .map(|((_, name), id)| (name.clone(), SecretReference::Postgres { secret_id: *id }))
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(out)
        }
    }

    fn tenant_id() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn provider() -> KubernetesTenantConfigProvider {
        KubernetesTenantConfigProvider::new(
            DEFAULT_K8S_NAMESPACE.to_string(),
            Arc::new(FakeSecretBackend::default()),
        )
    }

    #[tokio::test]
    async fn validates_and_persists_basic_crud_flow() {
        let provider = provider();
        let tenant_id = tenant_id();
        let mut fields = BTreeMap::new();
        fields.insert(
            "runtime.logLevel".to_string(),
            TenantConfigField {
                ownership: TenantConfigOwnership::Tenant,
                value: json!("info"),
            },
        );

        let doc = TenantConfigDocument {
            tenant_id: tenant_id.clone(),
            fields,
            secret_references: BTreeMap::new(),
        };
        let saved = provider.upsert_config(doc).await.unwrap();
        assert_eq!(saved.tenant_id, tenant_id);

        let found = provider.get_config(&tenant_id).await.unwrap().unwrap();
        assert_eq!(found.fields.len(), 1);

        let list = provider.list_configs().await.unwrap();
        assert_eq!(list.len(), 1);

        let reference = provider
            .set_secret_entry(
                &tenant_id,
                TenantSecretEntry {
                    logical_name: "repo.token".to_string(),
                    ownership: TenantConfigOwnership::Tenant,
                    secret_value: SecretBytes::from(b"super-secret-value".to_vec()),
                },
            )
            .await
            .unwrap();
        // Returned reference carries a Postgres SecretReference, not k8s strings.
        assert!(matches!(
            reference.reference,
            SecretReference::Postgres { .. }
        ));

        let after_secret = provider.get_config(&tenant_id).await.unwrap().unwrap();
        assert!(after_secret.secret_references.contains_key("repo.token"));

        // Serialization must never leak plaintext — neither the raw value
        // (it's not in the document at all) nor via SecretBytes (redacted).
        let serialized = serde_json::to_string(&after_secret).unwrap();
        assert!(!serialized.contains("super-secret-value"));

        // Roundtrip through the backend.
        let bytes = provider
            .get_secret_bytes(&tenant_id, "repo.token")
            .await
            .unwrap()
            .expect("secret must be retrievable");
        assert_eq!(bytes.expose(), b"super-secret-value");

        let deleted = provider
            .delete_secret_entry(&tenant_id, "repo.token")
            .await
            .unwrap();
        assert!(deleted);
        let after_delete = provider.get_config(&tenant_id).await.unwrap().unwrap();
        assert!(!after_delete.secret_references.contains_key("repo.token"));
        assert!(
            provider
                .get_secret_bytes(&tenant_id, "repo.token")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn rejects_raw_secret_material_in_config_payload() {
        let provider = provider();
        let tenant_id = tenant_id();
        let mut fields = BTreeMap::new();
        fields.insert(
            "database.password".to_string(),
            TenantConfigField {
                ownership: TenantConfigOwnership::Tenant,
                value: json!("plain-text-secret"),
            },
        );

        let doc = TenantConfigDocument {
            tenant_id,
            fields,
            secret_references: BTreeMap::new(),
        };
        let err = provider.upsert_config(doc).await.unwrap_err();
        assert!(matches!(err, TenantConfigProviderError::Validation(_)));
        assert!(err.to_string().contains("raw secret material"));
    }

    #[tokio::test]
    async fn rejects_non_uuid_tenant_ids() {
        let provider = provider();
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let err = provider.get_config(&tenant_id).await.unwrap_err();
        assert!(matches!(err, TenantConfigProviderError::InvalidTenantId(_)));
    }
}
