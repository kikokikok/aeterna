## ADDED Requirements

### Requirement: Manifest-Based Tenant Apply
The CLI SHALL expose `aeterna tenant apply -f <manifest>` as the primary command for creating and updating tenants. `apply` SHALL be idempotent, report per-step progress, and SHALL internally submit the parsed manifest to `POST /api/v1/admin/tenants/provision`.

#### Scenario: Apply a manifest from a file
- **WHEN** a user runs `aeterna tenant apply -f ./manifest.yaml`
- **AND** the user is authenticated with PlatformAdmin or a token bearing `tenant:provision`
- **THEN** the CLI SHALL parse and client-side-validate the manifest
- **AND** submit it to the provisioning endpoint with `X-Aeterna-Client-Kind: cli`
- **AND** stream per-step status to stdout
- **AND** exit 0 on `overallOk: true`, non-zero otherwise (see exit-code requirement)

#### Scenario: Apply from stdin
- **WHEN** a user runs `aeterna tenant apply -f -` and pipes a manifest on stdin
- **THEN** the CLI SHALL read until EOF and treat the content as the manifest body

#### Scenario: Apply with --dry-run
- **WHEN** `--dry-run` is passed
- **THEN** the CLI SHALL submit with `?dryRun=true`
- **AND** SHALL display planned steps prefixed with `[DRY-RUN]`
- **AND** SHALL NOT mutate server state

#### Scenario: Apply with --watch streams progress
- **WHEN** `--watch` is passed
- **THEN** the CLI SHALL open a server-sent-events or long-poll channel against the provisioning endpoint
- **AND** SHALL render each step status as it arrives
- **AND** SHALL exit when the final step completes or the connection closes

### Requirement: Render, Diff, and Validate
The CLI SHALL support reading a tenant's current state as a manifest (`render`), previewing a change against current state (`diff`), and validating a manifest offline (`validate`).

#### Scenario: Render current tenant state
- **WHEN** a user runs `aeterna tenant render --slug acme -o yaml`
- **THEN** the CLI SHALL GET `/api/v1/admin/tenants/acme/manifest`
- **AND** print the returned manifest to stdout
- **AND** secret values SHALL NOT appear; only references SHALL be printed

#### Scenario: Diff against current state
- **WHEN** a user runs `aeterna tenant diff --slug acme -f ./manifest.yaml`
- **THEN** the CLI SHALL submit the manifest to `/api/v1/admin/tenants/diff`
- **AND** print a unified-diff-style or JSON representation of the delta
- **AND** exit 0 if the delta is empty, 1 if it is non-empty, 2 on authorization error

#### Scenario: Validate a manifest offline
- **WHEN** a user runs `aeterna tenant validate -f ./manifest.yaml`
- **THEN** the CLI SHALL perform structural, schema, and reference-shape validation locally
- **AND** SHALL NOT contact any server
- **AND** exit 0 on valid, 1 on invalid (with line-numbered error messages)

### Requirement: Secure Secret Input Modes
The CLI SHALL provide mode-typed input for secret values. Raw `--value` arguments SHALL be rejected unless the caller also passes `--allow-inline-secret` AND the server advertises inline-secret support.

#### Scenario: Reference mode stores only a pointer
- **WHEN** a user runs `aeterna tenant secret set --logical-name llmCredentials --ref k8s:tenant-acme-llm#credentials.json`
- **THEN** the CLI SHALL construct a `K8sSecretRef` and submit it
- **AND** SHALL NOT read or transmit the underlying value

#### Scenario: File mode enforces 0600
- **WHEN** a user runs `aeterna tenant secret set ... --from-file ./secret.txt`
- **AND** the file's mode is looser than `0600`
- **THEN** the CLI SHALL refuse with exit code 1 and a clear message

#### Scenario: Stdin mode reads from fd 0
- **WHEN** a user runs `aeterna tenant secret set ... --from-stdin`
- **THEN** the CLI SHALL read exactly one line from stdin and use it as the value
- **AND** SHALL disable terminal echo if stdin is a TTY

#### Scenario: Inline value rejected by default
- **WHEN** a user runs `aeterna tenant secret set ... --value foo`
- **AND** `--allow-inline-secret` is not also set
- **THEN** the CLI SHALL refuse with exit code 1
- **AND** the error SHALL suggest `--from-stdin`, `--from-file`, or `--ref`

### Requirement: Unified Output Format Flag
Every CLI command that emits data SHALL accept `-o` / `--output` with the values `table` (default for TTY), `json`, `yaml`, `name`, and `jsonpath=<expr>`. Non-TTY invocations SHALL default to `json` unless a format is given.

#### Scenario: TTY defaults to table
- **WHEN** stdout is a TTY and `--output` is not specified
- **THEN** the CLI SHALL render a human-readable table

#### Scenario: Piped invocation defaults to JSON
- **WHEN** stdout is not a TTY and `--output` is not specified
- **THEN** the CLI SHALL render JSON suitable for `jq`

#### Scenario: jsonpath extracts a field
- **WHEN** `--output jsonpath='$.manifestHash'` is passed
- **THEN** the CLI SHALL print the extracted value followed by a newline

### Requirement: Standard Exit Codes
The CLI SHALL use the following exit codes uniformly across commands:

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Client-side validation error |
| 2    | Authentication or authorization error |
| 3    | Conflict (stale generation, resource already exists with different identity) |
| 4    | Transient error (timeout, 5xx, network), retry may succeed |
| 5    | Unrecoverable server error |

#### Scenario: Authentication failure exits 2
- **WHEN** the CLI receives HTTP 401 or 403
- **THEN** the CLI SHALL exit with code 2
- **AND** SHALL print a message indicating how to re-authenticate

#### Scenario: Stale generation exits 3
- **WHEN** `tenant apply` receives HTTP 409 with `reason: stale_generation`
- **THEN** the CLI SHALL exit with code 3
- **AND** SHALL suggest `aeterna tenant render --slug <s>` to fetch the current generation

### Requirement: Token Source Hierarchy
The CLI SHALL read API tokens from, in order: (1) `AETERNA_API_TOKEN` env var, (2) the OS keychain entry for the active context, (3) `~/.aeterna/credentials` (mode-gated to `0600`). The CLI SHALL NOT accept a `--token` flag.

#### Scenario: Env var overrides keychain
- **WHEN** `AETERNA_API_TOKEN` is set
- **THEN** the CLI SHALL use its value without reading the keychain

#### Scenario: Credentials file mode enforced
- **WHEN** the CLI falls back to `~/.aeterna/credentials`
- **AND** the file mode is looser than `0600`
- **THEN** the CLI SHALL refuse to read the file and exit with code 2

#### Scenario: --token flag rejected
- **WHEN** a user passes `--token <value>`
- **THEN** the CLI SHALL exit with code 1
- **AND** SHALL suggest `AETERNA_API_TOKEN` or `aeterna auth login`
