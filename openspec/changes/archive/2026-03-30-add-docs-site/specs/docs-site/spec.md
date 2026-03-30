## ADDED Requirements

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
