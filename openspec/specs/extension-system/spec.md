# extension-system Specification

## Purpose
TBD - created by archiving change add-cca-capabilities. Update Purpose after archive.
## Requirements
### Requirement: Extension Registration
The system SHALL support registration of extensions with unique identifiers and capabilities.

#### Scenario: Register extension
- **WHEN** extension registers with the system
- **THEN** system SHALL validate extension_id is unique
- **AND** system SHALL store callbacks, prompt_additions, and tool_config
- **AND** system SHALL assign extension to session scope by default

#### Scenario: Validate extension schema
- **WHEN** extension registration is received
- **THEN** system SHALL validate extension_id format (alphanumeric, hyphens, max 64 chars)
- **AND** system SHALL validate callback signatures match expected types
- **AND** system SHALL reject registration if validation fails

#### Scenario: Unregister extension
- **WHEN** extension unregistration is requested
- **THEN** system SHALL remove extension from registry
- **AND** system SHALL cleanup extension state
- **AND** system SHALL log unregistration event

### Requirement: Input Message Callback
The system SHALL invoke on_input_messages callback allowing extensions to transform input messages.

#### Scenario: Invoke input callback
- **WHEN** input messages are received for processing
- **AND** extension has on_input_messages callback registered
- **THEN** system SHALL invoke callback with message array
- **AND** system SHALL use returned messages for further processing
- **AND** system SHALL maintain message order

#### Scenario: Chain multiple extensions
- **WHEN** multiple extensions have on_input_messages callback
- **THEN** system SHALL invoke callbacks in registration order
- **AND** system SHALL pass output of one callback as input to next
- **AND** system SHALL allow any extension to short-circuit processing

#### Scenario: Handle callback error
- **WHEN** on_input_messages callback throws error
- **THEN** system SHALL log error with extension_id and context
- **AND** system SHALL continue with original messages (skip failed extension)
- **AND** system SHALL NOT propagate error to user

### Requirement: Plain Text Callback
The system SHALL invoke on_plain_text callback for text content processing.

#### Scenario: Invoke plain text callback
- **WHEN** plain text content is being processed
- **AND** extension has on_plain_text callback registered
- **THEN** system SHALL invoke callback with text and context
- **AND** system SHALL use returned text for further processing

#### Scenario: Provide context to callback
- **WHEN** invoking on_plain_text callback
- **THEN** system SHALL provide ExtensionContext with:
- **AND** extension_id, session context, state access methods
- **AND** available tools registry

### Requirement: Tag Callback
The system SHALL invoke on_tag callback when specific XML tags are encountered in content.

#### Scenario: Register tag handler
- **WHEN** extension registers on_tag callback with tag pattern
- **THEN** system SHALL associate callback with specified tag(s)
- **AND** system SHALL support exact match and prefix patterns
- **AND** system SHALL allow multiple extensions per tag

#### Scenario: Invoke tag callback
- **WHEN** content contains registered XML tag
- **THEN** system SHALL extract tag name and inner content
- **AND** system SHALL invoke callback with tag, content, and context
- **AND** system SHALL replace tag section with callback result

#### Scenario: Handle nested tags
- **WHEN** XML tag contains nested registered tags
- **THEN** system SHALL process innermost tags first
- **AND** system SHALL process outer tags with inner results
- **AND** system SHALL maintain proper nesting structure

### Requirement: LLM Output Callback
The system SHALL invoke on_llm_output callback after LLM generates response.

#### Scenario: Invoke output callback
- **WHEN** LLM generates output
- **AND** extension has on_llm_output callback registered
- **THEN** system SHALL invoke callback with output and context
- **AND** system SHALL use returned output for display/further processing

#### Scenario: Post-process LLM output
- **WHEN** on_llm_output callback returns modified output
- **THEN** system SHALL use modified output for subsequent operations
- **AND** system SHALL preserve original output in logs for debugging
- **AND** system SHALL allow callback to add metadata annotations

### Requirement: Extension State Management
The system SHALL provide persistent state management for extensions within session scope.

#### Scenario: Get state value
- **WHEN** extension calls get_state(key)
- **THEN** system SHALL return stored value or undefined
- **AND** system SHALL scope state to extension_id and session_id
- **AND** system SHALL deserialize value to requested type

#### Scenario: Set state value
- **WHEN** extension calls set_state(key, value)
- **THEN** system SHALL serialize and store value
- **AND** system SHALL scope to extension_id and session_id
- **AND** system SHALL persist to Redis with session TTL

#### Scenario: Clear state
- **WHEN** extension calls clear_state()
- **THEN** system SHALL remove all state for extension_id in current session
- **AND** system SHALL log state clear event

#### Scenario: State persistence
- **WHEN** session is active
- **THEN** system SHALL maintain state in Redis with session TTL
- **AND** system SHALL automatically cleanup state when session expires
- **AND** system SHALL support state recovery on reconnection

### Requirement: Prompt Wiring
The system SHALL support extension-specific prompt additions.

#### Scenario: Register prompt addition
- **WHEN** extension registers prompt_additions
- **THEN** system SHALL validate addition format (position, content)
- **AND** system SHALL store additions for assembly time
- **AND** system SHALL support positions: system_prefix, system_suffix, user_prefix, user_suffix

#### Scenario: Assemble prompt with additions
- **WHEN** assembling prompt for LLM
- **THEN** system SHALL collect all registered prompt additions
- **AND** system SHALL insert additions at specified positions
- **AND** system SHALL maintain deterministic ordering by extension_id

#### Scenario: Conditional prompt addition
- **WHEN** prompt_addition has condition specified
- **THEN** system SHALL evaluate condition against context
- **AND** system SHALL only include addition if condition passes
- **AND** system SHALL support conditions: tag_present, state_value, context_match

### Requirement: Tool Sequencing
The system SHALL support extension-based tool selection and sequencing guidance.

#### Scenario: Register tool config
- **WHEN** extension registers tool_config
- **THEN** system SHALL validate tool names exist in registry
- **AND** system SHALL store sequencing hints and selection criteria
- **AND** system SHALL support: preferred_tools, avoided_tools, tool_order

#### Scenario: Apply tool selection guidance
- **WHEN** LLM is selecting tools
- **THEN** system SHALL include tool_config guidance in context
- **AND** system SHALL boost preferred_tools in selection
- **AND** system SHALL demote avoided_tools

#### Scenario: Apply tool sequencing
- **WHEN** multiple tools are being executed
- **AND** tool_order is configured
- **THEN** system SHALL suggest execution order based on configuration
- **AND** system SHALL NOT enforce order (guidance only)

### Requirement: Extension Priority
The system SHALL support priority ordering for extension callbacks.

#### Scenario: Set extension priority
- **WHEN** extension registers
- **THEN** system SHALL accept optional priority (0-100, default 50)
- **AND** system SHALL invoke higher priority extensions first
- **AND** system SHALL use registration order for equal priorities

#### Scenario: Priority affects callback order
- **WHEN** multiple extensions register same callback type
- **THEN** system SHALL order invocations by priority descending
- **AND** system SHALL document ordering for predictability

### Requirement: Observability
The system SHALL emit metrics and logs for extension system operations.

#### Scenario: Emit registration metrics
- **WHEN** extension is registered or unregistered
- **THEN** system SHALL emit counter: extension.registration.total with labels (action, extension_id)
- **AND** system SHALL log registration details

#### Scenario: Emit callback metrics
- **WHEN** callback is invoked
- **THEN** system SHALL emit histogram: extension.callback.duration_ms with labels (callback_type, extension_id)
- **AND** system SHALL emit counter: extension.callback.total with labels (callback_type, extension_id, outcome)

#### Scenario: Emit state metrics
- **WHEN** state operation completes
- **THEN** system SHALL emit counter: extension.state.operations with labels (operation, extension_id)
- **AND** system SHALL emit histogram: extension.state.size_bytes with labels (extension_id)

### Requirement: Extension State Memory Limits
The system SHALL enforce memory limits on extension state to prevent Redis bloat during long sessions.

#### Scenario: TTL enforcement on state keys
- **WHEN** extension sets state value
- **THEN** system SHALL apply configurable TTL (default: 1 hour)
- **AND** system SHALL refresh TTL on state read
- **AND** system SHALL automatically expire stale state

#### Scenario: LRU eviction for state
- **WHEN** extension state exceeds per-extension limit (configurable, default: 10MB)
- **THEN** system SHALL evict least recently used keys
- **AND** system SHALL log eviction events
- **AND** system SHALL emit metric: extension.state.evictions with labels (extension_id)

#### Scenario: Session state size monitoring
- **WHEN** session is active
- **THEN** system SHALL track total state size per session
- **AND** system SHALL alert when approaching limit (80% of max)
- **AND** system SHALL enforce hard limit to prevent OOM

