# decide-rls-enforcement-model

Architectural decision change: records the chosen PostgreSQL row-level security enforcement model for Aeterna. Resolves #58.

- `proposal.md` — Why, What Changes, capabilities touched, decision summary.
- `design.md` — Full analysis of Options A / B / C, hazards surfaced by the analysis, recommendation rationale.
- `specs/runtime-security-hardening/spec.md` — 1 MODIFIED + 2 ADDED requirements on the `runtime-security-hardening` capability.
- `tasks.md` — Implementation tasks for the chosen option (C: RLS as CI-time gate).

**Decision:** Option C — RLS stays enabled and policies are authored, but prod connections remain BYPASSRLS; a dedicated non-BYPASSRLS role (`aeterna_app_rls`) is used by the integration test suite to convert the policies into a CI enforcement gate against missed `WHERE tenant_id = ?` clauses.
