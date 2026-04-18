## 1. `aeterna auth switch` command

- [ ] 1.1 Add `Switch(SwitchArgs)` variant to `AuthCommand` enum in `cli/src/commands/auth.rs`.
- [ ] 1.2 `SwitchArgs { slug: Option<String>, server_only: bool, local_only: bool }` with clap attributes; `server_only` and `local_only` are mutually exclusive.
- [ ] 1.3 Implement `switch_interactive()` — fetches `GET /auth/session`, extracts tenants, uses `dialoguer::Select` to prompt with the active tenant pre-highlighted.
- [ ] 1.4 Implement `switch_non_interactive(slug)` — validates slug against session tenants list (or any tenant if PlatformAdmin), rejects with friendly message if not a member.
- [ ] 1.5 Wire persistence: by default, call both `PUT /user/me/default-tenant` AND `write_context_file(slug)`. Respect `--server-only` / `--local-only`.
- [ ] 1.6 Print a confirmation: `✓ Active tenant: <name> (<slug>)\n  Server default: updated\n  Local context (.aeterna/context.toml): updated`.

## 2. `aeterna auth default-tenant` command

- [ ] 2.1 Add `DefaultTenant(DefaultTenantArgs)` variant to `AuthCommand`.
- [ ] 2.2 `DefaultTenantArgs { slug: Option<String>, clear: bool }`; mutually exclusive.
- [ ] 2.3 No args → `GET /user/me/default-tenant` and print the current value or `<none>`.
- [ ] 2.4 With slug → `PUT /user/me/default-tenant { slug }`.
- [ ] 2.5 `--clear` → `DELETE /user/me/default-tenant`.

## 3. Extended `aeterna auth status`

- [ ] 3.1 Fetch `GET /auth/session` in addition to local profile data.
- [ ] 3.2 Render sections: **Profile** (name, endpoint, token expiry), **Identity** (user login, email, platformAdmin flag), **Tenants** (list with active marker, membership role), **Default tenant** (server-side), **Context** (resolved tenant + resolution source).
- [ ] 3.3 Use colorized output respecting `--no-color` / `NO_COLOR` env.
- [ ] 3.4 Add `--json` flag for machine-readable output; fields stable for scripting.

## 4. Transparent `select_tenant` error handling

- [ ] 4.1 In `cli/src/api/client.rs`, detect HTTP 400 with `error: "select_tenant"` and parse `availableTenants[]`.
- [ ] 4.2 On detection, invoke `prompt_tenant_picker(tenants)` from a new `cli/src/ui/tenant_picker.rs`.
- [ ] 4.3 Picker behavior: if TTY and `--non-interactive` not set → prompt; else exit with the original error message and the helpful hint.
- [ ] 4.4 After a selection, retry the original request exactly once with `X-Tenant-ID: <chosen-slug>` appended.
- [ ] 4.5 Persistence: honor `AETERNA_TENANT_PICKER_PERSISTENCE=session|local|server|none`. Default `session`.
- [ ] 4.6 Guard against infinite retry loops (max 1 retry per request; a second `select_tenant` becomes a hard error).

## 5. `aeterna tenant use --server`

- [ ] 5.1 Add `--server` flag to `TenantUseArgs` in `cli/src/commands/tenant.rs`.
- [ ] 5.2 When set, also call `PUT /user/me/default-tenant` in addition to the existing local context write.
- [ ] 5.3 Print which destinations were updated.

## 6. Tests

- [ ] 6.1 Unit-test slug validation in `switch_non_interactive` (member, non-member, platformAdmin).
- [ ] 6.2 Integration test: mock server returns 2-tenant session; `aeterna auth switch` with stdin simulating arrow+enter picks one; both HTTP PUT and file write verified.
- [ ] 6.3 Integration test: mock server returns `400 select_tenant` on `aeterna user list`; CLI prompts, user picks, retry succeeds with header.
- [ ] 6.4 Integration test: `--non-interactive` + `select_tenant` error → exits 1 with the hint printed.
- [ ] 6.5 Integration test: `aeterna auth default-tenant` with no server-side value set → prints `<none>`; after `PUT`, prints the slug.
- [ ] 6.6 Integration test: `AETERNA_TENANT_PICKER_PERSISTENCE=server` after picker selection → verifies PUT issued.

## 7. Documentation

- [ ] 7.1 Update `docs/cli.md` with an `Authentication and tenant context` section covering the full resolution chain and the new commands.
- [ ] 7.2 Add examples to `README.md`: `aeterna auth switch`, `aeterna auth switch <slug>`, `aeterna auth default-tenant <slug>`.
- [ ] 7.3 Update `aeterna auth --help` output.
