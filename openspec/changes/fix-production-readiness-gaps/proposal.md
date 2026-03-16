# Change: Fix Production Readiness Gaps

## Why

Repository review showed that Aeterna has substantial core functionality, but several shipped runtime, deployment, and operational paths are still inconsistent or broken. The most serious issues are not missing future features; they are gaps between the documented product surface and the behavior that currently works in containers, Helm installs, CLI commands, and service integrations.

The current state creates four concrete risks:
- default startup and migration paths do not execute the intended commands
- the Helm chart contains broken or unsafe defaults for secrets, dependency wiring, and upgrade behavior
- several CLI/API/integration flows return simulated or placeholder responses instead of real backend behavior
- documentation and CI validate an incomplete picture, allowing broken paths to appear healthy

This change defines the work required to make Aeterna deployable, operationally honest, and production-credible.

## What Changes

- Correct runtime entrypoints, migration invocation, and health behavior so shipped commands execute real supported flows
- Replace simulated CLI/API/integration paths with real backend-backed behavior or explicit failure modes
- Harden Helm deployment defaults for secret reuse, dependency wiring, network isolation, ingress/TLS, and upgrade safety
- Align governance/auth behavior with fail-closed production defaults, including JWT and CORS expectations
- Ensure observability endpoints and metrics represent actual dependency and process health rather than placeholders
- Reconcile CI, docs, and example configuration with the actual supported deployment paths and runtime contracts

## Impact

- Affected specs:
  - `deployment`
  - `governance`
  - `observability`
  - `opencode-integration`
  - `runtime-operations` (new)
- Affected code:
  - `cli/`
  - `agent-a2a/`
  - `storage/`
  - `packages/opencode-plugin/`
  - `opal-fetcher/`
  - `charts/aeterna/`
  - `deploy/`
  - `.github/workflows/`
  - `docs/`, `README.md`, `INSTALL.md`, `Dockerfile`
