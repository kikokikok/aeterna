## ADDED Requirements
### Requirement: Iceberg Catalog Storage
The system SHALL persist DuckDB Knowledge Graph nodes and edges to an Apache Iceberg table in object storage.

#### Scenario: Transactional Write
- **WHEN** multiple memory modifications occur within a session
- **THEN** the graph updates must be committed atomically to the Iceberg catalog
- **AND** the snapshot must be recoverable via time-travel queries

### Requirement: Cross-Tenant Data Deletion (GDPR)
The system MUST support cascading soft-deletes across vector databases, relational states, and DuckDB Iceberg tables for a specific tenant or user.

#### Scenario: User Data Wipe
- **WHEN** a tenant initiates a data deletion request for user X
- **THEN** all graph nodes, edges, vectors, and relational states owned by user X are marked as deleted and cascade.