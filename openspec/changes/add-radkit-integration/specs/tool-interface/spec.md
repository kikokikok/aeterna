## ADDED Requirements

### Requirement: A2A Native Entry Point
The system SHALL provide an A2A-compliant entry point using Radkit to enable discovery and interaction by other agents.

#### Scenario: Agent Card Discovery
- **WHEN** a request is made to `GET /.well-known/agent.json`
- **THEN** it MUST return a valid A2A Agent Card containing:
  - Agent name, description, and version
  - List of available skills (Memory, Knowledge, Governance)
  - Supported capabilities and authentication requirements

#### Scenario: Agent Card Schema Compliance
- **WHEN** the Agent Card is retrieved
- **THEN** it MUST conform to the A2A Agent Card JSON schema specification

### Requirement: Memory Skill A2A Exposure
The system SHALL expose Memory operations as an A2A Skill with the following tools.

#### Scenario: Memory Add Tool
- **WHEN** an A2A `tasks/send` request invokes `memory_add`
- **THEN** the system MUST accept content, optional layer, and optional tags
- **AND** return a structured response with memory_id and success status

#### Scenario: Memory Search Tool
- **WHEN** an A2A `tasks/send` request invokes `memory_search`
- **THEN** the system MUST accept a natural language query and optional filters
- **AND** return ranked results with layer precedence applied

#### Scenario: Memory Delete Tool
- **WHEN** an A2A `tasks/send` request invokes `memory_delete`
- **THEN** the system MUST accept a memory_id
- **AND** return confirmation of deletion

### Requirement: Knowledge Skill A2A Exposure
The system SHALL expose Knowledge operations as an A2A Skill with the following tools.

#### Scenario: Knowledge Query Tool
- **WHEN** an A2A `tasks/send` request invokes `knowledge_query`
- **THEN** the system MUST accept a search query and optional type/layer filters
- **AND** return matching knowledge entries with summaries

#### Scenario: Knowledge Show Tool
- **WHEN** an A2A `tasks/send` request invokes `knowledge_show`
- **THEN** the system MUST accept a knowledge item ID
- **AND** return the full content including constraints and metadata

#### Scenario: Knowledge Check Tool
- **WHEN** an A2A `tasks/send` request invokes `knowledge_check`
- **THEN** the system MUST accept files and/or dependencies to check
- **AND** return policy violations with severity levels

### Requirement: Governance Skill A2A Exposure
The system SHALL expose Governance operations as an A2A Skill with the following tools.

#### Scenario: Governance Validate Tool
- **WHEN** an A2A `tasks/send` request invokes `governance_validate`
- **THEN** the system MUST accept context data and target layer
- **AND** return validation result with any policy violations

#### Scenario: Governance Drift Check Tool
- **WHEN** an A2A `tasks/send` request invokes `governance_drift_check`
- **THEN** the system MUST accept project context
- **AND** return drift score and detected violations

### Requirement: Stateful Tool Execution
The system MUST support multi-turn tool execution using Radkit's stateful task management.

#### Scenario: Multi-turn Memory Update
- **WHEN** an agent requests a memory update that requires clarification
- **THEN** the system SHALL maintain task state via A2A `tasks/{id}/send`
- **AND** return status `input-required` with a message requesting missing info

#### Scenario: Long-running Knowledge Sync
- **WHEN** a knowledge sync operation takes longer than 30 seconds
- **THEN** the system SHALL return status `working` with progress updates
- **AND** allow the caller to poll for completion via `tasks/{id}/get`

### Requirement: Multi-Tenant A2A Context
The system MUST propagate tenant context through all A2A requests.

#### Scenario: Tenant Context from Authentication
- **WHEN** an A2A request includes authentication credentials
- **THEN** the system MUST extract and propagate TenantContext to all downstream operations

#### Scenario: Tenant Isolation in A2A Responses
- **WHEN** returning A2A responses
- **THEN** the system MUST only include data accessible to the authenticated tenant

### Requirement: A2A Error Responses
The system MUST return standardized A2A error responses for all failure cases.

#### Scenario: Invalid Tool Parameters
- **WHEN** a tool is called with invalid parameters
- **THEN** the system MUST return status `failed` with error details
- **AND** include a structured error object with code and message

#### Scenario: Authorization Failure
- **WHEN** a tool is called without sufficient permissions
- **THEN** the system MUST return status `failed` with error code `UNAUTHORIZED`
