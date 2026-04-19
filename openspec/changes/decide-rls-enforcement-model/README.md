# decide-rls-enforcement-model

**Decision: Option A** — activate RLS on every request-scoped connection.

- `proposal.md` — why, decision summary, 5-bundle rollout plan.
- `design.md` — threat model, option analysis, implementation strategy, risk register.
- `tasks.md` — itemized work per bundle (A.1 hazard fixes → A.2 role + CI → A.3 repo refactor → A.4 prod flip → A.5 cleanup).
- `specs/runtime-security-hardening/spec.md` — normative spec delta (final-state wording).

Originally opened as Option C (RLS as a test-time gate). Architect override on threat-model grounds pivoted the decision to A. See commit trail.

Tracks issue #58. Graduated via PR #72.
