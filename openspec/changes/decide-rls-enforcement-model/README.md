# decide-rls-enforcement-model

**Decision: Option A** — activate RLS on every request-scoped connection, via a **two-role / two-pool / two-helper** design.

- `aeterna_app` (NOBYPASSRLS) + `state.pool` + `with_tenant_context(&ctx, …)` — 99% of traffic
- `aeterna_admin` (BYPASSRLS) + `state.admin_pool` (size=4) + `with_admin_context(&ctx, …)` — PA cross-tenant, scheduled cross-tenant jobs, migrations
- `system_ctx` sentinel for internal scheduled work
- Every `with_admin_context` call is auto-audited inside the helper

Implementation lands in **3 stacked bundles** (originally 5 — A.4 and A.5 collapsed into A.3 wave 6 because the system is pre-production):

- **A.1** — Hazard fixes (H1 session-scope set_config, H2 dual-GUC namespace)
- **A.2** — Roles + migration + dual pools + both helpers + CI verification suite
- **A.3** — Call-site refactor in 6 waves; wave 6 flips `DATABASE_URL`, deletes orphan calls (H3), graduates lint to deny

See `proposal.md` / `design.md` / `tasks.md` / `specs/runtime-security-hardening/spec.md`.

Tracks issue #58. Graduated via PR #72.
