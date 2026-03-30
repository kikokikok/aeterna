## ADDED Requirements

### Requirement: Local Development Mode

The system SHALL support a local development mode using Docker Compose that includes the Aeterna binary alongside all dependencies.

#### Scenario: docker-compose up starts full stack locally
- **WHEN** a developer runs `docker compose up`
- **THEN** PostgreSQL, Qdrant, Redis, OPAL Server, Cedar Agent, OPAL Fetcher, and Aeterna SHALL all start
- **AND** the Aeterna service SHALL be accessible at `http://localhost:8080`
- **AND** MCP tools SHALL be functional without any cloud dependencies

#### Scenario: Local mode with hot reload
- **WHEN** a developer runs Aeterna in local development mode
- **THEN** code changes SHALL be detectable (via cargo-watch or similar)
- **AND** the developer SHALL be able to test MCP tools and API endpoints locally

### Requirement: Hybrid Deployment Architecture

The system SHALL define a clear split between components that run locally (on developer machines) and components that run in the cloud (shared infrastructure).

Local components:
- Memory system (agent/user/session layers)
- Context Architect
- Code search indexing and querying
- OpenCode plugin

Cloud components:
- Knowledge repository (shared Git-based store)
- Governance engine (Cedar/OPAL)
- Multi-tenant hierarchy
- IdP sync (GitHub/Okta/Azure AD)
- Observability aggregation

#### Scenario: Local agent with cloud knowledge
- **WHEN** an agent runs locally via OpenCode
- **THEN** memory operations (add/search/delete) SHALL operate against local storage
- **AND** knowledge queries SHALL be forwarded to the cloud Aeterna instance
- **AND** governance checks SHALL be forwarded to the cloud OPAL stack
