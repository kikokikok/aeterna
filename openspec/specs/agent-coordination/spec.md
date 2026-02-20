# agent-coordination Specification

## Purpose
TBD - created by archiving change refactor-enterprise-architecture. Update Purpose after archive.
## Requirements
### Requirement: A2A Thread Persistence
The Agent-to-Agent (Radkit) module SHALL persist conversational thread states to PostgreSQL to guarantee conversation continuity.

#### Scenario: Pod Restart Mid-Conversation
- **WHEN** an Aeterna agent runner pod is restarted during an active A2A thread
- **THEN** the new pod must resume the thread seamlessly from the PostgreSQL persistence layer
- **AND** no messages must be dropped or duplicated

