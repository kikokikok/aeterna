## Why

Aeterna has a rich, production-grade implementation (Helm charts, CLI wizard, MCP server, multi-tenant governance, memory systems, code search, observability) but no browsable documentation site — making it hard for new users to understand the product, configure it, and deploy it confidently. A unified documentation website is the next critical step to make this project usable and adoptable.

## What Changes

- Scaffold a **Docusaurus v3** documentation website under `website/`
- Write or consolidate the following documentation sections:
  - Product overview (what Aeterna is, why it exists, architecture diagram)
  - Getting started guides for each deployment mode (local Docker Compose, Kubernetes Helm, OpenCode-only)
  - CLI reference (`aeterna setup`, sub-commands, flags, wizard walkthrough)
  - Helm chart reference (all values, deployment modes, secrets management, HA, sizing)
  - API / MCP tools reference (all 15+ MCP tools with parameters and examples)
  - Governance & policy guide (CCA, multi-tenant, RBAC, drift)
  - Integration guides (OpenCode, LangChain, Radkit, A2A protocol)
  - Code Search integration guide
  - Security guide (RBAC matrix, tenant isolation, secret management)
  - Observability setup (tracing, logging, metrics)
  - Contributing & development guide
- Add **GitHub Actions CI/CD workflow** to build and deploy to GitHub Pages on every push to `main`
- Configure Mermaid diagram support (architecture diagrams already exist in `docs/`)
- Add `docusaurus-plugin-openapi-docs` for interactive API reference rendering

## Capabilities

### New Capabilities
- `docs-site`: Full Docusaurus v3 documentation website with navigation, search, versioning, and GitHub Pages CI/CD deployment

### Modified Capabilities
<!-- No existing specs change behavior — this is purely additive infrastructure -->

## Impact

- **New directory**: `website/` — Docusaurus v3 app (Node.js, no impact on Rust crates)
- **New CI workflow**: `.github/workflows/deploy-docs.yml` — triggers on push to `main`, builds and deploys to `gh-pages` branch
- **No breaking changes** to existing code, Helm chart, or CLI
- **Dependencies**: Node.js 20+, `@docusaurus/core`, `@docusaurus/preset-classic`, `docusaurus-plugin-openapi-docs`
- **Affected content**: All existing docs in `docs/`, `charts/aeterna/docs/`, `openspec/specs/` will be referenced/linked from the new site
