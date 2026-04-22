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
pub const RENDERED_SECTIONS: &[&str] = &["tenant", "metadata", "config", "repository", "providers"];

/// Sections a full `TenantManifest` can carry but this renderer does
/// not yet cover. Reflected into `RenderedManifest::not_rendered`.
///
/// `providers.memoryLayers` is still partial (deferred to §2.2-D) —
/// the outer `providers` section is rendered for llm/embedding, but
/// `memoryLayers` has no canonical storage convention yet.
pub const NOT_RENDERED_SECTIONS: &[&str] = &[
    "domainMappings",
    "secrets",
    "hierarchy",
    "roles",
    "providers.memoryLayers",
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
    /// Reconstructed from `config.fields` + `config.secret_references`
    /// by `render_providers`. Elided entirely when no provider has been
    /// declared for this tenant (neither `llm_provider` nor
    /// `embedding_provider` field present) so the rendered manifest
    /// stays byte-identical to the input for tenants that don't use
    /// the providers block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub providers: Option<RenderedProviders>,
    pub not_rendered: Vec<&'static str>,
}

/// Mirror of `ManifestProviders` used by the input side
/// (`tenant_api::ManifestProviders`), minus `memoryLayers` which has
/// no storage convention yet (see FINDINGS-2-2 §2.2-D).
///
/// We redeclare the type here instead of reusing the input type
/// because the input type is `Deserialize`-only (no `Serialize`) and
/// changing its derive surface would ripple through validator tests.
/// The two must stay field-compatible for round-trip to work; a
/// dedicated `round_trip_manifest_providers` test in tenant_api.rs
/// locks that contract end-to-end.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedProviders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm: Option<RenderedProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<RenderedProvider>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedProvider {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The name the operator used in `config.secretReferences` when
    /// they originally declared this provider. Recovered by matching
    /// the canonical `{llm,embedding}_api_key` alias back to the
    /// operator-named entry by reference-equality on the underlying
    /// `SecretReference`. Falls back to the canonical name itself
    /// if no other entry matches (e.g. when the provider was
    /// configured via the dedicated `PUT .../providers/{llm,embedding}`
    /// handler which only writes the canonical name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    /// Provider-specific non-sensitive config (e.g. `projectId` for
    /// google, `region` for bedrock). Elided when empty so round-trip
    /// output stays minimal.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, String>,
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

    // Providers reverse-render reads the SAME fields/secret_references
    // the apply helper writes, so we compute it BEFORE handing those
    // maps to `render_config` (which consumes them). Redact mode
    // elides `secret_ref` because the redacted rendering replaces
    // operator names with opaque `secret-N` placeholders; exposing the
    // placeholder name in `secret_ref` would leak the set ordering
    // without giving the consumer anything actionable.
    let rendered_providers = config_doc
        .as_ref()
        .and_then(|doc| render_providers(&doc.fields, &doc.secret_references, redact));

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
        providers: rendered_providers,
        not_rendered: NOT_RENDERED_SECTIONS.to_vec(),
    })
}

/// Reverse of `apply_manifest_providers_to_config`. Reconstructs a
/// `RenderedProviders` block from `config.fields` +
/// `config.secret_references`.
///
/// Returns `None` when neither `llm_provider` nor `embedding_provider`
/// is present, so tenants that don't use the providers block produce
/// a manifest with no `providers:` key at all (byte-identical to the
/// input manifest on round-trip for that subset).
///
/// `secret_ref` resolution strategy: look for an entry in
/// `secret_references` whose `reference` equals the canonical
/// `{llm,embedding}_api_key` entry's reference but whose logical name
/// is NOT the canonical name. Deterministic because `BTreeMap` is
/// sorted; we take the first matching operator-named entry
/// alphabetically. Falls back to the canonical name itself when no
/// other entry matches (provider was configured via the dedicated
/// PUT handler which only writes the canonical key).
pub(crate) fn render_providers(
    fields: &BTreeMap<String, TenantConfigField>,
    secret_references: &BTreeMap<String, TenantSecretReference>,
    redact: bool,
) -> Option<RenderedProviders> {
    use memory::provider_registry::config_keys;

    let llm = render_one_provider(
        fields,
        secret_references,
        redact,
        config_keys::LLM_PROVIDER,
        config_keys::LLM_MODEL,
        config_keys::LLM_API_KEY,
        &[
            ("projectId", config_keys::LLM_GOOGLE_PROJECT_ID),
            ("location", config_keys::LLM_GOOGLE_LOCATION),
            ("region", config_keys::LLM_BEDROCK_REGION),
        ],
    );
    let embedding = render_one_provider(
        fields,
        secret_references,
        redact,
        config_keys::EMBEDDING_PROVIDER,
        config_keys::EMBEDDING_MODEL,
        config_keys::EMBEDDING_API_KEY,
        &[
            ("projectId", config_keys::EMBEDDING_GOOGLE_PROJECT_ID),
            ("location", config_keys::EMBEDDING_GOOGLE_LOCATION),
            ("region", config_keys::EMBEDDING_BEDROCK_REGION),
        ],
    );

    if llm.is_none() && embedding.is_none() {
        None
    } else {
        Some(RenderedProviders { llm, embedding })
    }
}

fn render_one_provider(
    fields: &BTreeMap<String, TenantConfigField>,
    secret_references: &BTreeMap<String, TenantSecretReference>,
    redact: bool,
    provider_key: &str,
    model_key: &str,
    api_key_key: &str,
    // (manifest-camelCase, config-snake_case) pairs for provider-specific
    // extras. We walk all of them and emit the ones whose config-side
    // key is present; it's cheap and avoids branching on kind.
    extras: &[(&str, &str)],
) -> Option<RenderedProvider> {
    let kind = field_as_string(fields, provider_key)?;
    let model = field_as_string(fields, model_key);

    let mut config = BTreeMap::new();
    for (manifest_name, storage_name) in extras {
        if let Some(v) = field_as_string(fields, storage_name) {
            config.insert((*manifest_name).to_string(), v);
        }
    }

    // secret_ref recovery: only in full mode. In redact mode we elide
    // the field entirely — the redacted renderer has already rewritten
    // operator names to `secret-N` placeholders, and surfacing one
    // here would leak nothing useful while implying a contract.
    let secret_ref = if redact {
        None
    } else {
        resolve_operator_secret_ref(secret_references, api_key_key)
    };

    Some(RenderedProvider {
        kind,
        model,
        secret_ref,
        config,
    })
}

/// Read a config field as a `String`. The apply helper writes
/// `serde_json::json!(provider.kind)` for a `String` which produces a
/// JSON string, so `as_str()` succeeds for valid apply output.
/// Returns `None` for missing fields and for fields whose value is
/// not a JSON string (defensive: a hand-edited DB row with a number
/// for `llm_model` won't crash the renderer).
fn field_as_string(fields: &BTreeMap<String, TenantConfigField>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|f| f.value.as_str())
        .map(|s| s.to_string())
}

/// Find the operator-given name for a secret whose canonical alias is
/// `canonical_key` (e.g. `llm_api_key`). Walks the map once in sorted
/// order and returns the first non-canonical entry whose `reference`
/// matches the canonical entry's. Falls back to `canonical_key` itself
/// when the canonical entry is the only one pointing at its secret.
fn resolve_operator_secret_ref(
    secret_references: &BTreeMap<String, TenantSecretReference>,
    canonical_key: &str,
) -> Option<String> {
    let canonical = secret_references.get(canonical_key)?;
    // BTreeMap iteration is sorted by key; deterministic pick.
    for (name, sref) in secret_references.iter() {
        if name == canonical_key {
            continue;
        }
        if sref.reference == canonical.reference {
            return Some(name.clone());
        }
    }
    // Fall back: provider was set via the dedicated PUT handler, not
    // via manifest.providers — there's no operator name, so emit the
    // canonical name. Round-trip through apply will re-register the
    // canonical key as its own alias, which is a fixed point.
    Some(canonical_key.to_string())
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
            providers: None,
            not_rendered: NOT_RENDERED_SECTIONS.to_vec(),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["apiVersion"], "aeterna.io/v1");
        assert_eq!(v["kind"], "TenantManifest");
        assert_eq!(v["metadata"]["generation"], 3);
        assert_eq!(v["metadata"]["manifestHash"], "sha256:abc");
        assert_eq!(v["tenant"]["sourceOwner"], "admin");
        assert!(v["notRendered"].is_array());
        // `providers` (the top-level section) is now rendered; only
        // the `memoryLayers` sub-section is still in the gap list.
        let not_rendered = v["notRendered"].as_array().unwrap();
        assert!(
            !not_rendered.contains(&json!("providers")),
            "providers should no longer be a top-level gap — \
             reverse-render landed in §2.2-A"
        );
        assert!(
            not_rendered.contains(&json!("providers.memoryLayers")),
            "memoryLayers still deferred to §2.2-D"
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
            providers: None,
            not_rendered: vec![],
        };
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("config").is_none());
        assert!(v.get("repository").is_none());
        assert!(
            v.get("providers").is_none(),
            "absent providers must be elided, not null-serialized"
        );
        assert!(v["metadata"].get("manifestHash").is_none());
    }

    #[test]
    fn not_rendered_list_contains_every_expected_section() {
        for section in [
            "hierarchy",
            "roles",
            "secrets",
            "domainMappings",
            "providers.memoryLayers",
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
        // Regression: the bare string "providers" must NOT appear in
        // NOT_RENDERED after §2.2-A. The sub-section form
        // `providers.memoryLayers` is the honest shape going forward.
        assert!(
            !NOT_RENDERED_SECTIONS.contains(&"providers"),
            "top-level `providers` must no longer be in the gap list"
        );
    }

    // ── §2.2-A render_providers unit tests ───────────────────────────────

    use memory::provider_registry::config_keys;

    fn platform_field(val: &str) -> TenantConfigField {
        TenantConfigField {
            ownership: TenantConfigOwnership::Platform,
            value: serde_json::Value::String(val.to_string()),
        }
    }

    fn postgres_ref(logical_name: &str, secret_id: u128) -> TenantSecretReference {
        TenantSecretReference {
            logical_name: logical_name.to_string(),
            ownership: TenantConfigOwnership::Tenant,
            reference: SecretReference::Postgres {
                secret_id: Uuid::from_u128(secret_id),
            },
        }
    }

    #[test]
    fn render_providers_none_when_no_provider_fields() {
        // A tenant with config fields that have nothing to do with
        // providers must produce no `providers:` block at all.
        let mut fields = BTreeMap::new();
        fields.insert("some_app_setting".to_string(), platform_field("v"));
        let out = render_providers(&fields, &BTreeMap::new(), false);
        assert!(out.is_none());
    }

    #[test]
    fn render_providers_reconstructs_llm_openai() {
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::LLM_PROVIDER.to_string(),
            platform_field("openai"),
        );
        fields.insert(config_keys::LLM_MODEL.to_string(), platform_field("gpt-4o"));
        let out = render_providers(&fields, &BTreeMap::new(), false).expect("llm should render");
        let llm = out.llm.expect("llm branch");
        assert_eq!(llm.kind, "openai");
        assert_eq!(llm.model.as_deref(), Some("gpt-4o"));
        assert!(llm.secret_ref.is_none(), "no api_key alias set");
        assert!(llm.config.is_empty());
        assert!(out.embedding.is_none());
    }

    #[test]
    fn render_providers_reconstructs_google_extras() {
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::LLM_PROVIDER.to_string(),
            platform_field("google"),
        );
        fields.insert(
            config_keys::LLM_MODEL.to_string(),
            platform_field("gemini-1.5-pro"),
        );
        fields.insert(
            config_keys::LLM_GOOGLE_PROJECT_ID.to_string(),
            platform_field("my-proj"),
        );
        fields.insert(
            config_keys::LLM_GOOGLE_LOCATION.to_string(),
            platform_field("europe-west1"),
        );
        let out = render_providers(&fields, &BTreeMap::new(), false).expect("llm should render");
        let llm = out.llm.unwrap();
        assert_eq!(llm.kind, "google");
        assert_eq!(
            llm.config.get("projectId").map(String::as_str),
            Some("my-proj")
        );
        assert_eq!(
            llm.config.get("location").map(String::as_str),
            Some("europe-west1")
        );
        assert!(
            !llm.config.contains_key("region"),
            "region is bedrock-only; google provider must not carry it"
        );
    }

    #[test]
    fn render_providers_prefers_operator_secret_name_over_canonical() {
        // When both the operator name AND the canonical alias point
        // at the same underlying secret_id (the shape
        // `apply_manifest_providers_to_config` produces), the
        // rendered `secret_ref` must be the operator's name so
        // round-trip through apply is byte-identical.
        let same_id = 0xC0FFEE;
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::LLM_PROVIDER.to_string(),
            platform_field("openai"),
        );
        let mut refs = BTreeMap::new();
        refs.insert(
            "openai_key".to_string(),
            postgres_ref("openai_key", same_id),
        );
        refs.insert(
            config_keys::LLM_API_KEY.to_string(),
            postgres_ref(config_keys::LLM_API_KEY, same_id),
        );
        let out = render_providers(&fields, &refs, false).unwrap();
        assert_eq!(out.llm.unwrap().secret_ref.as_deref(), Some("openai_key"));
    }

    #[test]
    fn render_providers_falls_back_to_canonical_when_alone() {
        // Provider was configured via `PUT .../providers/llm` only:
        // `llm_api_key` exists but no operator-named alias does.
        // Render the canonical name — re-applying is a fixed point
        // because apply will re-register llm_api_key → llm_api_key.
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::LLM_PROVIDER.to_string(),
            platform_field("openai"),
        );
        let mut refs = BTreeMap::new();
        refs.insert(
            config_keys::LLM_API_KEY.to_string(),
            postgres_ref(config_keys::LLM_API_KEY, 0xFEED),
        );
        let out = render_providers(&fields, &refs, false).unwrap();
        assert_eq!(
            out.llm.unwrap().secret_ref.as_deref(),
            Some(config_keys::LLM_API_KEY)
        );
    }

    #[test]
    fn render_providers_elides_secret_ref_in_redact_mode() {
        let same_id = 0xC0FFEE;
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::LLM_PROVIDER.to_string(),
            platform_field("openai"),
        );
        let mut refs = BTreeMap::new();
        refs.insert(
            "openai_key".to_string(),
            postgres_ref("openai_key", same_id),
        );
        refs.insert(
            config_keys::LLM_API_KEY.to_string(),
            postgres_ref(config_keys::LLM_API_KEY, same_id),
        );
        let out = render_providers(&fields, &refs, true).unwrap();
        assert!(
            out.llm.unwrap().secret_ref.is_none(),
            "redact mode must not surface operator-named secret refs"
        );
    }

    #[test]
    fn render_providers_handles_embedding_only() {
        // Edge case: tenant configured only an embedding provider,
        // no LLM. The renderer must emit a providers block with
        // llm=None and embedding=Some.
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::EMBEDDING_PROVIDER.to_string(),
            platform_field("bedrock"),
        );
        fields.insert(
            config_keys::EMBEDDING_BEDROCK_REGION.to_string(),
            platform_field("us-east-1"),
        );
        let out = render_providers(&fields, &BTreeMap::new(), false).unwrap();
        assert!(out.llm.is_none());
        let emb = out.embedding.unwrap();
        assert_eq!(emb.kind, "bedrock");
        assert_eq!(
            emb.config.get("region").map(String::as_str),
            Some("us-east-1")
        );
    }

    #[test]
    fn render_providers_defensive_ignores_non_string_field() {
        // Hand-edited DB row with a JSON number for `llm_model`
        // must not crash the renderer — field_as_string returns None,
        // model is emitted as absent, kind still surfaces.
        let mut fields = BTreeMap::new();
        fields.insert(
            config_keys::LLM_PROVIDER.to_string(),
            platform_field("openai"),
        );
        fields.insert(
            config_keys::LLM_MODEL.to_string(),
            TenantConfigField {
                ownership: TenantConfigOwnership::Platform,
                value: json!(42), // wrong shape on purpose
            },
        );
        let out = render_providers(&fields, &BTreeMap::new(), false).unwrap();
        let llm = out.llm.unwrap();
        assert_eq!(llm.kind, "openai");
        assert!(
            llm.model.is_none(),
            "non-string model value must not be silently serialized as a number"
        );
    }
}
