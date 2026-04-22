//! Reverse-render a persisted tenant's state as a `TenantManifest`-shaped
//! document. B3 tasks 2.2 + 2.3.
//!
//! # What this is for
//!
//! GitOps workflows need a read-side counterpart to `POST provision_tenant`:
//! "show me the manifest I could re-apply to recreate this tenant's
//! current state". This module produces that document, with two modes:
//!
//! * **Full** (`redact=false`): includes field values, secret-reference
//!   logical names, and repository credentials. Gated at the HTTP layer
//!   by the `PlatformAdmin` role.
//! * **Redacted** (`redact=true`): secret-reference logical names are
//!   replaced with opaque placeholders (`secret-0`, `secret-1`, ...) and
//!   repository `credentialRef` is elided. Safe to share with callers
//!   holding `tenant:read` only.
//!
//! # What this is NOT for
//!
//! Not a round-trip guarantee. Several manifest sections are persisted
//! through different pipelines (hierarchy via governance, roles via
//! role_grants, providers via the provider_registry) and reading them
//! back cleanly would require threading through authorization contexts
//! this module does not own. Sections we cannot currently reverse-render
//! are enumerated in [`RenderedManifest::not_rendered`] so callers can
//! tell the difference between "this section is empty" and "this section
//! was not read". The full-fidelity renderer is a later task (§2.4 diff
//! needs it to be trustworthy; until then, only the listed sections
//! participate in a meaningful diff).

use std::collections::BTreeMap;
use std::sync::Arc;

use mk_core::traits::TenantConfigProvider;
use mk_core::types::{
    TenantConfigField, TenantConfigOwnership, TenantId, TenantRecord, TenantSecretReference,
};
use serde::Serialize;
use serde_json::{Value, json};

use super::AppState;

/// Sections this renderer knows how to reverse-render today.
pub const RENDERED_SECTIONS: &[&str] = &["tenant", "metadata", "config", "repository"];

/// Sections a full `TenantManifest` can carry but this renderer does
/// not yet cover. Reflected into `RenderedManifest::not_rendered`.
pub const NOT_RENDERED_SECTIONS: &[&str] = &[
    "domainMappings",
    "secrets",
    "hierarchy",
    "roles",
    "providers",
];

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("tenant not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedManifest {
    pub api_version: String,
    pub kind: String,
    pub metadata: RenderedMetadata,
    pub tenant: RenderedTenant,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<RenderedConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<Value>,
    pub not_rendered: Vec<&'static str>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedMetadata {
    pub generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_hash: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub status: String,
    pub source_owner: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedConfig {
    pub fields: BTreeMap<String, TenantConfigField>,
    pub secret_references: BTreeMap<String, RenderedSecretRef>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedSecretRef {
    pub logical_name: String,
    pub ownership: TenantConfigOwnership,
    #[serde(flatten)]
    pub reference: Value,
}

pub async fn render_current_manifest(
    state: &Arc<AppState>,
    tenant_ref: &str,
    redact: bool,
) -> Result<RenderedManifest, RenderError> {
    let record: TenantRecord = state
        .tenant_store
        .get_tenant(tenant_ref)
        .await
        .map_err(|e| RenderError::Storage(e.to_string()))?
        .ok_or_else(|| RenderError::NotFound(tenant_ref.to_string()))?;

    let tenant_id: TenantId = record.id.clone();

    // `get_manifest_state` takes a slug (not a `TenantId`) and returns
    // a flat `(Option<hash>, generation_i64)` tuple. The generation
    // column is `BIGINT NOT NULL DEFAULT 0` in migration 027, so zero
    // is the schema default for a tenant that has never been applied.
    let (manifest_hash, generation_i64) = state
        .tenant_store
        .get_manifest_state(&record.slug)
        .await
        .map_err(|e| RenderError::Storage(e.to_string()))?;
    // u64 at the wire. Negative generations are a schema-invariant
    // violation; we clamp with `max(0)` rather than panic so the
    // renderer stays read-only.
    let generation: u64 = generation_i64.max(0) as u64;

    let config_doc = state
        .tenant_config_provider
        .get_config(&tenant_id)
        .await
        .map_err(|e| RenderError::Storage(e.to_string()))?;

    let rendered_config =
        config_doc.map(|doc| render_config(doc.fields, doc.secret_references, redact));

    // Repository binding lives on its own store (see AppState):
    // `TenantRepositoryBindingStore`, not `TenantStore`. Kept separate
    // in storage because the sync-owner guard logic there is orthogonal
    // to tenant lifecycle.
    let binding = state
        .tenant_repository_binding_store
        .get_binding(&tenant_id)
        .await
        .map_err(|e| RenderError::Storage(e.to_string()))?;

    let rendered_repository = binding.map(|b| {
        if redact {
            b.redacted()
        } else {
            json!({
                "id": b.id,
                "tenantId": b.tenant_id.as_str(),
                "kind": b.kind.to_string(),
                "localPath": b.local_path,
                "remoteUrl": b.remote_url,
                "branch": b.branch,
                "branchPolicy": b.branch_policy.to_string(),
                "credentialKind": b.credential_kind.to_string(),
                "credentialRef": b.credential_ref,
                "gitProviderConnectionId": b.git_provider_connection_id,
                "githubOwner": b.github_owner,
                "githubRepo": b.github_repo,
                "sourceOwner": b.source_owner.to_string(),
                "createdAt": b.created_at,
                "updatedAt": b.updated_at,
            })
        }
    });

    Ok(RenderedManifest {
        api_version: "aeterna.io/v1".to_string(),
        kind: "TenantManifest".to_string(),
        metadata: RenderedMetadata {
            generation,
            manifest_hash,
        },
        tenant: RenderedTenant {
            id: record.id.as_str().to_string(),
            slug: record.slug,
            name: record.name,
            status: record.status.to_string(),
            source_owner: record.source_owner.to_string(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        },
        config: rendered_config,
        repository: rendered_repository,
        not_rendered: NOT_RENDERED_SECTIONS.to_vec(),
    })
}

pub(crate) fn render_config(
    fields: BTreeMap<String, TenantConfigField>,
    secret_references: BTreeMap<String, TenantSecretReference>,
    redact: bool,
) -> RenderedConfig {
    let secret_references = if redact {
        redact_secret_references(secret_references)
    } else {
        secret_references
            .into_iter()
            .map(|(key, sref)| {
                let reference_value =
                    serde_json::to_value(&sref.reference).unwrap_or_else(|_| json!(null));
                (
                    key,
                    RenderedSecretRef {
                        logical_name: sref.logical_name,
                        ownership: sref.ownership,
                        reference: reference_value,
                    },
                )
            })
            .collect()
    };

    RenderedConfig {
        fields,
        secret_references,
    }
}

/// Replace every secret reference with an opaque placeholder.
/// Indexing follows `BTreeMap` sorted-key iteration so the placeholder
/// assignment is deterministic across calls and pods.
fn redact_secret_references(
    input: BTreeMap<String, TenantSecretReference>,
) -> BTreeMap<String, RenderedSecretRef> {
    let mut out = BTreeMap::new();
    for (index, (_original_key, sref)) in input.into_iter().enumerate() {
        let placeholder = format!("secret-{index}");
        out.insert(
            placeholder.clone(),
            RenderedSecretRef {
                logical_name: placeholder,
                ownership: sref.ownership,
                reference: json!({"kind": "redacted"}),
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::secret::SecretReference;
    use uuid::Uuid;

    fn field(val: &str) -> TenantConfigField {
        TenantConfigField {
            ownership: TenantConfigOwnership::Tenant,
            value: serde_json::Value::String(val.to_string()),
        }
    }

    fn sref(logical_name: &str) -> TenantSecretReference {
        TenantSecretReference {
            logical_name: logical_name.to_string(),
            ownership: TenantConfigOwnership::Tenant,
            reference: SecretReference::Postgres {
                secret_id: Uuid::from_u128(0xDEAD_BEEF),
            },
        }
    }

    #[test]
    fn render_config_passes_fields_through_unchanged() {
        let mut fields = BTreeMap::new();
        fields.insert("region".to_string(), field("eu-west-1"));
        fields.insert("model".to_string(), field("claude-opus-4-7"));

        let out = render_config(fields.clone(), BTreeMap::new(), false);
        assert_eq!(out.fields, fields);
        assert!(out.secret_references.is_empty());
    }

    #[test]
    fn render_config_full_mode_preserves_secret_logical_names() {
        let mut refs = BTreeMap::new();
        refs.insert("openaiKey".to_string(), sref("openaiKey"));
        refs.insert("anthropicKey".to_string(), sref("anthropicKey"));

        let out = render_config(BTreeMap::new(), refs, false);
        assert_eq!(out.secret_references.len(), 2);
        assert_eq!(out.secret_references["openaiKey"].logical_name, "openaiKey");
        assert_eq!(
            out.secret_references["openaiKey"].reference["kind"],
            "postgres"
        );
    }

    #[test]
    fn render_config_redact_mode_replaces_names_deterministically() {
        let mut refs = BTreeMap::new();
        refs.insert("openaiKey".to_string(), sref("openaiKey"));
        refs.insert("anthropicKey".to_string(), sref("anthropicKey"));
        refs.insert("githubToken".to_string(), sref("githubToken"));

        let out = render_config(BTreeMap::new(), refs, true);
        let keys: Vec<&String> = out.secret_references.keys().collect();
        assert_eq!(keys, vec!["secret-0", "secret-1", "secret-2"]);
        for v in out.secret_references.values() {
            assert_eq!(v.reference, json!({"kind": "redacted"}));
            assert!(v.logical_name.starts_with("secret-"));
        }
    }

    #[test]
    fn redact_is_stable_across_calls() {
        let mut refs = BTreeMap::new();
        refs.insert("b".to_string(), sref("b"));
        refs.insert("a".to_string(), sref("a"));
        refs.insert("c".to_string(), sref("c"));

        let out1 = render_config(BTreeMap::new(), refs.clone(), true);
        let out2 = render_config(BTreeMap::new(), refs, true);
        assert_eq!(out1.secret_references, out2.secret_references);
    }

    #[test]
    fn rendered_manifest_serializes_as_camel_case() {
        let m = RenderedManifest {
            api_version: "aeterna.io/v1".into(),
            kind: "TenantManifest".into(),
            metadata: RenderedMetadata {
                generation: 3,
                manifest_hash: Some("sha256:abc".into()),
            },
            tenant: RenderedTenant {
                id: "id".into(),
                slug: "acme".into(),
                name: "Acme".into(),
                status: "active".into(),
                source_owner: "admin".into(),
                created_at: 1,
                updated_at: 2,
            },
            config: None,
            repository: None,
            not_rendered: NOT_RENDERED_SECTIONS.to_vec(),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["apiVersion"], "aeterna.io/v1");
        assert_eq!(v["kind"], "TenantManifest");
        assert_eq!(v["metadata"]["generation"], 3);
        assert_eq!(v["metadata"]["manifestHash"], "sha256:abc");
        assert_eq!(v["tenant"]["sourceOwner"], "admin");
        assert!(v["notRendered"].is_array());
        assert!(
            v["notRendered"]
                .as_array()
                .unwrap()
                .contains(&json!("providers"))
        );
    }

    #[test]
    fn rendered_manifest_omits_absent_optional_sections() {
        let m = RenderedManifest {
            api_version: "aeterna.io/v1".into(),
            kind: "TenantManifest".into(),
            metadata: RenderedMetadata {
                generation: 0,
                manifest_hash: None,
            },
            tenant: RenderedTenant {
                id: "id".into(),
                slug: "acme".into(),
                name: "Acme".into(),
                status: "active".into(),
                source_owner: "admin".into(),
                created_at: 1,
                updated_at: 2,
            },
            config: None,
            repository: None,
            not_rendered: vec![],
        };
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("config").is_none());
        assert!(v.get("repository").is_none());
        assert!(v["metadata"].get("manifestHash").is_none());
    }

    #[test]
    fn not_rendered_list_contains_every_expected_section() {
        for section in [
            "hierarchy",
            "roles",
            "providers",
            "secrets",
            "domainMappings",
        ] {
            assert!(
                NOT_RENDERED_SECTIONS.contains(&section),
                "section {section} missing from NOT_RENDERED"
            );
        }
        for section in RENDERED_SECTIONS {
            assert!(
                !NOT_RENDERED_SECTIONS.contains(section),
                "{section} is in both RENDERED and NOT_RENDERED"
            );
        }
    }
}
