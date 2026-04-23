-- Migration 030: external_id + idp_provider on modern hierarchy tables
--
-- Closes the last remaining #130 follow-up: migrate idp-sync writes off
-- the legacy `organizational_units` table and onto
-- `companies` / `organizations` / `teams` (introduced in migration 009).
--
-- This migration adds the columns idp-sync needs to do ON CONFLICT
-- upserts by external identity. It is schema-only and non-destructive.
-- The behavioral cut-over (idp-sync rewriting its INSERTs, role_grants
-- reading from the new tables, bridge_sync_to_governance joining via
-- `teams` directly) ships in the same PR as application-layer code.
--
-- Why the columns live here, not in idp-sync::initialize_github_sync_schema:
--   Prior to PR #131 the idp-sync schema init block raced the storage
--   migration tree by re-defining views with CREATE OR REPLACE. #131
--   relocated `v_hierarchy` + `v_user_permissions`, #132 relocated
--   `v_agent_permissions`, and this migration completes the pattern for
--   the idp columns on the modern hierarchy.
--
-- Uniqueness model:
--   - companies: UNIQUE (tenant_id, external_id, idp_provider) where both
--     NOT NULL. A tenant may have at most one company per external IdP id.
--   - organizations: UNIQUE (company_id, external_id, idp_provider) where
--     both NOT NULL. Scoped to the parent company (matches how
--     `organizations.slug` is scoped today).
--   - teams: UNIQUE (org_id, external_id, idp_provider) where both NOT
--     NULL. Scoped to the parent org (matches `teams.slug`).
--
-- Null semantics: rows provisioned locally (not via an IdP) have NULL
-- external_id + NULL idp_provider and are excluded from the partial
-- unique indexes, so they cannot collide with each other.

-- ---------------------------------------------------------------------
-- companies
-- ---------------------------------------------------------------------

ALTER TABLE companies
    ADD COLUMN IF NOT EXISTS external_id TEXT,
    ADD COLUMN IF NOT EXISTS idp_provider TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_companies_tenant_external_provider
    ON companies (tenant_id, external_id, idp_provider)
    WHERE external_id IS NOT NULL
      AND idp_provider IS NOT NULL
      AND deleted_at IS NULL;

-- ---------------------------------------------------------------------
-- organizations
-- ---------------------------------------------------------------------

ALTER TABLE organizations
    ADD COLUMN IF NOT EXISTS external_id TEXT,
    ADD COLUMN IF NOT EXISTS idp_provider TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_organizations_company_external_provider
    ON organizations (company_id, external_id, idp_provider)
    WHERE external_id IS NOT NULL
      AND idp_provider IS NOT NULL
      AND deleted_at IS NULL;

-- ---------------------------------------------------------------------
-- teams
-- ---------------------------------------------------------------------

ALTER TABLE teams
    ADD COLUMN IF NOT EXISTS external_id TEXT,
    ADD COLUMN IF NOT EXISTS idp_provider TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_teams_org_external_provider
    ON teams (org_id, external_id, idp_provider)
    WHERE external_id IS NOT NULL
      AND idp_provider IS NOT NULL
      AND deleted_at IS NULL;
