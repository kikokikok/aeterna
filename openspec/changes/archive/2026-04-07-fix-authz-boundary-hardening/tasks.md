## 1. Identity and tenant-context hardening

- [x] 1.1 Remove raw caller-controlled tenant/user header fallback from tenant-scoped HTTP APIs in production-capable modes.
- [x] 1.2 Expand `TenantContext` to carry the canonical roles and hierarchy path required by the governance specs.
- [x] 1.3 Centralize validated tenant-context construction in auth middleware and ensure downstream routes consume the validated context.

## 2. Authorization backend enforcement

- [x] 2.1 Replace allow-all authorization wiring in memory/control-plane paths with the configured auth backend or explicit fail-closed behavior.
- [x] 2.2 Implement real role lookup, assignment, and revocation behavior in the active authorization adapter.
- [x] 2.3 Align role-catalog validation across runtime types, API schemas, CLI validation, and Cedar policies.

## 3. Governance integrity protections

- [x] 3.1 Reject governance self-approval and other requestor/approver integrity violations.
- [x] 3.2 Add regression tests for spoofed tenant headers, missing validated identity, and unauthorized role mutation attempts.
- [x] 3.3 Add regression tests for self-approval denial and canonical role-catalog consistency.
