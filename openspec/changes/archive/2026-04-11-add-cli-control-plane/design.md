## Context

The current `aeterna` binary mixes three very different roles: local bootstrap utility, server launcher, and nominal end-user control plane. The first two are implemented; the third is mostly stubbed. The code already contains the pieces needed to support a real control plane — HTTP endpoints, plugin auth bootstrap/refresh APIs, a partially built offline client, and project/user context resolution — but those pieces are not assembled into a coherent CLI experience.

This change is cross-cutting because it touches command dispatch, auth/session UX, configuration precedence, offline handling, packaging/distribution, and user-facing documentation. It also interacts with tenant hardening work: authenticated CLI flows must not encode assumptions that conflict with the multi-tenant remediation change.

## Goals / Non-Goals

**Goals:**
- Define one supported control-plane UX for the existing `aeterna` binary rather than keeping most commands as disconnected shells.
- Standardize CLI authentication, token persistence, server targeting, config file layout, and command execution semantics.
- Preserve honest runtime behavior: commands either execute real work against a backend or fail explicitly.
- Document full CLI user journeys end to end, including first install, first login, daily usage, and operator flows.
- Define native installation/distribution expectations for macOS and Linux.

**Non-Goals:**
- Replacing the OpenCode plugin auth flow or moving plugin auth responsibilities into a separate binary.
- Solving tenant-isolation defects inside this change; those are handled by the dedicated fail-closed multi-tenant remediation.
- Re-implementing the legacy standalone code-search binary.

## Decisions

### Keep a single `aeterna` control-plane binary
Use the existing `aeterna` binary as the supported control plane instead of introducing a second dedicated CLI binary. This preserves the existing command namespace, avoids fragmenting installation and support docs, and keeps `aeterna serve` colocated with the same binary users use operationally.

**Alternatives considered:**
- **Separate control-plane binary**: rejected because it would duplicate branding, installation, and auth/config logic while leaving the existing `aeterna` command surface ambiguous.
- **Keep only local/bootstrap commands in CLI**: rejected because the server and auth surfaces already exist and users expect the CLI commands to actually work.

### Introduce authenticated CLI profiles with explicit target selection
The CLI will use named profiles that capture server URL, auth method metadata, and environment labeling. A user-level config file will hold reusable profiles, and project-level config will be allowed to select a default profile or override selected settings.

**Alternatives considered:**
- **Only environment variables**: rejected because it keeps the current ad hoc UX and does not scale for multiple environments.
- **Only project-local config**: rejected because operators and developers often target multiple servers from one workstation.

### Use secure local credential storage with explicit fallback behavior
Interactive CLI auth will use OS-backed secure storage when available, with a documented fallback path only for environments where secure storage is unavailable. The CLI will persist only the minimum credentials required for session continuation and refresh.

**Alternatives considered:**
- **Plaintext token file only**: rejected because it conflicts with credential-security expectations already present elsewhere in the product.
- **Environment variable only**: rejected because it does not support a first-class login/logout/status UX.

### Reuse the server auth bootstrap flow from the CLI
The CLI control plane will consume the existing auth bootstrap/refresh/logout server endpoints rather than inventing a second server-side auth contract. This keeps auth lifecycle logic centralized in the server.

**Alternatives considered:**
- **Separate CLI-only auth API**: rejected because it would duplicate token lifecycle and identity resolution.
- **Static token only**: rejected because it leaves interactive auth unsolved.

### Make command behavior converge on backend-backed execution through a shared client layer
Backend-facing commands will share one authenticated CLI client abstraction that handles profile resolution, auth headers, retries, offline/degraded checks, and output normalization. The existing offline client code becomes part of this shared layer instead of remaining disconnected.

**Alternatives considered:**
- **Per-command ad hoc HTTP calls**: rejected because it would duplicate auth/config/error logic across many command groups.
- **Continue with simulated responses**: rejected because runtime specs already require honest behavior.

### Treat `code-search` as a supported integration contract, not a legacy shell
The CLI must either route `code-search` commands to a supported backend (for example, MCP-backed or service-backed) or fail explicitly with a documented unsupported status. Dead commands that only reference a removed binary are not acceptable.

### Ship native install/distribution through release artifacts plus package-manager entry points
The supported distribution model will center on GitHub release artifacts with package-manager entry points (such as Homebrew for macOS and native Linux install packages or scripted installers) rather than container-only guidance.

## Risks / Trade-offs

- **[Risk] Auth UX diverges from plugin auth expectations** → Mitigation: reuse the same bootstrap/refresh/logout server contract and document differences explicitly.
- **[Risk] Profile/config layering becomes confusing** → Mitigation: define one canonical precedence model and add `config show/validate` commands plus end-to-end scenarios.
- **[Risk] Offline and connected modes drift apart** → Mitigation: require one shared client abstraction and honest unsupported/degraded output.
- **[Risk] Packaging increases release complexity** → Mitigation: keep one binary target and define packaging outputs as release artifacts, not separate product variants.
- **[Risk] CLI auth design bakes in tenant assumptions before hardening lands** → Mitigation: treat the multi-tenant fail-closed change as a dependency for production-grade authenticated flows.

## Migration Plan

1. Define the CLI control-plane contract in specs and user journeys.
2. Introduce profile/config/auth client infrastructure behind existing command names.
3. Convert command groups incrementally from stubs to real backend-backed execution.
4. Add native packaging and install documentation to the release flow.
5. Deprecate legacy/dead command behaviors only after supported replacements exist.

## Open Questions

- Which secure credential store abstraction best fits the current Rust dependency posture across macOS and Linux?
- Should Linux packaging target `.deb`/`.rpm`, a curl installer, or both in the first supported release?
- Which code-search backend becomes the supported CLI path: direct service calls, MCP passthrough, or an explicit unsupported contract for now?
