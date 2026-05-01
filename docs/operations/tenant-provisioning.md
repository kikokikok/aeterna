# Tenant provisioning operations runbook

This runbook covers the day-2 operations side of `aeterna tenant apply`:
what to do when provisioning fails partway, how to interpret the new
exit codes, and how to recover safely.

Applies to **0.8.0-rc.7** and later.

---

## 1. Exit codes for `aeterna tenant apply`

| code | meaning                                            | next step                                |
|------|----------------------------------------------------|------------------------------------------|
| 0    | Applied, or Unchanged (no-op)                      | nothing — desired state matches reality |
| 1    | Validation, generation conflict, or transport error| read stderr; usually a manifest issue    |
| 2    | **Partial** — some steps succeeded, others failed  | this runbook section 3                   |

Pipelines must distinguish 1 from 2 — a Partial outcome is recoverable
and re-applying the same manifest is safe (see §2).

```bash
set +e
aeterna tenant apply -f tenant.yaml
rc=$?
set -e
case "$rc" in
    0) echo "ok" ;;
    2) echo "partial — retrying after 5s"; sleep 5; aeterna tenant apply -f tenant.yaml ;;
    *) echo "hard fail"; exit "$rc" ;;
esac
```

## 2. Idempotency guarantees

Migration `031_provisioning_idempotency` adds two unique constraints that
make `tenant apply` a true no-op when desired state already matches:

* `organizational_units (tenant_id, COALESCE(parent_id,''), name)` —
  re-applying a manifest after a partial failure no longer inserts
  duplicate root units. The second apply upserts metadata + updated_at
  and leaves id / created_at intact.
* `tenant_domain_mappings (lower(domain)) WHERE verified = true` —
  two tenants can no longer both verify the same email domain.
  Domain → tenant resolution is deterministic again.

### Brownfield migration may fail

If an existing cluster already has duplicate rows (likely on systems that
ran any pre-rc.7 build against shared data) the migration will fail at
`CREATE UNIQUE INDEX`. Check for offenders before applying:

```sql
-- duplicate (tenant_id, parent_id, name) tuples in organizational_units
SELECT tenant_id, COALESCE(parent_id, '') AS parent, name, COUNT(*)
FROM organizational_units
GROUP BY 1, 2, 3
HAVING COUNT(*) > 1;

-- two tenants verifying the same domain
SELECT lower(domain), array_agg(tenant_id) AS tenants
FROM tenant_domain_mappings
WHERE verified = true
GROUP BY 1
HAVING COUNT(*) > 1;
```

Resolve the duplicates manually, then re-run the migration job.

## 3. Recovering from a Partial outcome

1. **Read the response body.** `tenant apply` prints a per-step status
   table; identify which steps reported `failed` vs `applied`.
2. **Fix the underlying cause.** Most common: missing secret, wrong
   credential kind, network blip. Read the error message verbatim —
   `CredentialKind` errors now use the canonical PascalCase variants
   (e.g. `GitHubApp`, not `github_app`).
3. **Re-apply.** With migration 031 in place, re-running the same
   manifest is safe — successful steps short-circuit on the unique
   indexes, failed steps retry.

If a partial apply left so much state behind that an operator wants to
start over (rare), use `scripts/cleanup-tenant.sh`:

```bash
DATABASE_URL=postgres://aeterna:***@db:5432/aeterna \
    ./scripts/cleanup-tenant.sh acme-corp
```

The script wraps everything in a single transaction — if any DELETE
fails the whole cleanup is rolled back. Safe to run on a tenant that
doesn't exist (no-op).

## 4. Common 422s on apply

| symptom                                                  | cause                                  | fix                                          |
|----------------------------------------------------------|----------------------------------------|----------------------------------------------|
| `inline_secret_not_allowed`                              | manifest contains plaintext secret     | move to `secretRef` + `aeterna secret put`   |
| `generation_conflict`                                    | concurrent edit, your generation stale | re-fetch with `tenant render`, edit, re-apply|
| `validation_failed: credential_kind ...`                 | invalid `credentialKind` value         | use one of: `None`, `Pat`, `SshKey`, `GitHubApp` (PascalCase) |
| `validation_failed: role ...`                            | unknown role                           | use one of: `Viewer`, `Developer`, `TechLead`, `Architect`, `Admin`, `TenantAdmin`, `Agent`, `PlatformAdmin` |

## 5. See also

* `storage/migrations/031_provisioning_idempotency.sql` — the schema change
* `scripts/cleanup-tenant.sh` — hard-reset helper
* `docs/architecture/tenant-manifest.md` — the manifest shape and wire-format conventions
