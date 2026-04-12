## Context

Aeterna's LLM and embedding services are initialized once during server bootstrap in `cli/src/server/bootstrap.rs`. The functions `create_llm_service_from_env()` and `create_embedding_service_from_env()` read environment variables (`AETERNA_LLM_PROVIDER`, `OPENAI_API_KEY`, `AETERNA_GOOGLE_PROJECT_ID`, etc.), construct a single `Arc<dyn LlmService>` and `Arc<dyn EmbeddingService>`, and inject them into `MemoryManager`. Every tenant in the instance shares these singletons.

The factory infrastructure already supports config-driven construction: `create_llm_service(config: LlmProviderConfig)` and `create_embedding_service(config: EmbeddingProviderConfig)` accept config structs with provider type, model, and credentials. The gap is that these configs are only built from environment variables — there is no path from tenant config to provider config.

The tenant config system is mature: `TenantConfigProvider` (trait with `get_config()`, `get_secret_value()`, `set_secret_entry()`, `list_configs()`), `KubernetesTenantConfigProvider` (production implementation backed by ConfigMaps and Secrets), `TenantConfigDocument` (typed fields + secret references), and `TenantManifest` (declaration surface for provisioning). API keys are stored as Kubernetes secrets and accessed via `get_secret_value()`.

This change bridges tenant config to provider construction: read well-known config fields, resolve secrets, build provider configs, call existing factories, and cache the resulting services per tenant.

## Goals / Non-Goals

**Goals:**
- Define standardized well-known config field names for all supported LLM and embedding providers (OpenAI, Google Vertex AI, AWS Bedrock).
- Implement per-tenant service registries that cache `Arc<dyn LlmService>` and `Arc<dyn EmbeddingService>` instances, lazily initialized on first request.
- Implement config-to-provider resolution that reads tenant config + secrets and produces `LlmProviderConfig` / `EmbeddingProviderConfig`.
- Wire registries into `AppState` and `MemoryManager` so memory operations resolve tenant-specific providers at request time.
- Maintain full backward compatibility: tenants without provider config use the platform default (env var singleton).
- Enforce embedding dimension safety: prevent silent data corruption when switching embedding models mid-tenant.
- Provide API endpoints and Admin UI for managing per-tenant provider configuration.
- Invalidate cached services when tenant config changes (TTL-based with event-driven override).

**Non-Goals:**
- Custom/self-hosted LLM provider support (e.g., Ollama, vLLM). This change covers the three existing providers only; custom providers are a future extension.
- Per-request model selection (choosing different models for different memory operations within a single tenant). This change is per-tenant, not per-request.
- Provider health monitoring or automatic failover between providers. If a tenant's provider is down, operations fail with the provider's error.
- Cost tracking or usage metering per tenant. This is an observability concern outside the scope of provider configuration.
- Hot-reloading of platform default provider config without restart. Only tenant overrides are dynamic; platform defaults still come from env vars.

## Decisions

### Use DashMap for per-tenant service caching

**Decision:** Each registry (`TenantLlmServiceRegistry`, `TenantEmbeddingServiceRegistry`) stores cached services in a `DashMap<TenantId, CachedService<T>>` where `CachedService` wraps `Arc<T>` with a creation timestamp for TTL checking. On `get_or_init(tenant_id)`, the registry checks the cache, validates TTL, and either returns the cached service or constructs a new one via the factory.

**Why:** DashMap provides concurrent read/write access without a global write lock, which is critical because every memory operation resolves the tenant's provider. RwLock would create write contention during cache misses in high-concurrency scenarios. DashMap's sharded locking ensures that cache misses for different tenants do not block each other.

**Alternatives considered:**
- **RwLock<HashMap>**: Rejected because a cache miss requires a write lock, which blocks all concurrent reads for all tenants during service construction (which involves secret resolution and may involve network calls for credential refresh).
- **No cache (construct per request)**: Rejected because service construction involves secret resolution (potentially Kubernetes API calls) and provider initialization. Per-request construction would add unacceptable latency and API pressure.
- **Pre-warm all tenants at boot**: Rejected because it requires loading all tenant configs at startup, which does not scale and introduces a boot dependency on every tenant's provider being available.

### Define well-known config field names as string constants

**Decision:** Well-known field names are defined as string constants in a `tenant_provider_fields` module: `llm_provider`, `llm_model`, `llm_api_key` (secret), `llm_google_project_id`, `llm_google_location`, `llm_bedrock_region`, `embedding_provider`, `embedding_model`, `embedding_api_key` (secret), `embedding_google_project_id`, `embedding_google_location`, `embedding_bedrock_region`. These are documented as the standard field names that the resolution layer reads.

**Why:** String constants provide a single source of truth for field names used by the resolution layer, API validation, Admin UI, and tenant provisioning manifests. They do not require schema changes to `TenantConfigDocument` — the existing `BTreeMap<String, TenantConfigField>` already supports arbitrary field names.

**Alternatives considered:**
- **Typed config struct on TenantConfigDocument**: Rejected because it would require a schema migration and break the extensible field bag design. The field bag is intentionally generic to support future config categories without schema changes.
- **Enum-based field registry**: Considered but rejected as over-engineering. String constants with validation at the resolution layer are simpler and achieve the same goal.

### Resolve secrets via existing TenantConfigProvider.get_secret_value()

**Decision:** API keys for providers that require them (OpenAI, potentially others) are stored as tenant secrets via `set_secret_entry()` and resolved at service construction time via `get_secret_value()`. The resolution layer reads the `TenantSecretReference` for `llm_api_key` or `embedding_api_key` from the config document, then calls `get_secret_value()` to retrieve the actual key material.

**Why:** This reuses the existing secret management infrastructure without introducing a parallel secret storage mechanism. Kubernetes secrets provide encryption at rest, RBAC, and audit logging. The `TenantConfigProvider` trait abstracts the storage backend, supporting both Kubernetes (production) and in-memory (testing).

**Alternatives considered:**
- **Embed API keys directly in config fields**: Rejected because `TenantConfigDocument.contains_raw_secret_material()` explicitly guards against this pattern. Config fields are not encrypted and may be logged or exposed in API responses.
- **Separate secret provider**: Rejected because `TenantConfigProvider` already implements secret management. Adding a parallel system creates confusion about which system to use.

### Use TenantManifest as the declaration surface

**Decision:** Per-tenant provider configuration is declared in the `TenantManifest` using the existing `config.fields` and `secrets` sections. The provisioning flow writes these to `TenantConfigDocument` via the existing `upsert_config()` and `set_secret_entry()` paths.

**Why:** TenantManifest is already the standard declaration surface for tenant provisioning. Using it for provider config means operators use the same workflow for all tenant settings. No new provisioning primitives are needed.

**Alternatives considered:**
- **Separate provider config API only**: Rejected because it creates a split experience — some tenant config via manifest, some via a different API. Manifests should remain the canonical declaration.
- **Environment variable overrides per tenant**: Rejected because environment variables are process-scoped, not tenant-scoped. Per-tenant env vars would require one process per tenant.

### Platform fallback for unconfigured tenants

**Decision:** When a tenant has no provider config fields (or the fields are absent from their `TenantConfigDocument`), the registry returns the platform default service constructed from environment variables at boot time. The platform default is stored as the fallback in each registry.

**Why:** This ensures 100% backward compatibility. Existing deployments with no per-tenant config continue to work exactly as before. New tenants start with the platform default and can be upgraded to per-tenant config at any time.

**Alternatives considered:**
- **Fail if no tenant config**: Rejected because it would break all existing deployments on upgrade.
- **Copy platform config to every tenant**: Rejected because it creates unnecessary duplication and makes platform-wide provider changes require updating every tenant.

### Block embedding model changes when vectors exist

**Decision:** When a tenant attempts to change their `embedding_model` or `embedding_provider` config and the tenant already has stored embeddings, the system warns that existing vectors will become incompatible (different models produce different vector dimensions and semantic spaces). The API returns a validation warning requiring explicit `force: true` to proceed. The Admin UI displays a confirmation dialog explaining the impact.

**Why:** Silently changing the embedding model corrupts the tenant's vector search: new embeddings will be in a different dimensional space than existing ones, producing meaningless similarity scores. This is a data integrity issue, not a configuration issue.

**Alternatives considered:**
- **Automatically re-embed all existing data**: Rejected because it is prohibitively expensive (API costs + time) and would block tenant operations during re-embedding.
- **Allow silently with no warning**: Rejected because it causes silent data corruption that is difficult to diagnose.
- **Hard block with no override**: Rejected because there are legitimate scenarios for model migration (tenant accepts re-embedding cost, tenant has no existing data worth preserving).

## Risks / Trade-offs

- **[Risk] DashMap cache entry is stale after config change** — Mitigation: TTL-based invalidation (default 5 minutes) ensures config changes take effect within the TTL window. The Admin API for config updates also triggers immediate cache eviction for the affected tenant via `registry.invalidate(tenant_id)`.
- **[Risk] Secret resolution failure during service construction blocks tenant operations** — Mitigation: The resolution layer returns a clear error identifying the missing secret. The registry does not cache failed constructions, so the next request retries. The Admin UI pre-validates credentials before saving.
- **[Risk] Tenant configures a provider not enabled in the build (feature flags)** — Mitigation: The resolution layer checks feature flags before attempting construction and returns a clear error: "Provider X requires the Y feature flag to be enabled in this build."
- **[Risk] High cardinality of cached services in large deployments** — Mitigation: Each cached service is lightweight (an Arc to a struct with an HTTP client and config). For 1000 tenants this is ~1000 HTTP client instances. Services for inactive tenants are evicted by TTL. A configurable max cache size with LRU eviction can be added if needed.
- **[Risk] Race condition during concurrent cache misses for the same tenant** — Mitigation: DashMap's `entry()` API provides atomic get-or-insert semantics. The resolution function may be called more than once for the same tenant under high concurrency, but only one result is stored. The duplicate construction is wasted work but not incorrect.
- **[Risk] Embedding dimension mismatch after forced model change** — Mitigation: The system logs a warning with the old and new model names. The Admin UI displays a persistent banner for tenants with mixed-model embeddings. A future enhancement could track embedding model version per vector and re-embed in the background.

## Migration Plan

1. Define well-known config field name constants and the `TenantProviderConfig` resolution struct in `memory/src/llm/` and `memory/src/embedding/`.
2. Implement `TenantLlmServiceRegistry` and `TenantEmbeddingServiceRegistry` with DashMap caching, TTL invalidation, and platform fallback.
3. Implement config-to-provider resolution: `resolve_llm_config(tenant_config, secrets) -> LlmProviderConfig` and `resolve_embedding_config(tenant_config, secrets) -> EmbeddingProviderConfig`.
4. Wire registries into `AppState` in `bootstrap.rs` and modify `MemoryManager` to accept a registry reference instead of (or in addition to) a singleton service.
5. Add REST API endpoints for tenant provider config management behind admin auth.
6. Add Admin UI settings page for provider configuration.
7. Add comprehensive tests for resolution, caching, fallback, invalidation, and dimension safety.

## Open Questions

- Should the cache TTL be configurable per-tenant or only globally? Per-tenant TTL adds complexity with limited benefit. Starting with a global default (5 minutes) that is configurable via environment variable.
- Should we support tenant-level embedding dimension override (e.g., `embedding_dimensions` config field) for models that support configurable output dimensions (OpenAI text-embedding-3-small supports 256/512/1536)?
- Should the credential validation endpoint actually call the provider API (e.g., list models) or only validate the config structure? API validation catches invalid keys early but adds latency and external dependency to the config save flow.
- Should we emit a Prometheus metric for cache hit/miss ratio per registry to help operators tune the TTL?
