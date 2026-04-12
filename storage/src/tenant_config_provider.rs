use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use mk_core::traits::TenantConfigProvider;
use mk_core::types::{TenantConfigDocument, TenantId, TenantSecretEntry, TenantSecretReference};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TenantConfigProviderError {
    #[error("invalid tenant id for kubernetes tenant config provider: {0}")]
    InvalidTenantId(String),

    #[error("tenant config validation failed: {0}")]
    Validation(String),
}

#[derive(Debug, Default)]
struct KubernetesState {
    config_maps: HashMap<String, TenantConfigDocument>,
    secrets: HashMap<String, BTreeMap<String, String>>,
}

#[derive(Clone)]
pub struct KubernetesTenantConfigProvider {
    namespace: String,
    state: Arc<RwLock<KubernetesState>>,
}

impl KubernetesTenantConfigProvider {
    #[must_use]
    pub fn new(namespace: String) -> Self {
        Self {
            namespace,
            state: Arc::new(RwLock::new(KubernetesState::default())),
        }
    }

    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    #[must_use]
    pub fn config_map_name_for_tenant(tenant_id: &TenantId) -> String {
        format!("aeterna-tenant-{}", tenant_id.as_str())
    }

    #[must_use]
    pub fn secret_name_for_tenant(tenant_id: &TenantId) -> String {
        format!("aeterna-tenant-{}-secret", tenant_id.as_str())
    }

    fn validate_tenant_id(tenant_id: &TenantId) -> Result<(), TenantConfigProviderError> {
        Uuid::parse_str(tenant_id.as_str()).map_err(|_| {
            TenantConfigProviderError::InvalidTenantId(tenant_id.as_str().to_string())
        })?;
        Ok(())
    }
}

#[async_trait]
impl TenantConfigProvider for KubernetesTenantConfigProvider {
    type Error = TenantConfigProviderError;

    async fn get_config(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Option<TenantConfigDocument>, Self::Error> {
        Self::validate_tenant_id(tenant_id)?;
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
        Self::validate_tenant_id(tenant_id)?;
        if secret.logical_name.trim().is_empty() {
            return Err(TenantConfigProviderError::Validation(
                "secret logical_name must not be empty".to_string(),
            ));
        }
        if secret.secret_value.trim().is_empty() {
            return Err(TenantConfigProviderError::Validation(
                "secret secret_value must not be empty".to_string(),
            ));
        }

        let secret_name = Self::secret_name_for_tenant(tenant_id);
        let reference = TenantSecretReference {
            logical_name: secret.logical_name.clone(),
            ownership: secret.ownership,
            secret_name: secret_name.clone(),
            secret_key: secret.logical_name.clone(),
        };

        let mut state = self.state.write().await;
        state
            .secrets
            .entry(secret_name)
            .or_default()
            .insert(secret.logical_name.clone(), secret.secret_value);

        let config_map_name = Self::config_map_name_for_tenant(tenant_id);
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
            .insert(reference.logical_name.clone(), reference.clone());

        Ok(reference)
    }

    async fn delete_secret_entry(
        &self,
        tenant_id: &TenantId,
        logical_name: &str,
    ) -> Result<bool, Self::Error> {
        Self::validate_tenant_id(tenant_id)?;
        let secret_name = Self::secret_name_for_tenant(tenant_id);
        let config_map_name = Self::config_map_name_for_tenant(tenant_id);

        let mut state = self.state.write().await;
        let removed_from_secret = state
            .secrets
            .get_mut(&secret_name)
            .map(|secret_data| secret_data.remove(logical_name).is_some())
            .unwrap_or(false);

        let removed_from_config = state
            .config_maps
            .get_mut(&config_map_name)
            .map(|doc| doc.secret_references.remove(logical_name).is_some())
            .unwrap_or(false);

        Ok(removed_from_secret || removed_from_config)
    }

    async fn get_secret_value(
        &self,
        tenant_id: &TenantId,
        logical_name: &str,
    ) -> Result<Option<String>, Self::Error> {
        Self::validate_tenant_id(tenant_id)?;
        let secret_name = Self::secret_name_for_tenant(tenant_id);
        let state = self.state.read().await;
        Ok(state
            .secrets
            .get(&secret_name)
            .and_then(|secret_data| secret_data.get(logical_name).cloned()))
    }

    async fn validate(&self, config: &TenantConfigDocument) -> Result<(), Self::Error> {
        Self::validate_tenant_id(&config.tenant_id)?;

        if config.contains_raw_secret_material() {
            return Err(TenantConfigProviderError::Validation(
                "config payload contains raw secret material in non-secret fields".to_string(),
            ));
        }

        let expected_secret_name = Self::secret_name_for_tenant(&config.tenant_id);
        for (logical_name, reference) in &config.secret_references {
            if logical_name != &reference.logical_name {
                return Err(TenantConfigProviderError::Validation(format!(
                    "secret reference key '{logical_name}' must match logical_name '{}'",
                    reference.logical_name
                )));
            }
            if reference.secret_name != expected_secret_name {
                return Err(TenantConfigProviderError::Validation(format!(
                    "secret reference '{}' targets '{}' but must target '{}'",
                    reference.logical_name, reference.secret_name, expected_secret_name
                )));
            }
            if reference.secret_key.trim().is_empty() {
                return Err(TenantConfigProviderError::Validation(format!(
                    "secret reference '{}' must include secret_key",
                    reference.logical_name
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{TenantConfigField, TenantConfigOwnership};
    use serde_json::json;

    fn tenant_id() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn provider() -> KubernetesTenantConfigProvider {
        KubernetesTenantConfigProvider::new("default".to_string())
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
                    secret_value: "super-secret-value".to_string(),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            reference.secret_name,
            "aeterna-tenant-11111111-1111-1111-1111-111111111111-secret"
        );

        let after_secret = provider.get_config(&tenant_id).await.unwrap().unwrap();
        assert!(after_secret.secret_references.contains_key("repo.token"));

        let serialized = serde_json::to_string(&after_secret).unwrap();
        assert!(!serialized.contains("super-secret-value"));

        let secret_name = KubernetesTenantConfigProvider::secret_name_for_tenant(&tenant_id);
        let state = provider.state.read().await;
        assert_eq!(
            state
                .secrets
                .get(&secret_name)
                .and_then(|values| values.get("repo.token"))
                .map(String::as_str),
            Some("super-secret-value")
        );
        drop(state);

        let deleted = provider
            .delete_secret_entry(&tenant_id, "repo.token")
            .await
            .unwrap();
        assert!(deleted);
        let after_delete = provider.get_config(&tenant_id).await.unwrap().unwrap();
        assert!(!after_delete.secret_references.contains_key("repo.token"));
    }

    #[tokio::test]
    async fn rejects_cross_tenant_secret_reference() {
        let provider = provider();
        let tenant_id = tenant_id();
        let mut refs = BTreeMap::new();
        refs.insert(
            "repo.token".to_string(),
            TenantSecretReference {
                logical_name: "repo.token".to_string(),
                ownership: TenantConfigOwnership::Tenant,
                secret_name: "aeterna-tenant-22222222-2222-2222-2222-222222222222-secret"
                    .to_string(),
                secret_key: "repo.token".to_string(),
            },
        );

        let doc = TenantConfigDocument {
            tenant_id,
            fields: BTreeMap::new(),
            secret_references: refs,
        };

        let err = provider.upsert_config(doc).await.unwrap_err();
        assert!(matches!(err, TenantConfigProviderError::Validation(_)));
        assert!(err.to_string().contains("must target"));
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
    async fn rejects_non_uuid_tenant_ids_for_kubernetes_naming_contract() {
        let provider = provider();
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let err = provider.get_config(&tenant_id).await.unwrap_err();
        assert!(matches!(err, TenantConfigProviderError::InvalidTenantId(_)));
    }
}
