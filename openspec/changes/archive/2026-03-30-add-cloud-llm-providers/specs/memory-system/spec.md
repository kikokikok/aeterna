## MODIFIED Requirements

### Requirement: Embedding Generation
The system SHALL generate vector embeddings for memory content using the configured runtime embedding provider.

#### Scenario: Generate embedding for new content
- **WHEN** adding a memory with new content
- **THEN** the system SHALL generate vector embedding using the configured runtime embedding provider
- **AND** the system SHALL store the embedding with memory
- **AND** the system SHALL return `embeddingGenerated: true`

#### Scenario: Cache repeated embeddings
- **WHEN** adding memory with content seen before
- **THEN** the system SHALL return a cached embedding if one exists
- **AND** the system SHALL NOT call the embedding provider again

#### Scenario: Reject provider-dependent embedding operation when provider construction failed
- **WHEN** an operation requires embeddings but the configured embedding provider failed validation or construction
- **THEN** the system SHALL fail closed
- **AND** the error SHALL indicate that the configured embedding provider is unavailable

### Requirement: LLM-based Entity Extraction
The system SHALL use the configured runtime LLM provider to extract entities and relationships from memory content.

#### Scenario: Extract entities with configured runtime provider
- **WHEN** processing memory content that requires entity extraction
- **THEN** the system SHALL invoke the configured runtime LLM provider to extract named entities

#### Scenario: Extract relationships with configured runtime provider
- **WHEN** processing memory content that contains multiple named entities
- **THEN** the system SHALL invoke the configured runtime LLM provider to identify relationships between entities

#### Scenario: Fail closed when configured LLM provider is unavailable
- **WHEN** an operation requires LLM-based extraction and the configured runtime LLM provider failed validation or construction
- **THEN** the system SHALL fail closed
- **AND** the error SHALL identify that the configured LLM provider is unavailable
