# Permit.io / Cedar Policies for Aeterna

# This file defines the Relationship-Based Access Control (RBAC) 
# and Organizational Hierarchy logic for the Aeterna Multi-Tenant system.

# Hierarchy:
# Company > Organization > Team > Project

# Roles:
# - Admin: Full access to everything in the tenant.
# - Architect: Can approve knowledge proposals, define policies.
# - TechLead: Can propose knowledge, manage team-level settings.
# - Developer: Can create memories, read knowledge.
# - Agent: Identity for LLM agents, can have roles assigned.

# Permissions:
# - memory:read, memory:write
# - knowledge:read, knowledge:propose, knowledge:approve, knowledge:write
# - policy:read, policy:write

permit_check(principal, action, resource, context) {
    # 1. Hard Isolation (handled by PDP/Middleware, but here for completeness)
    principal.tenant == resource.tenant;

    # 2. Hierarchy Inheritance
    # If user has role at Org level, they inherit permissions for all Teams/Projects under it.
    
    # 3. Role-based logic
    if (principal.role == "Admin") { return allow; }
    
    if (action == "knowledge:approve") {
        return principal.role == "Architect" || principal.role == "Admin";
    }

    if (action == "knowledge:propose") {
        return principal.role == "Developer" || principal.role == "TechLead" || principal.role == "Architect";
    }
    
    # Default deny
    return deny;
}
