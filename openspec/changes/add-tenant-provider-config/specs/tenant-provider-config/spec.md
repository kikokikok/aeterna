---
title: Tenant Provider Configuration Specification
status: draft
version: 0.1.0
created: 2026-04-12
authors:
  - AI Systems Architecture Team
related:
  - model-provider-runtime
  - memory-system
  - tenant-config-provider
  - tenant-provisioning
  - storage
---

## Purpose

The Tenant Provider Configuration capability enables per-tenant override of LLM and embedding provider settings in a multi-tenant Aeterna deployment. It bridges the existing tenant config infrastructure (`TenantConfigProvider`, `TenantConfigDocument`, Kubernetes-backed secrets) to the existing provider factory system (`create_llm_service`, `create_embedding_service`) through a caching service registry that lazily initializes and caches provider instances per tenant with TTL-based invalidation. Tenants without provider configuration automatically fall back to the platform default provider configured via environment variables.

## Requirements

### Requirement: Well-Known Config Field Names
The system SHALL define standardized config field names for per-tenant LLM and embedding provider configuration.

#### Scenario: LLM provider config fields
- **WHEN** a tenant's `TenantConfigDocument` contains the field `llm_provider`
- **THEN** the system SHALL interpret it as the tenant's LLM provider selection
- **AND** the valid values SHALL be `openai`, `google`, `bedrock`, and `none`
- **AND** the system SHALL recognize provider-specific fields: `llm_model` (all providers), `llm_google_project_id` and `llm_google_location` (Google provider), `llm_bedrock_region` (Bedrock provider)

#### Scenario: Embedding provider config fields
- **WHEN** a tenant's `TenantConfigDocument` contains the field `embedding_provider`
- **THEN** the system SHALL interpret it as the tenant's embedding provider selection
- **AND** the valid values SHALL be `openai`, `google`, `bedrock`, and `none`
- **AND** the system SHALL recognize provider-specific fields: `embedding_model` (all providers), `embedding_google_project_id` and `embedding_google_location` (Google provider), `embedding_bedrock_region` (Bedrock provider)

#### Scenario: API key secret references
- **WHEN** a tenant's `TenantConfigDocument` contains a secret reference with logical name `llm_api_key`
- **THEN** the system SHALL resolve it via `TenantConfigProvider.get_secret_value()` during provider construction
- **AND** the system SHALL support `embedding_api_key` as the logical name for embedding provider API keys
- **AND** API key values SHALL NOT be stored in config fields, only in secret references

#### Scenario: Missing provider fields indicate platform default
- **WHEN** a tenant's `TenantConfigDocument` does not contain the `llm_provider` field
- **THEN** the system SHALL use the platform default LLM service for that tenant
- **AND** the same fallback SHALL apply to embedding when `embedding_provider` is absent

### Requirement: Config Field Validation
The system SHALL validate well-known config field values before persisting them.

#### Scenario: Validate provider type values
- **WHEN** a tenant config update sets `llm_provider` or `embedding_provider`
- **THEN** the system SHALL validate the value is one of the recognized provider type strings
- **AND** the system SHALL reject unrecognized provider values with a descriptive error

#### Scenario: Validate provider-specific required fields
- **WHEN** a tenant config sets `llm_provider` to `google`
- **THEN** the system SHALL require `llm_google_project_id` and `llm_google_location` to be present and non-empty
- **AND** the system SHALL reject the configuration if required fields are missing
- **AND** the same validation SHALL apply to Bedrock (`llm_bedrock_region` required) and embedding equivalents

#### Scenario: Validate model field is non-empty
- **WHEN** a tenant config sets a provider other than `none`
- **THEN** the system SHALL require the corresponding model field (`llm_model` or `embedding_model`) to be present and non-empty
- **AND** the system SHALL reject the configuration if the model field is missing or empty

### Requirement: Tenant Service Registry with Caching
The system SHALL maintain per-tenant caches of LLM and embedding service instances with lazy initialization and TTL-based invalidation.

#### Scenario: Cache miss triggers lazy initialization
- **WHEN** a memory operation requests the LLM or embedding service for a tenant that is not in the cache
- **THEN** the system SHALL read the tenant's config document, resolve secrets, construct a provider config, and call the factory function to create a new service instance
- **AND** the system SHALL cache the resulting service keyed by tenant ID

#### Scenario: Cache hit returns existing service
- **WHEN** a memory operation requests the LLM or embedding service for a tenant that is in the cache and within the TTL window
- **THEN** the system SHALL return the cached service instance without re-reading config or re-constructing the service
- **AND** the returned instance SHALL be the same `Arc` as previously cached

#### Scenario: TTL expiry triggers re-initialization
- **WHEN** a cache entry's age exceeds the configured TTL (default 5 minutes)
- **THEN** the system SHALL treat the next access as a cache miss
- **AND** the system SHALL re-read the tenant's config, resolve secrets, and reconstruct the service
- **AND** the TTL SHALL be configurable via environment variable

#### Scenario: Explicit invalidation on config change
- **WHEN** a tenant's provider config is updated via the admin API
- **THEN** the system SHALL immediately evict the tenant's cached LLM and embedding services
- **AND** the next memory operation for that tenant SHALL trigger a fresh resolution and construction

#### Scenario: Concurrent cache miss for the same tenant
- **WHEN** multiple concurrent requests trigger a cache miss for the same tenant simultaneously
- **THEN** the system SHALL ensure at least one resolution succeeds and is cached
- **AND** the system SHALL NOT deadlock or block unrelated tenants during resolution

### Requirement: Platform Default Fallback
The system SHALL fall back to the platform default provider when a tenant has no provider configuration.

#### Scenario: Unconfigured tenant uses platform default
- **WHEN** a tenant has no `llm_provider` or `embedding_provider` fields in their config document
- **THEN** the system SHALL return the platform default LLM and embedding services (constructed from environment variables at boot time)
- **AND** the behavior SHALL be identical to the pre-change singleton model for unconfigured tenants

#### Scenario: Tenant with provider set to none
- **WHEN** a tenant's config sets `llm_provider` to `none`
- **THEN** the system SHALL return `None` for that tenant's LLM service
- **AND** memory operations that require an LLM service SHALL fail with a descriptive error for that tenant
- **AND** the same SHALL apply when `embedding_provider` is set to `none`

#### Scenario: Platform default is used when tenant config resolution fails
- **WHEN** a tenant's config contains provider fields but resolution fails (e.g., missing secret, invalid field)
- **THEN** the system SHALL NOT fall back to the platform default silently
- **AND** the system SHALL return the resolution error to the caller
- **AND** the failed resolution SHALL NOT be cached

### Requirement: Embedding Dimension Safety
The system SHALL protect against silent data corruption caused by embedding model changes.

#### Scenario: Warn on embedding model change with existing vectors
- **WHEN** a tenant config update changes `embedding_provider` or `embedding_model` and the tenant has existing embedding vectors in the vector store
- **THEN** the system SHALL return a 409 Conflict response with a warning explaining that existing vectors will become incompatible with the new model's output dimensions
- **AND** the system SHALL require an explicit `force: true` parameter to proceed with the change

#### Scenario: Allow embedding model change with no existing vectors
- **WHEN** a tenant config update changes `embedding_provider` or `embedding_model` and the tenant has no existing embedding vectors
- **THEN** the system SHALL allow the change without requiring `force`

#### Scenario: Allow embedding model change with force flag
- **WHEN** a tenant config update changes `embedding_provider` or `embedding_model` with `force: true` and the tenant has existing vectors
- **THEN** the system SHALL persist the change and log a warning including the old and new model names
- **AND** the system SHALL NOT automatically re-embed existing vectors

### Requirement: Tenant Provider Config API
The system SHALL provide REST API endpoints for managing per-tenant provider configuration.

#### Scenario: Read tenant provider config
- **WHEN** an authorized admin requests the provider config for a tenant
- **THEN** the system SHALL return the tenant's current LLM and embedding provider settings (type, model, provider-specific fields)
- **AND** the response SHALL NOT include API key values or secret material
- **AND** the response SHALL indicate whether the tenant is using an override or platform default

#### Scenario: Update tenant provider config
- **WHEN** an authorized admin submits a provider config update for a tenant
- **THEN** the system SHALL validate the config fields, persist them to the tenant's config document, and invalidate the cached services
- **AND** the system SHALL return the updated config in the response

#### Scenario: Set tenant provider secrets
- **WHEN** an authorized admin submits API key values for a tenant
- **THEN** the system SHALL store them via `set_secret_entry()` with the well-known logical names
- **AND** the system SHALL invalidate cached services so the new keys are picked up

#### Scenario: Validate tenant provider credentials
- **WHEN** an authorized admin requests credential validation for a tenant
- **THEN** the system SHALL construct the provider from the tenant's current config and perform a lightweight API call to verify the credentials work
- **AND** the system SHALL return success or failure with a descriptive message

#### Scenario: Delete tenant provider config
- **WHEN** an authorized admin deletes the provider config for a tenant
- **THEN** the system SHALL remove the provider-related config fields and secret references
- **AND** the system SHALL invalidate cached services, reverting the tenant to platform defaults

#### Scenario: Authorization for provider config endpoints
- **WHEN** a user without TenantAdmin or PlatformAdmin role attempts to access provider config endpoints
- **THEN** the system SHALL reject the request with an authorization error
- **AND** the system SHALL NOT expose any provider configuration details

### Requirement: Tenant Manifest Declaration
The system SHALL support declaring per-tenant provider configuration in the tenant provisioning manifest.

#### Scenario: Provision tenant with provider config
- **WHEN** a tenant provisioning manifest includes provider config fields and secrets
- **THEN** the provisioning flow SHALL write the config fields to the tenant's config document and store API keys as secrets via the existing provisioning pipeline
- **AND** the well-known field names in the manifest SHALL match the config field names used by the resolution layer

#### Scenario: Provision tenant without provider config
- **WHEN** a tenant provisioning manifest does not include provider config fields
- **THEN** the provisioned tenant SHALL use the platform default provider
- **AND** provider config can be added later via the admin API
