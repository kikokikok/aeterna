## Why

Aeterna currently initializes LLM and embedding services once at server boot from environment variables. Every tenant in the instance shares the same provider, model, and credentials. This singleton architecture blocks three enterprise requirements:

1. **Multi-provider tenancy**: Different tenants have different compliance, cost, and latency requirements. A healthcare tenant may require AWS Bedrock in a HIPAA-eligible region while a general-purpose tenant uses OpenAI. Today this requires separate Aeterna instances per provider configuration.
2. **Credential isolation**: All tenants share a single API key. A compromised or rate-limited key affects every tenant. Per-tenant keys enable independent rotation, cost attribution, and blast-radius containment.
3. **Model flexibility**: Tenants need to select models appropriate to their workload — smaller/cheaper models for high-volume low-complexity tasks, larger models for reasoning-heavy workflows — without affecting other tenants.

The infrastructure to support per-tenant configuration already exists: `TenantConfigProvider` with `get_config()`, `get_secret_value()`, and `set_secret_entry()`; `TenantConfigDocument` with typed fields and secret references; `TenantManifest` with config and secrets declarations; and Kubernetes-backed secret storage. The LLM and embedding factory functions already accept config structs (`LlmProviderConfig`, `EmbeddingProviderConfig`). The missing piece is the resolution layer that reads tenant config, builds provider configs, and caches the resulting service instances.

## What Changes

- Define a new `tenant-provider-config` capability specifying well-known tenant config field names for LLM and embedding provider selection, per-provider settings, and secret references for API keys.
- Implement `TenantLlmServiceRegistry` and `TenantEmbeddingServiceRegistry` backed by `DashMap<TenantId, Arc<dyn Service>>` for lazy-init caching of per-tenant service instances with TTL-based invalidation on config change.
- Implement config-to-provider resolution that reads tenant config fields and secrets via `TenantConfigProvider`, constructs `LlmProviderConfig` / `EmbeddingProviderConfig`, and calls the existing factory functions.
- Wire the registries into `AppState` and `MemoryManager` so that memory operations resolve the tenant's provider at request time instead of using the boot-time singleton.
- Add REST API endpoints for tenant provider configuration management (get/set provider config, validate credentials, view active provider status).
- Add Admin UI settings page for per-tenant provider configuration with provider selection, model configuration, and credential management.
- Enforce embedding dimension consistency: warn or block when a tenant attempts to change embedding model after vectors have been stored (different models produce different dimensions).

## Capabilities

### New Capabilities
- `tenant-provider-config`: Well-known config field names for LLM and embedding providers, tenant service registries with DashMap caching, config-to-provider resolution, platform fallback semantics, embedding dimension safety, credential validation, and Admin UI configuration surface.

### Modified Capabilities
- `memory-system`: Add tenant-aware provider resolution in memory operations — MemoryManager resolves LLM and embedding services per-tenant instead of using boot-time singletons.
- `model-provider-runtime`: Add tenant-scoped provider lifecycle — factory functions are invoked per-tenant from cached config rather than once at startup from environment variables.

## Impact

- Affected code: `memory/src/llm/factory.rs` and `memory/src/embedding/factory.rs` (new `from_tenant_config()` constructors), `memory/src/manager.rs` (tenant-aware service resolution), `cli/src/server/bootstrap.rs` (registry initialization and AppState wiring), `cli/src/server/router.rs` (new admin API routes), `admin-ui/` (new settings page).
- Affected APIs: New `/api/v1/admin/tenants/{tenant_id}/provider-config` endpoint family under the existing authenticated admin API surface.
- Affected systems: TenantConfigProvider (new well-known field names), Kubernetes secrets (per-tenant API keys), MemoryManager (per-request provider resolution), embedding storage (dimension consistency checks).
