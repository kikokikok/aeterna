## ADDED Requirements

### Requirement: Tenant-Scoped Provider Lifecycle
The provider factory system SHALL support constructing service instances from per-tenant configuration in addition to environment variables.

#### Scenario: Construct LLM service from tenant config
- **WHEN** the tenant service registry needs to create an LLM service for a tenant with provider config
- **THEN** the factory SHALL accept a `LlmProviderConfig` constructed from tenant config fields and resolved secrets
- **AND** the resulting service instance SHALL be functionally identical to one constructed from environment variables with the same settings

#### Scenario: Construct embedding service from tenant config
- **WHEN** the tenant service registry needs to create an embedding service for a tenant with provider config
- **THEN** the factory SHALL accept an `EmbeddingProviderConfig` constructed from tenant config fields and resolved secrets
- **AND** the resulting service instance SHALL be functionally identical to one constructed from environment variables with the same settings

#### Scenario: Feature flag validation during tenant construction
- **WHEN** a tenant's config specifies a provider that requires a Cargo feature flag (e.g., `google-provider`, `bedrock-provider`)
- **THEN** the factory SHALL check the feature flag at construction time
- **AND** the factory SHALL return a descriptive error if the feature is not enabled in the current build
- **AND** the error SHALL identify both the provider and the required feature flag

#### Scenario: Config struct construction from non-environment sources
- **WHEN** provider config structs (`OpenAiLlmConfig`, `GoogleLlmConfig`, `BedrockLlmConfig`, and their embedding equivalents) are constructed from tenant config values
- **THEN** the structs SHALL accept values directly via constructors or struct initialization without requiring environment variables
- **AND** the `from_env()` methods SHALL remain available for platform default construction

### Requirement: Multiple Concurrent Provider Instances
The provider runtime SHALL support multiple simultaneous service instances with different configurations.

#### Scenario: Independent tenant service instances
- **WHEN** two tenants have different provider configurations (e.g., tenant A uses OpenAI with gpt-4o, tenant B uses OpenAI with gpt-4o-mini)
- **THEN** the system SHALL maintain independent service instances for each tenant
- **AND** operations for tenant A SHALL NOT be affected by tenant B's configuration or service state

#### Scenario: Shared platform default instance
- **WHEN** multiple tenants without provider overrides use the platform default service
- **THEN** the system SHALL share a single platform default service instance across all unconfigured tenants
- **AND** the shared instance SHALL be the same `Arc` to minimize resource usage
