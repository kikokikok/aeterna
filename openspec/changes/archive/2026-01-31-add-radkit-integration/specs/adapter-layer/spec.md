## ADDED Requirements

### Requirement: Radkit Runtime Adapter
The system SHALL provide a Radkit Runtime adapter that serves the Memory-Knowledge system as an A2A-compliant agent.

#### Scenario: Runtime Initialization
- **WHEN** the `agent-a2a` binary starts
- **THEN** it MUST initialize MemoryManager, GitRepository, and GovernanceEngine
- **AND** compose them into Radkit Skills
- **AND** start the Radkit Runtime server

#### Scenario: Runtime Configuration
- **WHEN** the runtime starts
- **THEN** it MUST accept configuration via environment variables or config file
- **AND** support configurable bind address, port, and authentication settings

### Requirement: Skill Handler Implementation
The system SHALL implement Radkit SkillHandler trait for each domain.

#### Scenario: MemorySkill Handler
- **GIVEN** a MemoryManager instance
- **WHEN** the MemorySkill receives a tool invocation
- **THEN** it MUST delegate to the appropriate MemoryManager method
- **AND** convert the response to A2A-compatible JSON

#### Scenario: KnowledgeSkill Handler
- **GIVEN** a GitRepository instance
- **WHEN** the KnowledgeSkill receives a tool invocation
- **THEN** it MUST delegate to the appropriate KnowledgeRepository method
- **AND** handle git operations appropriately

#### Scenario: GovernanceSkill Handler
- **GIVEN** a GovernanceEngine instance
- **WHEN** the GovernanceSkill receives a tool invocation
- **THEN** it MUST delegate to the appropriate GovernanceEngine method
- **AND** propagate validation results correctly

### Requirement: A2A Protocol Compliance
The Radkit adapter MUST comply with the A2A protocol specification.

#### Scenario: Tasks Endpoint
- **WHEN** a POST request is made to `/tasks/send`
- **THEN** the system MUST accept the A2A task message format
- **AND** return an A2A-compliant task response

#### Scenario: Task Status Polling
- **WHEN** a GET request is made to `/tasks/{id}/get`
- **THEN** the system MUST return the current task status
- **AND** include any artifacts or partial results

#### Scenario: Streaming Support
- **WHEN** a client requests streaming via SSE
- **THEN** the system MUST support Server-Sent Events for task updates
- **AND** stream intermediate results as they become available

### Requirement: Agent Card Generation
The adapter MUST generate and serve a valid A2A Agent Card.

#### Scenario: Dynamic Agent Card
- **WHEN** skills are registered with the runtime
- **THEN** the Agent Card MUST be dynamically updated to reflect available tools
- **AND** include accurate descriptions and input schemas

#### Scenario: Agent Capabilities
- **WHEN** the Agent Card is requested
- **THEN** it MUST advertise the following capabilities:
  - `streaming: true` (if SSE is enabled)
  - `pushNotifications: false` (initial implementation)
  - `stateTransitionHistory: true`

### Requirement: Thread and Conversation Support
The adapter SHALL support A2A Thread management for multi-turn conversations.

#### Scenario: Thread Creation
- **WHEN** a new conversation begins
- **THEN** the system MUST create an A2A Thread
- **AND** maintain conversation context across tool invocations

#### Scenario: Thread Persistence
- **WHEN** a Thread is created
- **THEN** the system MUST persist thread state
- **AND** allow resumption of conversations via thread ID

### Requirement: Health and Observability
The Radkit adapter MUST expose health and observability endpoints.

#### Scenario: Health Check Endpoint
- **WHEN** a GET request is made to `/health`
- **THEN** the system MUST return health status of all dependencies
- **AND** include MemoryManager, GitRepository, and storage backend status

#### Scenario: Metrics Endpoint
- **WHEN** a GET request is made to `/metrics`
- **THEN** the system MUST return Prometheus-compatible metrics
- **AND** include request counts, latencies, and error rates per skill

## MODIFIED Requirements

### Requirement: Ecosystem Adapter Interface
The system SHALL support multiple ecosystem adapter implementations including A2A via Radkit.

#### Scenario: A2A as Ecosystem Adapter
- **GIVEN** the existing EcosystemAdapter interface
- **WHEN** the Radkit adapter is registered
- **THEN** it MUST conform to the EcosystemAdapter interface pattern
- **AND** generate ecosystem-native tools via getMemoryTools() and getKnowledgeTools()

#### Scenario: Session Context from A2A
- **WHEN** an A2A session begins
- **THEN** the adapter MUST call getSessionContext() to inject relevant memories
- **AND** include active constraints in the response context

### Requirement: Radkit SDK Version Stability (RAD-C1)
The system SHALL maintain stability guarantees despite underlying SDK pre-release status.

#### Scenario: SDK Version Pinning
- **WHEN** the project builds
- **THEN** the `radkit` dependency MUST be pinned to an exact version (not semver range)
- **AND** the pinned version MUST be documented in CHANGELOG on update

#### Scenario: SDK Abstraction Layer
- **WHEN** implementing Radkit integration
- **THEN** the system MUST implement an abstraction layer between business logic and Radkit SDK
- **AND** this abstraction MUST allow SDK replacement without changing business logic

#### Scenario: SDK Integration Test Suite
- **WHEN** the SDK version changes
- **THEN** integration tests MUST verify all skill handlers work correctly
- **AND** tests MUST cover Agent Card generation, task submission, and response formats

### Requirement: Thread State Persistence (RAD-C2)
The system SHALL persist A2A Thread state to survive pod restarts and enable conversation recovery.

#### Scenario: Thread Storage Backend
- **WHEN** a new A2A Thread is created
- **THEN** the system MUST persist thread state to PostgreSQL
- **AND** the state MUST include thread_id, tenant_id, created_at, updated_at, and conversation_context

#### Scenario: Thread Recovery on Startup
- **WHEN** the agent-a2a service starts
- **THEN** it MUST recover active threads from PostgreSQL
- **AND** restore conversation context for each active thread

#### Scenario: Thread Expiration
- **WHEN** a thread has been inactive for longer than configured TTL (default: 24 hours)
- **THEN** the system MUST mark the thread as expired
- **AND** return appropriate error when clients attempt to continue expired threads

### Requirement: A2A Spec Compliance Monitoring (RAD-H1)
The system SHALL maintain compliance with evolving A2A specification.

#### Scenario: Compliance Test Suite
- **WHEN** CI runs
- **THEN** the system MUST execute A2A compliance tests against official test vectors
- **AND** fail the build if any compliance tests fail

#### Scenario: Spec Version Tracking
- **WHEN** the A2A spec version changes
- **THEN** the system MUST document supported A2A spec version in Agent Card metadata
- **AND** log warning when receiving requests from newer spec versions

### Requirement: Error Mapping Completeness (RAD-H2)
The system SHALL provide exhaustive mapping from domain errors to A2A result variants.

#### Scenario: Exhaustive Error Mapping
- **WHEN** any domain error occurs during skill execution
- **THEN** the system MUST map it to an appropriate A2A error response
- **AND** include error code, message, and optional details

#### Scenario: Unmapped Error Handling
- **WHEN** an unexpected error type is encountered
- **THEN** the system MUST return status `failed` with generic error code `INTERNAL_ERROR`
- **AND** log the unmapped error with full context for debugging
- **AND** never expose internal error details to clients

### Requirement: A2A Rate Limiting (RAD-H3)
The system SHALL implement rate limiting to protect against endpoint abuse.

#### Scenario: Per-Tenant Rate Limits
- **WHEN** an A2A request is received
- **THEN** the system MUST enforce per-tenant rate limits
- **AND** return HTTP 429 with `Retry-After` header when limits exceeded

#### Scenario: Rate Limit Configuration
- **WHEN** the system starts
- **THEN** rate limits MUST be configurable per tenant tier (default: 100 req/min)
- **AND** support different limits for different skill types

#### Scenario: Rate Limit Metrics
- **WHEN** rate limiting occurs
- **THEN** the system MUST emit metrics for rate limit hits
- **AND** include tenant_id and skill_name in metric labels

### Requirement: LLM Cost Optimization (RAD-H4)
The system SHALL minimize LLM usage for operations that don't require reasoning.

#### Scenario: Direct Tool Routing
- **WHEN** an A2A request contains explicit tool invocation
- **THEN** the system MUST route directly to the skill handler without LLM involvement
- **AND** only invoke LLM for ambiguous requests requiring interpretation

#### Scenario: Minimal LLM Configuration
- **WHEN** Radkit requires LLM for routing
- **THEN** the system MUST configure the cheapest/fastest model available
- **AND** limit LLM context to skill descriptions and tool schemas only

### Requirement: Thread State Memory Management (RAD-H5)
The system SHALL prevent unbounded state growth during multi-turn conversations.

#### Scenario: State TTL Enforcement
- **WHEN** thread state is persisted
- **THEN** each state record MUST have TTL (default: 1 hour for conversation context)
- **AND** expired state MUST be automatically cleaned up

#### Scenario: Periodic State Cleanup Job
- **WHEN** the cleanup job runs (default: every 5 minutes)
- **THEN** it MUST remove expired thread states
- **AND** emit metrics for states cleaned up

#### Scenario: State Size Limits
- **WHEN** thread state grows
- **THEN** the system MUST enforce maximum state size per thread (default: 1MB)
- **AND** return error when state would exceed limit
