# RBAC Testing Procedures

## Overview

This document describes procedures for testing Role-Based Access Control (RBAC) in Aeterna's multi-tenant governance system.

## Test Categories

### 1. Permission Matrix Tests

Location: `adapters/tests/rbac_matrix_test.rs`

Tests verify each role-action-resource combination:
- `rbac_matrix::*` - Full permission matrix
- `privilege_escalation_prevention::*` - Security boundary tests
- `role_hierarchy_enforcement::*` - Precedence validation
- `resource_action_matrix::*` - Comprehensive combinations

### 2. Cedar Integration Tests

Location: `adapters/tests/cedar_integration.rs`

Tests verify Cedar policy evaluation:
- `multi_tenant_isolation::*` - Cross-tenant boundary tests
- `agent_delegation::*` - ActAs permission inheritance
- `rbac::*` - Role-based permission tests
- `hierarchical_permissions::*` - Unit hierarchy tests
- `edge_cases::*` - Boundary condition tests
- `error_handling::*` - Invalid input handling

## Running Tests

```bash
cargo test -p adapters --test rbac_matrix_test
cargo test -p adapters --test cedar_integration
cargo test -p adapters -- --nocapture
```

## Test Result Criteria

### Pass Criteria

All tests must:
1. Return expected permission decisions (allow/deny)
2. Enforce role hierarchy (higher roles have superset permissions)
3. Prevent privilege escalation
4. Maintain tenant isolation

### Failure Indicators

- Higher role denied action that lower role can perform
- Lower role granted action reserved for higher role
- Cross-tenant access permitted
- Agent escalation beyond delegated user

## Role Hierarchy

| Precedence | Role | Key Permissions |
|------------|------|-----------------|
| 4 | Admin | Full access, role management |
| 3 | Architect | Knowledge approval, org promotion |
| 2 | TechLead | Team management, team promotion |
| 1 | Developer | User-layer memory, view knowledge |
| 0 | Agent | Delegated view-only by default |

## Adding New RBAC Tests

When adding new roles, actions, or resources:

1. Update schema in `ROLE_SCHEMA` constant
2. Add policies to `ROLE_BASED_POLICIES` constant
3. Add test cases to `resource_action_matrix` module
4. Run `scripts/generate_rbac_matrix.sh` to update documentation
5. Commit updated `docs/security/rbac-matrix.md`

## CI Integration

RBAC tests run automatically via `.github/workflows/rbac-tests.yml`:
- Triggered on changes to auth code or policies
- Verifies all permission tests pass
- Ensures RBAC matrix documentation is current
- Blocks merge on any test failure

## Security Review Checklist

Before releasing RBAC changes:

- [ ] All positive authorization tests pass
- [ ] All negative authorization tests pass
- [ ] Privilege escalation tests pass
- [ ] Role hierarchy enforcement verified
- [ ] Cross-tenant isolation verified
- [ ] Agent delegation scoped correctly
- [ ] RBAC matrix documentation updated
- [ ] Security team sign-off obtained
