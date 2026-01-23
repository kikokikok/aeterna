// Aeterna Agent Delegation Policies
// Defines how AI agents inherit permissions from their delegating principals.
//
// Key Concepts:
// - Agents receive permissions from the user/agent that delegated to them
// - Delegation depth tracks hops from a human (default max: 3)
// - Agents can only delegate subset of their own capabilities
// - Capabilities are explicit strings like "memory:read", "knowledge:propose"

// =============================================================================
// AGENT MEMORY ACCESS - Via Delegation
// =============================================================================

// Agent: View memory if delegator has capability and agent has memory:read
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ViewMemory",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("memory:read")
};

// Agent: Create memory if delegator has capability and agent has memory:write
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"CreateMemory",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("memory:write")
};

// Agent: Update memory if delegator has capability and agent has memory:write
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"UpdateMemory",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("memory:write")
};

// Agent: Delete memory if delegator has capability and agent has memory:delete
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"DeleteMemory",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("memory:delete")
};

// Agent: Promote memory requires explicit capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"PromoteMemory",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("memory:promote")
};

// =============================================================================
// AGENT KNOWLEDGE ACCESS - Via Delegation
// =============================================================================

// Agent: View knowledge if has knowledge:read capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ViewKnowledge",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("knowledge:read")
};

// Agent: Propose knowledge if has knowledge:propose capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ProposeKnowledge",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("knowledge:propose")
};

// Agent: Edit knowledge if has knowledge:edit capability (rare for agents)
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"EditKnowledge",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("knowledge:edit")
};

// =============================================================================
// AGENT POLICY ACCESS - Via Delegation
// =============================================================================

// Agent: View policy if has policy:read capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ViewPolicy",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("policy:read")
};

// Agent: Create policy draft if has policy:create capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"CreatePolicy",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("policy:create")
};

// Agent: Simulate policy if has policy:simulate capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"SimulatePolicy",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("policy:simulate")
};

// =============================================================================
// AGENT GOVERNANCE ACCESS - Via Delegation
// =============================================================================

// Agent: View governance requests if has governance:read capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ViewGovernanceRequest",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("governance:read")
};

// Agent: Submit governance requests if has governance:submit capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"SubmitGovernanceRequest",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("governance:submit")
};

// =============================================================================
// AGENT ORGANIZATION ACCESS - Via Delegation
// =============================================================================

// Agent: View organization structure if has org:read capability
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ViewOrganization",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth <= principal.max_delegation_depth &&
    principal.capabilities.contains("org:read")
};

// =============================================================================
// AGENT DELEGATION TO OTHER AGENTS - Constrained
// =============================================================================

// Agent: Register sub-agent if has agent:register capability
// Note: Sub-agent will have delegation_depth + 1
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"RegisterAgent",
    resource
)
when {
    principal.status == "active" &&
    principal.delegation_depth < principal.max_delegation_depth &&
    principal.capabilities.contains("agent:register")
};

// Agent: Delegate to another agent if has agent:delegate capability
// Note: Can only delegate capabilities the agent itself has
permit (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"DelegateToAgent",
    resource is Aeterna::Agent
)
when {
    principal.status == "active" &&
    principal.delegation_depth < principal.max_delegation_depth &&
    principal.capabilities.contains("agent:delegate")
};

// =============================================================================
// DEFAULT CAPABILITY SETS (Reference)
// =============================================================================
//
// Standard AI Coding Assistant:
//   capabilities: ["memory:read", "memory:write", "knowledge:read", 
//                  "knowledge:propose", "policy:read", "policy:simulate",
//                  "governance:read", "governance:submit", "org:read"]
//
// Autonomous Agent (higher trust):
//   capabilities: ["memory:read", "memory:write", "memory:delete",
//                  "knowledge:read", "knowledge:propose", "knowledge:edit",
//                  "policy:read", "policy:create", "policy:simulate",
//                  "governance:read", "governance:submit", "org:read",
//                  "agent:register", "agent:delegate"]
//
// Read-Only Agent:
//   capabilities: ["memory:read", "knowledge:read", "policy:read",
//                  "governance:read", "org:read"]
//
// Memory-Only Agent:
//   capabilities: ["memory:read", "memory:write"]
