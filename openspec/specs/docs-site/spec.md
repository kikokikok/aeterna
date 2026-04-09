# docs-site Specification

## Purpose
TBD - created by archiving change add-docs-site. Update Purpose after archive.
## Requirements
### Requirement: Documentation Website
The system SHALL provide a Docusaurus v3 documentation website at `website/` that consolidates all existing Aeterna documentation into a navigable, searchable, and professionally structured site.

#### Scenario: Site builds successfully
- **WHEN** `npm run build` is executed in the `website/` directory
- **THEN** the site compiles without errors to `website/build/`

#### Scenario: Full navigation structure is present
- **WHEN** a user opens the documentation site
- **THEN** they can navigate to all major sections: Overview, Getting Started, Concepts, Memory System, Knowledge Repository, Governance, CCA, Integrations, Security, Helm Deployment, Operations, Examples, Reference, and Contributing

#### Scenario: Existing docs are wired into the site
- **WHEN** a user navigates any section of the documentation site
- **THEN** the content from the corresponding existing markdown file is displayed (e.g. `docs/cca/overview.md`, `charts/aeterna/docs/architecture.md`, `specs/00-overview.md`)

#### Scenario: Mermaid diagrams render
- **WHEN** a page containing a Mermaid code block is viewed
- **THEN** the diagram renders as an inline SVG graphic

#### Scenario: Search works
- **WHEN** a user types a query in the search bar
- **THEN** matching pages and headings are shown in real time

### Requirement: GitHub Pages CI/CD Deployment
The system SHALL automatically build and deploy the documentation website to GitHub Pages on every push to the `main` branch.

#### Scenario: Deployment triggers on push to main
- **WHEN** a commit is pushed to the `main` branch
- **THEN** the GitHub Actions workflow `deploy-docs.yml` triggers, builds the site, and deploys it to the `gh-pages` branch

#### Scenario: Build failures block deployment
- **WHEN** the Docusaurus build step exits with a non-zero code
- **THEN** the deployment step does not run and the workflow is marked failed

### Requirement: Admin Provisioning Guide
The documentation site SHALL include a comprehensive Admin Provisioning Guide in the Getting Started section that covers the full end-to-end workflow from PlatformAdmin bootstrap through tenant creation, user management, role assignment, organizational hierarchy, and configuration.

#### Scenario: Guide is accessible from navigation
- **WHEN** a user opens the documentation site and navigates to Getting Started
- **THEN** the Admin Provisioning Guide SHALL appear in the sidebar navigation alongside the CLI Quick Reference and Tenant Admin Control Plane guides

#### Scenario: Guide covers full provisioning workflow
- **WHEN** a user reads the Admin Provisioning Guide
- **THEN** the guide SHALL document PlatformAdmin bootstrap (env vars, startup behavior), CLI profile setup, GitHub device-code authentication, tenant creation, user registration and invitation, role assignment and hierarchy, tenant configuration and secrets, repository binding, shared Git provider connections, manifest-based provisioning, permission inspection, and REST API equivalents for all CLI operations

#### Scenario: Guide uses only generic examples
- **WHEN** example values appear in the Admin Provisioning Guide
- **THEN** all examples SHALL use generic placeholder values (e.g. `acme-corp`, `alice@acme-corp.com`, `aeterna.example.com`) and SHALL NOT contain environment-specific hostnames, internal user identifiers, or real credentials

