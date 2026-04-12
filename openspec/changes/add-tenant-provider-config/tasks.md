## 1. Tenant provider config types and well-known field names

- [ ] 1.1 Define well-known config field name constants in a `tenant_provider_fields` module: `LLM_PROVIDER`, `LLM_MODEL`, `LLM_GOOGLE_PROJECT_ID`, `LLM_GOOGLE_LOCATION`, `LLM_BEDROCK_REGION`, `EMBEDDING_PROVIDER`, `EMBEDDING_MODEL`, `EMBEDDING_GOOGLE_PROJECT_ID`, `EMBEDDING_GOOGLE_LOCATION`, `EMBEDDING_BEDROCK_REGION`.
- [ ] 1.2 Define well-known secret logical names: `LLM_API_KEY`, `EMBEDDING_API_KEY`.
- [ ] 1.3 Implement `TenantLlmProviderConfig` struct that mirrors `LlmProviderConfig` but is constructed from `TenantConfigDocument` fields and resolved secrets rather than environment variables.
- [ ] 1.4 Implement `TenantEmbeddingProviderConfig` struct that mirrors `EmbeddingProviderConfig` but is constructed from `TenantConfigDocument` fields and resolved secrets.
- [ ] 1.5 Implement `resolve_llm_config(config_doc: &TenantConfigDocument, secret_resolver: &dyn TenantConfigProvider) -> Result<Option<LlmProviderConfig>>` that reads well-known fields, resolves API key secrets, and returns `None` if no LLM provider fields are present.
- [ ] 1.6 Implement `resolve_embedding_config(config_doc: &TenantConfigDocument, secret_resolver: &dyn TenantConfigProvider) -> Result<Option<EmbeddingProviderConfig>>` with the same pattern.
- [ ] 1.7 Add `from_tenant_config()` constructors on `OpenAiLlmConfig`, `GoogleLlmConfig`, `BedrockLlmConfig` and their embedding counterparts that accept field values directly rather than reading env vars.
- [ ] 1.8 Add validation for well-known field values: `llm_provider` and `embedding_provider` must be valid `LlmProviderType` / `EmbeddingProviderType` strings; model names must be non-empty; Google project/location must be non-empty when provider is google; Bedrock region must be non-empty when provider is bedrock.

## 2. Tenant service registries with DashMap cache

- [ ] 2.1 Add `dashmap` dependency to the memory crate `Cargo.toml`.
- [ ] 2.2 Implement `CachedService<T>` wrapper struct containing `service: Arc<T>`, `created_at: Instant`, and `config_hash: u64` (hash of the config fields that produced this service, for change detection).
- [ ] 2.3 Implement `TenantLlmServiceRegistry` with `DashMap<TenantId, CachedService<dyn LlmService>>`, a `platform_default: Option<Arc<dyn LlmService>>` fallback, a configurable `ttl: Duration` (default 5 minutes), and a reference to the `TenantConfigProvider`.
- [ ] 2.4 Implement `TenantLlmServiceRegistry::get_or_init(&self, tenant_id: &TenantId) -> Result<Option<Arc<dyn LlmService>>>` that checks cache (with TTL validation), falls back to resolution + factory on miss, falls back to platform default if tenant has no config, and caches the result.
- [ ] 2.5 Implement `TenantLlmServiceRegistry::invalidate(&self, tenant_id: &TenantId)` to remove a specific tenant's cached service on config change.
- [ ] 2.6 Implement `TenantLlmServiceRegistry::invalidate_all(&self)` for bulk cache clear (e.g., platform config reload).
- [ ] 2.7 Implement `TenantEmbeddingServiceRegistry` with identical structure and methods for `Arc<dyn EmbeddingService>`.
- [ ] 2.8 Implement `TenantEmbeddingServiceRegistry::get_or_init()` with the same cache-check, resolution, factory, and fallback pattern.
- [ ] 2.9 Implement TTL-based eviction: on `get_or_init`, if `Instant::now() - cached.created_at > ttl`, evict and reconstruct.
- [ ] 2.10 Add config hash comparison: on `get_or_init`, if the tenant's current config hash differs from the cached config hash, evict and reconstruct (catches config changes within the TTL window when combined with event-driven invalidation).

## 3. Config-to-provider resolution

- [ ] 3.1 Implement `TenantProviderResolver` struct holding a reference to `Arc<dyn TenantConfigProvider>` that encapsulates the full resolution pipeline: read config doc, extract well-known fields, resolve secrets, validate, build provider config.
- [ ] 3.2 Implement `TenantProviderResolver::resolve_llm(&self, tenant_id: &TenantId) -> Result<Option<LlmProviderConfig>>`: fetch `TenantConfigDocument`, check for `llm_provider` field, resolve remaining fields based on provider type, resolve `llm_api_key` secret if needed (OpenAI), return `None` if no provider fields present.
- [ ] 3.3 Implement `TenantProviderResolver::resolve_embedding(&self, tenant_id: &TenantId) -> Result<Option<EmbeddingProviderConfig>>` with the same pattern for embedding fields and secrets.
- [ ] 3.4 Implement config field extraction helpers: `extract_string_field(doc, field_name) -> Option<String>` that reads a `TenantConfigField` and extracts its string value.
- [ ] 3.5 Implement secret resolution helper: `resolve_secret(doc, provider, secret_logical_name) -> Result<String>` that reads the `TenantSecretReference` for the logical name and calls `provider.get_secret_value()`.
- [ ] 3.6 Implement config hash computation: `compute_provider_config_hash(doc: &TenantConfigDocument, prefix: &str) -> u64` that hashes all well-known fields with the given prefix (llm_ or embedding_) to detect config changes.
- [ ] 3.7 Add feature flag checking in the resolver: before returning a config for a provider, verify the corresponding Cargo feature (`embedding-integration`, `google-provider`, `bedrock-provider`) is enabled in the build, and return a descriptive error if not.

## 4. Wire registries into AppState and MemoryManager

- [ ] 4.1 Add `tenant_llm_registry: Arc<TenantLlmServiceRegistry>` and `tenant_embedding_registry: Arc<TenantEmbeddingServiceRegistry>` fields to `AppState` in `cli/src/server/mod.rs`.
- [ ] 4.2 Initialize both registries in `bootstrap.rs` with the platform default services (from `create_llm_service_from_env()` and `create_embedding_service_from_env()`), the `TenantConfigProvider`, and the configured TTL.
- [ ] 4.3 Add `with_tenant_llm_registry()` and `with_tenant_embedding_registry()` builder methods to `MemoryManager`.
- [ ] 4.4 Modify `MemoryManager` memory operation methods (add, search, delete) to resolve the tenant's LLM and embedding services from the registries using the `TenantContext.tenant_id` instead of using the singleton services, falling back to the singleton if no registry is configured (backward compatibility for non-server contexts).
- [ ] 4.5 Pass the registries from `AppState` to `MemoryManager` during server construction in `bootstrap.rs`.
- [ ] 4.6 Ensure `McpServer` and `tools/src/server.rs` propagate `TenantContext` to `MemoryManager` calls so that tenant-aware resolution works through the MCP tool interface.

## 5. API endpoints for tenant provider config management

- [ ] 5.1 Add `GET /api/v1/admin/tenants/{tenant_id}/provider-config` endpoint that returns the tenant's current provider configuration (LLM + embedding provider type, model, non-secret settings) without exposing API key material.
- [ ] 5.2 Add `PUT /api/v1/admin/tenants/{tenant_id}/provider-config` endpoint that accepts a provider config payload (provider type, model, provider-specific settings), validates the fields, writes them to `TenantConfigDocument` via `upsert_config()`, and invalidates the cached services for the tenant.
- [ ] 5.3 Add `PUT /api/v1/admin/tenants/{tenant_id}/provider-config/secrets` endpoint that accepts API key values, stores them via `set_secret_entry()`, and invalidates cached services.
- [ ] 5.4 Add `POST /api/v1/admin/tenants/{tenant_id}/provider-config/validate` endpoint that constructs the provider from the tenant's current config and performs a lightweight validation call (e.g., list models for OpenAI, describe model for Bedrock) to verify credentials work.
- [ ] 5.5 Add `GET /api/v1/admin/tenants/{tenant_id}/provider-config/status` endpoint that returns the active provider status: which provider is active (tenant override or platform default), cache age, last validation result.
- [ ] 5.6 Add `DELETE /api/v1/admin/tenants/{tenant_id}/provider-config` endpoint that removes tenant provider config fields and secrets, reverting the tenant to platform defaults, and invalidates cached services.
- [ ] 5.7 Wire all provider config routes into the admin router in `cli/src/server/router.rs` with TenantAdmin or PlatformAdmin role authorization.
- [ ] 5.8 Add embedding dimension safety check to the `PUT` endpoint: when `embedding_provider` or `embedding_model` changes and the tenant has existing embeddings, return a 409 Conflict with a warning unless `force: true` is provided in the request body.

## 6. Admin UI settings page for provider config

- [ ] 6.1 Add a "Provider Configuration" page to the Admin UI tenant settings section with LLM and Embedding provider configuration panels.
- [ ] 6.2 Implement provider type selector (dropdown: Platform Default, OpenAI, Google Vertex AI, AWS Bedrock) that shows/hides provider-specific fields based on selection.
- [ ] 6.3 Implement model name input field with provider-appropriate placeholders (e.g., "gpt-4o" for OpenAI, "gemini-2.5-flash" for Google, "anthropic.claude-3-5-haiku-20241022-v1:0" for Bedrock).
- [ ] 6.4 Implement provider-specific settings fields: Google (Project ID, Location), Bedrock (Region).
- [ ] 6.5 Implement API key input with masked display, "Set New Key" / "Clear Key" actions that call the secrets endpoint.
- [ ] 6.6 Implement "Validate Credentials" button that calls the validation endpoint and displays success/failure.
- [ ] 6.7 Implement embedding model change confirmation dialog: when the user changes the embedding provider or model and the tenant has existing embeddings, display a warning dialog explaining dimension incompatibility and requiring explicit confirmation.
- [ ] 6.8 Implement provider status display showing current active provider (override or platform default), cache age, and last validation timestamp.
- [ ] 6.9 Add "Revert to Platform Default" button that calls the DELETE endpoint and confirms the action.

## 7. Tests

- [ ] 7.1 Add unit tests for well-known field name validation: valid provider strings parse correctly, invalid strings produce descriptive errors, empty/missing fields return None from resolution.
- [ ] 7.2 Add unit tests for `resolve_llm_config()`: OpenAI config resolves model + API key from fields and secrets, Google config resolves project/location/model from fields, Bedrock config resolves region/model from fields, missing provider field returns None, missing required sub-fields produce errors.
- [ ] 7.3 Add unit tests for `resolve_embedding_config()` with the same coverage as LLM resolution.
- [ ] 7.4 Add unit tests for `TenantLlmServiceRegistry`: cache hit returns same Arc instance, cache miss triggers resolution and caches result, TTL expiry causes re-resolution, `invalidate()` forces re-resolution on next call, platform fallback returned when tenant has no config, platform fallback returned when resolution returns None.
- [ ] 7.5 Add unit tests for `TenantEmbeddingServiceRegistry` with the same coverage.
- [ ] 7.6 Add unit tests for config hash computation: identical configs produce same hash, changed model produces different hash, changed API key reference produces different hash, unrelated field changes do not affect provider config hash.
- [ ] 7.7 Add unit tests for embedding dimension safety: changing embedding model with existing vectors returns 409, changing embedding model with no existing vectors succeeds, changing embedding model with `force: true` succeeds with warning, changing non-model embedding fields (like location) does not trigger dimension check.
- [ ] 7.8 Add integration tests for end-to-end tenant provider lifecycle: provision tenant with default provider, set per-tenant OpenAI config with API key, verify memory operations use tenant's provider, update model, verify new model is used after cache invalidation, delete tenant config, verify fallback to platform default.
- [ ] 7.9 Add integration tests for concurrent registry access: multiple threads calling `get_or_init()` for the same tenant simultaneously all receive a valid service, at most one resolution call is made per cache miss cycle.
- [ ] 7.10 Add property-based tests (proptest) for config resolution: arbitrary valid config documents produce valid provider configs, arbitrary invalid field values produce errors (not panics), config hash is deterministic (same input always produces same hash).
- [ ] 7.11 Add API endpoint tests: GET returns current config without secrets, PUT validates and persists config, PUT with dimension conflict returns 409, DELETE clears config and reverts to default, unauthorized requests are rejected.
