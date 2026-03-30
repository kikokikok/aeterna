## Context

Aeterna's repository contains a mixture of mature subsystems and incomplete last-mile integration. Core storage, tenancy, and broad platform capabilities exist, but the repo still ships broken default paths in the Docker entrypoint, Helm hooks, OPAL secret handling, dependency defaults, and placeholder runtime flows in CLI and service layers.

The goal of this change is to define a coherent hardening pass that makes the shipped deployment and runtime contracts accurate and verifiable without introducing new product scope.

## Goals / Non-Goals

### Goals
- Make default runtime entrypoints and migration flows execute valid supported commands
- Make Helm deployment defaults safe to install and safe to upgrade
- Require real backend-backed behavior for exposed runtime commands and service health endpoints
- Define fail-closed auth, token handling, and CORS requirements for production deployment modes
- Require CI and documentation to validate and describe the same supported paths that the repo actually ships

### Non-Goals
- Add new research capabilities or future platform features unrelated to current broken paths
- Redesign the Aeterna architecture or replace existing subsystems wholesale
- Define managed cloud provisioning strategy beyond what is needed to fix current production-readiness gaps

## Decisions

### Decision: Use one umbrella change with phased tasks
This change spans multiple existing capabilities, but the issues are tightly coupled through one outcome: making the shipped product deployable and operationally credible. The change will therefore use a single umbrella proposal with capability-specific deltas and phased implementation tasks.

### Decision: Add a dedicated `runtime-operations` capability
Current runtime contracts for CLI behavior, API health semantics, entrypoint correctness, and migration invocation are scattered across docs and code. A dedicated capability is needed so these runtime behaviors become first-class, testable requirements.

### Decision: Fix broken paths before advanced hardening
The implementation order prioritizes broken shipped paths first, then placeholder runtime behavior, then CI/docs alignment. This keeps the work measurable and avoids polishing deployments that still fail at startup or upgrade time.

## Risks / Trade-offs

- A single change touching multiple specs is broader than ideal, but splitting it would fragment one tightly related remediation effort into several partially overlapping proposals.
- Tightening auth, secrets, and validation may break previously permissive or loosely documented workflows; this is intentional and must be documented as part of the migration.
- Replacing placeholder CLI/API success responses with explicit failures may temporarily surface unsupported areas more clearly to users.

## Migration Plan

1. Correct shipped runtime/deployment entrypoints and unsafe Helm defaults
2. Replace placeholder command and health behavior with real backend-backed execution or explicit unsupported errors
3. Tighten auth/secrets/upgrade behavior for production deployments
4. Align CI, examples, and docs to validate only supported, working flows

## Open Questions

- Which deployment path becomes the single canonical operator path: main Helm chart only, or a supported subset of `deploy/` assets?
- Which runtime surfaces must fail explicitly versus remain feature-flagged during the transition from placeholder behavior?
