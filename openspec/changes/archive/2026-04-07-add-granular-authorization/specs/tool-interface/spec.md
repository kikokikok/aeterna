## ADDED Requirements

### Requirement: Per-Tool Authorization in MCP Dispatcher
The MCP tool dispatcher SHALL map each tool invocation to its corresponding domain Cedar action and evaluate authorization before executing the tool.

#### Scenario: Memory tool maps to memory Cedar action
- **WHEN** the MCP dispatcher receives a `memory_add` tool call
- **THEN** the dispatcher SHALL evaluate the `CreateMemory` Cedar action against the authenticated principal
- **AND** the tool SHALL only execute if the Cedar evaluation permits the action

#### Scenario: Knowledge tool maps to knowledge Cedar action
- **WHEN** the MCP dispatcher receives a `knowledge_query` tool call
- **THEN** the dispatcher SHALL evaluate the `SearchKnowledge` Cedar action against the authenticated principal

#### Scenario: Governance tool maps to governance Cedar action
- **WHEN** the MCP dispatcher receives a `governance_role_assign` tool call
- **THEN** the dispatcher SHALL evaluate the `AssignRoles` Cedar action against the authenticated principal

#### Scenario: CCA tool maps to CCA Cedar action
- **WHEN** the MCP dispatcher receives any CCA tool call (`context_assemble`, `note_capture`, `hindsight_query`, `meta_loop_status`)
- **THEN** the dispatcher SHALL evaluate the `InvokeCCA` Cedar action against the authenticated principal

#### Scenario: Graph tool maps to graph Cedar action
- **WHEN** the MCP dispatcher receives a read-only graph tool call (`graph_query`, `graph_neighbors`, `graph_path`)
- **THEN** the dispatcher SHALL evaluate the `QueryGraph` Cedar action
- **AND** mutating graph tool calls (`graph_link`, `graph_unlink`) SHALL evaluate the `ModifyGraph` Cedar action

### Requirement: Unique MCP Tool Names
The MCP tool dispatcher SHALL ensure that every registered tool has a unique name, preventing name collisions that cause undefined dispatch behavior.

#### Scenario: No duplicate tool names at registration
- **WHEN** the MCP server registers all tools at startup
- **THEN** the registration SHALL detect and reject duplicate tool names
- **AND** the server SHALL log an error and fail to start if a name collision is found

#### Scenario: governance_role_assign collision resolved
- **WHEN** the MCP tool set includes the previously colliding `governance_role_assign` tools
- **THEN** one SHALL be renamed to `governance_role_grant` (or equivalent unique name)
- **AND** both tools SHALL have distinct names and distinct Cedar action mappings
