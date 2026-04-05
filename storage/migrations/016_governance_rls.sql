CREATE OR REPLACE FUNCTION current_app_company_id()
RETURNS uuid AS $$
    SELECT NULLIF(current_setting('app.company_id', true), '')::uuid
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION scope_belongs_to_current_company(
    p_company_id uuid,
    p_org_id uuid,
    p_team_id uuid,
    p_project_id uuid
)
RETURNS boolean AS $$
    SELECT CASE
        WHEN current_app_company_id() IS NULL THEN FALSE
        WHEN p_company_id IS NOT NULL THEN p_company_id = current_app_company_id()
        WHEN p_org_id IS NOT NULL THEN EXISTS (
            SELECT 1
            FROM organizations o
            WHERE o.id = p_org_id
              AND o.company_id = current_app_company_id()
        )
        WHEN p_team_id IS NOT NULL THEN EXISTS (
            SELECT 1
            FROM teams t
            JOIN organizations o ON o.id = t.org_id
            WHERE t.id = p_team_id
              AND o.company_id = current_app_company_id()
        )
        WHEN p_project_id IS NOT NULL THEN EXISTS (
            SELECT 1
            FROM projects p
            JOIN teams t ON t.id = p.team_id
            JOIN organizations o ON o.id = t.org_id
            WHERE p.id = p_project_id
              AND o.company_id = current_app_company_id()
        )
        ELSE FALSE
    END
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION approval_request_belongs_to_current_company(
    p_request_id uuid
)
RETURNS boolean AS $$
    SELECT EXISTS (
        SELECT 1
        FROM approval_requests ar
        WHERE ar.id = p_request_id
          AND scope_belongs_to_current_company(
              ar.company_id,
              ar.org_id,
              ar.team_id,
              ar.project_id
          )
    )
$$ LANGUAGE sql STABLE;

ALTER TABLE governance_configs ENABLE ROW LEVEL SECURITY;
ALTER TABLE governance_configs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS governance_configs_company_isolation ON governance_configs;
CREATE POLICY governance_configs_company_isolation ON governance_configs
    FOR ALL
    USING (scope_belongs_to_current_company(company_id, org_id, team_id, project_id))
    WITH CHECK (scope_belongs_to_current_company(company_id, org_id, team_id, project_id));

ALTER TABLE approval_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE approval_requests FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS approval_requests_company_isolation ON approval_requests;
CREATE POLICY approval_requests_company_isolation ON approval_requests
    FOR ALL
    USING (scope_belongs_to_current_company(company_id, org_id, team_id, project_id))
    WITH CHECK (scope_belongs_to_current_company(company_id, org_id, team_id, project_id));

ALTER TABLE governance_roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE governance_roles FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS governance_roles_company_isolation ON governance_roles;
CREATE POLICY governance_roles_company_isolation ON governance_roles
    FOR ALL
    USING (scope_belongs_to_current_company(company_id, org_id, team_id, project_id))
    WITH CHECK (scope_belongs_to_current_company(company_id, org_id, team_id, project_id));

ALTER TABLE approval_decisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE approval_decisions FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS approval_decisions_company_isolation ON approval_decisions;
CREATE POLICY approval_decisions_company_isolation ON approval_decisions
    FOR ALL
    USING (approval_request_belongs_to_current_company(request_id))
    WITH CHECK (approval_request_belongs_to_current_company(request_id));

ALTER TABLE escalation_queue ENABLE ROW LEVEL SECURITY;
ALTER TABLE escalation_queue FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS escalation_queue_company_isolation ON escalation_queue;
CREATE POLICY escalation_queue_company_isolation ON escalation_queue
    FOR ALL
    USING (approval_request_belongs_to_current_company(request_id))
    WITH CHECK (approval_request_belongs_to_current_company(request_id));
