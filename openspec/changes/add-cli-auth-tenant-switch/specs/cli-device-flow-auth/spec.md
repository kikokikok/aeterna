## ADDED Requirements

### Requirement: CLI Tenant Switch Command
The CLI SHALL provide an `aeterna auth switch [slug]` command that lets the authenticated user select or change their active tenant. The command SHALL persist the selection both locally (in `.aeterna/context.toml`) and server-side (via `PUT /api/v1/user/me/default-tenant`) by default.

#### Scenario: Interactive tenant switch
- **WHEN** the user runs `aeterna auth switch` with no slug argument
- **AND** stdin is a TTY
- **THEN** the CLI SHALL fetch `GET /api/v1/auth/session`, render a selectable list of accessible tenants, and prompt the user to choose one
- **AND** after selection, the CLI SHALL call `PUT /api/v1/user/me/default-tenant` and write the slug to `.aeterna/context.toml`
- **AND** the CLI SHALL print a confirmation indicating which destinations were updated

#### Scenario: Non-interactive tenant switch
- **WHEN** the user runs `aeterna auth switch <slug>`
- **AND** the slug is one of the user's accessible tenants, OR the user is a PlatformAdmin
- **THEN** the CLI SHALL persist the selection without prompting and print the confirmation

#### Scenario: Tenant switch with --server-only
- **WHEN** the user runs `aeterna auth switch <slug> --server-only`
- **THEN** the CLI SHALL update the server-side default only and SHALL NOT modify `.aeterna/context.toml`

#### Scenario: Tenant switch with --local-only
- **WHEN** the user runs `aeterna auth switch <slug> --local-only`
- **THEN** the CLI SHALL update `.aeterna/context.toml` only and SHALL NOT call any server endpoint

#### Scenario: Tenant switch to foreign tenant by non-admin
- **WHEN** a non-PlatformAdmin user runs `aeterna auth switch <slug>` with a slug they have no membership in
- **THEN** the CLI SHALL refuse the operation client-side with a friendly error and SHALL NOT issue the PUT request

### Requirement: CLI Default-Tenant Subcommand
The CLI SHALL provide an `aeterna auth default-tenant [slug]` command that manages only the server-side default tenant preference, without touching local context files.

#### Scenario: Read current default
- **WHEN** the user runs `aeterna auth default-tenant` with no arguments
- **THEN** the CLI SHALL call `GET /api/v1/user/me/default-tenant` and print the current value (slug and name), or `<none>` if unset

#### Scenario: Set default
- **WHEN** the user runs `aeterna auth default-tenant <slug>`
- **THEN** the CLI SHALL call `PUT /api/v1/user/me/default-tenant { slug }` and print the updated value

#### Scenario: Clear default
- **WHEN** the user runs `aeterna auth default-tenant --clear`
- **THEN** the CLI SHALL call `DELETE /api/v1/user/me/default-tenant` and print `<none>`

### Requirement: Transparent select_tenant Handling
When any CLI command receives `400 select_tenant` from the server, the CLI SHALL interpret the enriched error payload, prompt the user to select a tenant (when interactive and when `--non-interactive` is not set), and transparently retry the original request with the chosen tenant applied as `X-Tenant-ID`.

#### Scenario: Interactive picker triggered by select_tenant
- **WHEN** the CLI sends a request that receives `400 select_tenant`
- **AND** stdin is a TTY and `--non-interactive` is not set
- **THEN** the CLI SHALL render the `availableTenants` list from the error payload in a selectable prompt
- **AND** after selection, the CLI SHALL retry the original request exactly once with `X-Tenant-ID: <chosen-slug>` added

#### Scenario: Non-interactive select_tenant failure
- **WHEN** the CLI sends a request that receives `400 select_tenant`
- **AND** stdin is not a TTY, or `--non-interactive` is set
- **THEN** the CLI SHALL exit with a non-zero status and print the error `hint` along with the accessible tenants from the payload

#### Scenario: Picker persistence modes
- **WHEN** the user selects a tenant via the `select_tenant` picker
- **AND** `AETERNA_TENANT_PICKER_PERSISTENCE` is set to `session` (default)
- **THEN** the CLI SHALL use the selection only for the current invocation and SHALL NOT persist it anywhere
- **WHEN** the value is `local`, the CLI SHALL additionally write `.aeterna/context.toml`
- **WHEN** the value is `server`, the CLI SHALL additionally call `PUT /api/v1/user/me/default-tenant`
- **WHEN** the value is `none`, the CLI SHALL behave as `session` but SHALL also suppress future pickers in the same run (for scripted loops)

#### Scenario: Retry loop guard
- **WHEN** a retried request also returns `400 select_tenant`
- **THEN** the CLI SHALL NOT prompt again and SHALL exit with a non-zero status

### Requirement: Extended auth status Output
The `aeterna auth status` command SHALL report the full authentication and tenant-resolution state so users can diagnose context issues without reading code or inspecting files.

#### Scenario: Status shows resolution source
- **WHEN** the user runs `aeterna auth status`
- **THEN** the output SHALL include the resolved active tenant (if any) and the source that produced it (one of: `flag`, `env`, `header`, `local-context`, `server-default`, `auto-single-tenant`, `unresolved`)
- **AND** the output SHALL list all accessible tenants with an indicator on the active one
- **AND** the output SHALL show the server-side default tenant slug (or `<none>`)

#### Scenario: Status JSON output
- **WHEN** the user runs `aeterna auth status --json`
- **THEN** the CLI SHALL output a stable JSON document containing `profile`, `identity`, `tenants[]`, `defaultTenant`, and `context` fields

## MODIFIED Requirements

### Requirement: CLI Tenant Use Command
The existing `aeterna tenant use <slug>` command SHALL continue to set the local repo-pinned tenant in `.aeterna/context.toml`. It SHALL additionally support a `--server` flag that mirrors the selection to the server-side default.

#### Scenario: Local-only tenant use (default)
- **WHEN** the user runs `aeterna tenant use <slug>`
- **THEN** the CLI SHALL write the slug to `.aeterna/context.toml` and SHALL NOT call the server default-tenant endpoint

#### Scenario: Tenant use with --server
- **WHEN** the user runs `aeterna tenant use <slug> --server`
- **THEN** the CLI SHALL write the slug to `.aeterna/context.toml` AND call `PUT /api/v1/user/me/default-tenant`
