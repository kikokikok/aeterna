## ADDED Requirements

### Requirement: MCP Promotion Lifecycle Tools
The OpenCode integration SHALL expose first-class MCP tools for knowledge promotion workflows.

#### Scenario: Preview promotion from MCP
- **WHEN** a client invokes `aeterna_knowledge_promotion_preview`
- **THEN** the tool SHALL return suggested shared content, residual content, and residual role

#### Scenario: Submit promotion from MCP
- **WHEN** a client invokes `aeterna_knowledge_promote`
- **THEN** the tool SHALL create a promotion request
- **AND** the tool SHALL return the request identifier and review status

#### Scenario: Review pending promotion requests
- **WHEN** a client invokes `aeterna_knowledge_review_pending`
- **THEN** the tool SHALL return pending promotion requests with target layer and reviewer context

### Requirement: MCP Backward Compatibility
The OpenCode integration SHALL preserve existing proposal tools while adding promotion lifecycle tools.

#### Scenario: Existing proposal tool still works
- **WHEN** a client invokes `aeterna_knowledge_propose`
- **THEN** the tool SHALL continue to behave as before
- **AND** promotion lifecycle tools SHALL be additive
