#!/usr/bin/env bash
# cleanup-tenant.sh
#
# Idempotently delete a tenant and all its dependent rows from the Aeterna
# Postgres database. Use this when:
#   * a `tenant apply` left half-provisioned state behind
#   * an operator wants to reclaim a slug that was used in a failed test
#   * #RC7 ops runbook step "hard reset before re-apply"
#
# Usage:
#   ./scripts/cleanup-tenant.sh <tenant-slug>
#
# Required env:
#   DATABASE_URL    — full Postgres URL the same migrations connect to
#                     e.g. postgres://aeterna:***@db:5432/aeterna
#
# Behaviour:
#   * Wraps everything in a single transaction. If any step fails, nothing
#     is deleted. Re-runnable on already-deleted tenants (no-op).
#   * Deletes in FK-safe order even though most child tables have
#     ON DELETE CASCADE — explicit deletes make the audit trail readable.
#   * Leaves users / user_roles untouched: they may belong to other tenants.
#
# Exit codes:
#   0  cleanup succeeded (or tenant did not exist)
#   1  usage error (missing slug or DATABASE_URL)
#   2  database error (rolled back, see psql output)

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <tenant-slug>" >&2
    exit 1
fi

slug="$1"

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "error: DATABASE_URL must be set" >&2
    exit 1
fi

echo ">> resolving tenant '${slug}'..."
tenant_id=$(psql "$DATABASE_URL" -At -c \
    "SELECT id FROM tenants WHERE slug = '${slug}' LIMIT 1" 2>/dev/null || true)

if [[ -z "$tenant_id" ]]; then
    echo ">> tenant '${slug}' not found — nothing to do"
    exit 0
fi

echo ">> tenant_id = ${tenant_id}"
echo ">> deleting dependent rows + tenant in a single transaction..."

psql "$DATABASE_URL" --set ON_ERROR_STOP=1 <<SQL || { echo "database error — see above" >&2; exit 2; }
BEGIN;

-- Children that don't have ON DELETE CASCADE on tenant_id (or where we
-- prefer explicit deletion for the audit trail).
DELETE FROM tenant_secrets             WHERE tenant_id = '${tenant_id}';
DELETE FROM tenant_manifest_state      WHERE tenant_id = '${tenant_id}';
DELETE FROM organizational_units       WHERE tenant_id = '${tenant_id}';
DELETE FROM tenant_repository_bindings WHERE tenant_id = '${tenant_id}';
DELETE FROM tenant_domain_mappings     WHERE tenant_id = '${tenant_id}';

-- Finally the tenant row itself. Other FK chains (e.g. user_roles) cascade.
DELETE FROM tenants WHERE id = '${tenant_id}';

COMMIT;
SQL

echo ">> tenant '${slug}' fully removed"
