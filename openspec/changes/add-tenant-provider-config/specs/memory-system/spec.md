## MODIFIED Requirements

### Requirement: Tenant-Aware Provider Resolution in Memory Operations
The memory system SHALL resolve LLM and embedding services per-tenant at request time instead of using boot-time singletons.

#### Scenario: Memory add resolves tenant embedding service
- **WHEN** a memory add operation is invoked with a `TenantContext`
- **THEN** the `MemoryManager` SHALL resolve the embedding service for the tenant via the `TenantEmbeddingServiceRegistry`
- **AND** the resolved service SHALL be used for embedding generation for that specific operation
- **AND** if the tenant has no override, the platform default embedding service SHALL be used

#### Scenario: Memory search resolves tenant embedding service
- **WHEN** a memory search operation is invoked with a `TenantContext`
- **THEN** the `MemoryManager` SHALL resolve the embedding service for the tenant via the `TenantEmbeddingServiceRegistry`
- **AND** the query embedding SHALL be generated using the tenant's configured embedding provider and model

#### Scenario: Memory operations resolve tenant LLM service
- **WHEN** a memory operation that requires LLM processing (reasoning, summarization, drift analysis) is invoked with a `TenantContext`
- **THEN** the `MemoryManager` SHALL resolve the LLM service for the tenant via the `TenantLlmServiceRegistry`
- **AND** the resolved service SHALL be used for all LLM calls within that operation

#### Scenario: Backward compatibility without registries
- **WHEN** a `MemoryManager` is constructed without tenant registries (e.g., in non-server contexts, tests, or CLI tools)
- **THEN** the `MemoryManager` SHALL use the directly-injected singleton services as before
- **AND** all existing tests and CLI usage SHALL continue to work without modification

#### Scenario: Registry resolution failure
- **WHEN** the tenant service registry fails to resolve a service (missing config, invalid credentials, feature not enabled)
- **THEN** the `MemoryManager` SHALL propagate the error to the caller
- **AND** the error SHALL identify the tenant, the failing provider, and the reason for failure
- **AND** the `MemoryManager` SHALL NOT silently fall back to the platform default on resolution errors
