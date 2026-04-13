-- Migration 020: Fix codesearch views
-- The v_code_search_requests view in migration 014 incorrectly referenced a
-- non-existent tenant_id column on codesearch_requests. It should join with
-- codesearch_repositories to obtain the tenant_id.

CREATE OR REPLACE VIEW v_code_search_requests AS
SELECT
    r.id,
    r.repository_id,
    r.requester_id,
    r.status,
    repo.tenant_id
FROM codesearch_requests r
JOIN codesearch_repositories repo ON repo.id = r.repository_id;

COMMENT ON VIEW v_code_search_requests IS 'OPAL view: Code Search requests for Cedar entities.';
