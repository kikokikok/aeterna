## ADDED Requirements

### Requirement: Tenants Table

The storage layer SHALL maintain a `tenants` table for multi-tenant identity resolution.

The table MUST have:
- `id UUID PRIMARY KEY DEFAULT gen_random_uuid()`
- `name TEXT NOT NULL UNIQUE`
- `created_at TIMESTAMPTZ DEFAULT NOW()`

#### Scenario: Tenant resolution by name
- **WHEN** the system resolves a tenant by name (e.g., `AETERNA_TENANT_ID=default`)
- **THEN** it SHALL query `SELECT id FROM tenants WHERE name = $1`
- **AND** if not found, SHALL create a new tenant with a generated UUID

#### Scenario: Idempotent tenant creation
- **WHEN** tenant creation is called twice with the same name
- **THEN** the second call SHALL return the existing tenant's UUID
- **AND** no duplicate entry SHALL be created

### Requirement: Agents Table

The storage layer SHALL maintain an `agents` table for AI agent identity and delegation tracking.

The table MUST have:
- `id UUID PRIMARY KEY DEFAULT gen_random_uuid()`
- `name TEXT NOT NULL`
- `agent_type TEXT NOT NULL DEFAULT 'coding-assistant'`
- `delegated_by_user_id UUID REFERENCES users(id)`
- `delegated_by_agent_id UUID REFERENCES agents(id)`
- `delegation_depth INT NOT NULL DEFAULT 0`
- `capabilities JSONB DEFAULT '[]'`
- `allowed_company_ids UUID[]`, `allowed_org_ids UUID[]`, `allowed_team_ids UUID[]`, `allowed_project_ids UUID[]`
- `status TEXT NOT NULL DEFAULT 'active'`
- `created_at TIMESTAMPTZ DEFAULT NOW()`
- `updated_at TIMESTAMPTZ DEFAULT NOW()`

#### Scenario: Agent with user delegation
- **WHEN** an agent is created with `delegated_by_user_id` set
- **THEN** the agent SHALL inherit the delegating user's permissions
- **AND** the `delegation_depth` SHALL be 0 (direct delegation)

#### Scenario: Agent-to-agent delegation chain
- **WHEN** an agent is delegated by another agent
- **THEN** `delegation_depth` SHALL increment by 1 from the delegating agent
- **AND** the delegation chain SHALL be traceable via `delegated_by_agent_id`

### Requirement: Schema Initialization Order

The storage initialization function (`initialize_github_sync_schema`) SHALL create ALL required tables in the correct dependency order before any queries are executed.

The creation order MUST be:
1. Extensions (`pgcrypto`)
2. `tenants` table
3. `users` table
4. `agents` table
5. `memberships` table
6. `idp_group_mappings` table
7. `organizational_units` columns (`external_id`, `idp_provider`, `slug`)
8. Indexes

#### Scenario: Fresh database initialization
- **WHEN** `initialize_github_sync_schema` is called on a fresh database
- **THEN** all tables SHALL be created successfully
- **AND** the function SHALL be idempotent (safe to call multiple times)

#### Scenario: Existing database re-initialization
- **WHEN** `initialize_github_sync_schema` is called on a database with existing tables
- **THEN** no errors SHALL occur
- **AND** existing data SHALL NOT be modified or deleted

## MODIFIED Requirements

### Requirement: PostgreSQL Implementation
The system SHALL implement a PostgreSQL backend for structured data storage.

The PostgreSQL backend MUST include:
- Authorization views (`v_hierarchy`, `v_user_permissions`, `v_agent_permissions`)
- Code search views (`v_code_search_repositories`, `v_code_search_requests`, `v_code_search_identities`)
- PG NOTIFY triggers on `users`, `memberships`, `organizational_units`, `governance_roles`, `agents`
- The `organizational_units` table MUST include a `slug TEXT` column

#### Scenario: Initialize PostgreSQL connection
- **WHEN** system starts
- **THEN** backend SHALL initialize PostgreSQL client using sqlx
- **AND** backend SHALL create connection pool with deadpool
- **AND** backend SHALL run health check

#### Scenario: Create schema for episodic memories
- **WHEN** creating episodic memory table
- **THEN** system SHALL create table with fields: id, content, layer, identifiers, metadata, createdAt, updatedAt
- **AND** system SHALL add indexes on layer and timestamps

#### Scenario: Authorization views exist
- **WHEN** the OPAL fetcher queries the authorization views
- **THEN** `v_hierarchy`, `v_user_permissions`, and `v_agent_permissions` SHALL return valid rows
- **AND** all UUID columns SHALL be properly typed
