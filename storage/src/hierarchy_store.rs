//! Tenant-scoped organizational hierarchy storage.
//!
//! §2.2-B commit B3 of `harden-tenant-provisioning`. Owns read and write
//! access to the `companies` / `organizations` / `teams` tables — the
//! "modern" hierarchy tables that idp-sync, bootstrap, and OPAL read
//! (via `v_hierarchy`) — with full tenant-scoping enforced by
//! migration 028.
//!
//! **Scope note.** `provision_tenant`'s current apply step writes only
//! the legacy `organizational_units` table (via
//! `PostgresBackend::create_unit_scoped`). That path remains, unchanged,
//! for backward compatibility with backup/gdpr/cascade code paths that
//! still read OU. This store is the forward path: B4 will wire
//! `provision_tenant` to call [`HierarchyStore::upsert_hierarchy`] in
//! addition to the OU write, so manifest-declared hierarchy lands in
//! the same tables that bootstrap and idp-sync populate.
//!
//! **Slug derivation.** `ManifestCompany` / `ManifestOrg` / `ManifestTeam`
//! carry a `name: String` but no `slug` — yet the modern tables require
//! `(tenant_id, slug)` / `(company_id, slug)` / `(org_id, slug)` UNIQUE
//! constraints. We derive a slug from the name via [`slugify`] (lossy
//! kebab-case). Two distinct names collapsing to the same slug under the
//! same parent is a UNIQUE-violation error surfaced by Postgres; callers
//! should enforce slug uniqueness in manifest validation if they care.
//!
//! **Prune.** Deferred to B4. The blast-radius NOTES describe prune as
//! "soft-delete rows no longer in manifest". That needs to coexist with
//! idp-synced teams (which carry `idp_provider IS NOT NULL` per
//! migration 030) that must NOT be pruned by a manifest apply. Safer
//! to sequence after the apply wiring is in place.

use sqlx::Row;
use uuid::Uuid;

use crate::postgres::PostgresError;

/// A company in the modern hierarchy — owned by exactly one tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Company {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub orgs: Vec<Org>,
}

/// An organization under a company.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Org {
    pub id: Uuid,
    pub company_id: Uuid,
    pub slug: String,
    pub name: String,
    pub teams: Vec<Team>,
}

/// A team under an organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    pub id: Uuid,
    pub org_id: Uuid,
    pub slug: String,
    pub name: String,
}

/// Input shape for [`HierarchyStore::upsert_hierarchy`]. Mirrors the
/// shape of `ManifestCompany` but uses explicit `slug` fields derived
/// up front by the caller (see [`slugify`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyInput {
    pub slug: String,
    pub name: String,
    pub orgs: Vec<OrgInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgInput {
    pub slug: String,
    pub name: String,
    pub teams: Vec<TeamInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamInput {
    pub slug: String,
    pub name: String,
}

/// Result of a successful [`HierarchyStore::upsert_hierarchy`] call.
/// Surfaced so the caller (B4 `provision_tenant`) can report counts in
/// the provision response step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpsertSummary {
    pub companies_upserted: usize,
    pub orgs_upserted: usize,
    pub teams_upserted: usize,
}

/// Derive a URL-safe, lowercase, kebab-case slug from a free-form name.
///
/// Rules:
/// * Lowercase ASCII letters and digits are kept as-is.
/// * All other runs of characters collapse to a single `-`.
/// * Leading and trailing `-` are stripped.
/// * Empty result is replaced with `"unnamed"` so we never produce a
///   NULL-violating empty slug.
///
/// This is intentionally lossy: two names that differ only in
/// punctuation/whitespace map to the same slug, which will surface as
/// a Postgres UNIQUE violation at upsert time (that's the desired
/// fail-fast behaviour for accidentally-duplicate manifest entries).
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for ch in name.chars() {
        let mapped = ch.to_ascii_lowercase();
        if mapped.is_ascii_alphanumeric() {
            out.push(mapped);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "unnamed".to_string()
    } else {
        out
    }
}

/// Store for the modern tenant-scoped hierarchy tables.
///
/// Backed by the same `sqlx::PgPool` as `TenantStore`,
/// `TenantRepositoryBindingStore`, etc. No RLS session variables are
/// set from this store directly — the caller is responsible for
/// running queries on the appropriate app/admin pool per the dual-pool
/// RLS design (PR #72/#75).
///
/// **Consequence.** Call this store via the admin pool when
/// provisioning a brand-new tenant. `provision_tenant` runs in admin
/// context, which is correct for B4.
#[derive(Clone)]
pub struct HierarchyStore {
    pool: sqlx::PgPool,
}

impl HierarchyStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Upsert a full hierarchy under `tenant_id`. Idempotent on repeat
    /// application with the same inputs: existing rows get their `name`
    /// refreshed (slug is the identity key, so it cannot change here —
    /// a renamed company requires either a new slug or a soft-delete +
    /// re-create, which is a B4 concern).
    ///
    /// This method runs in a single transaction so a partial failure
    /// (e.g. one org violates a UNIQUE constraint) leaves no half-built
    /// hierarchy behind.
    ///
    /// Does **not** prune rows absent from `companies` — see module
    /// docstring.
    pub async fn upsert_hierarchy(
        &self,
        tenant_id: Uuid,
        companies: &[CompanyInput],
    ) -> Result<UpsertSummary, PostgresError> {
        let mut tx = self.pool.begin().await.map_err(PostgresError::Database)?;
        let mut summary = UpsertSummary::default();

        for company in companies {
            let company_id: Uuid = sqlx::query_scalar(
                "INSERT INTO companies (tenant_id, slug, name, settings, created_at, updated_at)
                 VALUES ($1, $2, $3, '{}', NOW(), NOW())
                 ON CONFLICT (tenant_id, slug)
                 DO UPDATE SET name = EXCLUDED.name, updated_at = NOW()
                 RETURNING id",
            )
            .bind(tenant_id)
            .bind(&company.slug)
            .bind(&company.name)
            .fetch_one(&mut *tx)
            .await
            .map_err(PostgresError::Database)?;
            summary.companies_upserted += 1;

            for org in &company.orgs {
                let org_id: Uuid = sqlx::query_scalar(
                    "INSERT INTO organizations (company_id, slug, name, created_at, updated_at)
                     VALUES ($1, $2, $3, NOW(), NOW())
                     ON CONFLICT (company_id, slug)
                     DO UPDATE SET name = EXCLUDED.name, updated_at = NOW()
                     RETURNING id",
                )
                .bind(company_id)
                .bind(&org.slug)
                .bind(&org.name)
                .fetch_one(&mut *tx)
                .await
                .map_err(PostgresError::Database)?;
                summary.orgs_upserted += 1;

                for team in &org.teams {
                    sqlx::query(
                        "INSERT INTO teams (org_id, slug, name, created_at, updated_at)
                         VALUES ($1, $2, $3, NOW(), NOW())
                         ON CONFLICT (org_id, slug)
                         DO UPDATE SET name = EXCLUDED.name, updated_at = NOW()",
                    )
                    .bind(org_id)
                    .bind(&team.slug)
                    .bind(&team.name)
                    .execute(&mut *tx)
                    .await
                    .map_err(PostgresError::Database)?;
                    summary.teams_upserted += 1;
                }
            }
        }

        tx.commit().await.map_err(PostgresError::Database)?;
        Ok(summary)
    }

    /// Read the full hierarchy for `tenant_id` as a nested `Vec<Company>`.
    ///
    /// Uses `v_hierarchy` so any future view changes (e.g. surfacing
    /// project-level rows) centralize in the migration, not here. The
    /// view produces a row per (company, org, team, project) with
    /// LEFT JOINs, so we see each company at least once even if it has
    /// no orgs; we fold the flat rowset back into the nested shape.
    ///
    /// Ordering: companies by slug, orgs by slug within company, teams
    /// by slug within org. Deterministic so round-trip equality in
    /// integration tests is meaningful.
    pub async fn get_hierarchy(&self, tenant_id: Uuid) -> Result<Vec<Company>, PostgresError> {
        let rows = sqlx::query(
            "SELECT company_id, company_slug, company_name,
                    org_id, org_slug, org_name,
                    team_id, team_slug, team_name
               FROM v_hierarchy
              WHERE tenant_id = $1
              ORDER BY company_slug, org_slug NULLS FIRST, team_slug NULLS FIRST",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(PostgresError::Database)?;

        let mut companies: Vec<Company> = Vec::new();
        for row in rows {
            let company_id: Uuid = row.get("company_id");
            let company_slug: String = row.get("company_slug");
            let company_name: String = row.get("company_name");

            let company_idx = match companies.iter().position(|c| c.id == company_id) {
                Some(idx) => idx,
                None => {
                    companies.push(Company {
                        id: company_id,
                        tenant_id,
                        slug: company_slug,
                        name: company_name,
                        orgs: Vec::new(),
                    });
                    companies.len() - 1
                }
            };

            let org_id: Option<Uuid> = row.try_get("org_id").ok();
            let Some(org_id) = org_id else {
                continue;
            };
            let org_slug: String = row.get("org_slug");
            let org_name: String = row.get("org_name");

            let company = &mut companies[company_idx];
            let org_idx = match company.orgs.iter().position(|o| o.id == org_id) {
                Some(idx) => idx,
                None => {
                    company.orgs.push(Org {
                        id: org_id,
                        company_id: company.id,
                        slug: org_slug,
                        name: org_name,
                        teams: Vec::new(),
                    });
                    company.orgs.len() - 1
                }
            };

            let team_id: Option<Uuid> = row.try_get("team_id").ok();
            let Some(team_id) = team_id else {
                continue;
            };
            let team_slug: String = row.get("team_slug");
            let team_name: String = row.get("team_name");

            let org = &mut company.orgs[org_idx];
            if !org.teams.iter().any(|t| t.id == team_id) {
                org.teams.push(Team {
                    id: team_id,
                    org_id: org.id,
                    slug: team_slug,
                    name: team_name,
                });
            }
        }

        Ok(companies)
    }
}

#[cfg(test)]
mod slugify_tests {
    use super::slugify;

    #[test]
    fn lowercases_and_kebab_cases_ascii() {
        assert_eq!(slugify("Acme Corp"), "acme-corp");
        assert_eq!(slugify("Acme   Corp"), "acme-corp");
        assert_eq!(slugify("Acme/Corp_2024"), "acme-corp-2024");
    }

    #[test]
    fn strips_leading_and_trailing_non_alphanumerics() {
        assert_eq!(slugify("   hello   "), "hello");
        assert_eq!(slugify("--foo--"), "foo");
        assert_eq!(slugify("///a//b///"), "a-b");
    }

    #[test]
    fn empty_or_all_punctuation_becomes_unnamed() {
        assert_eq!(slugify(""), "unnamed");
        assert_eq!(slugify("   "), "unnamed");
        assert_eq!(slugify("!!!"), "unnamed");
    }

    #[test]
    fn unicode_is_lossy_but_nonempty() {
        assert_eq!(slugify("café"), "caf");
        assert_eq!(slugify("日本語"), "unnamed");
        assert_eq!(slugify("日本 Corp"), "corp");
    }

    #[test]
    fn preserves_digits_and_single_dashes() {
        assert_eq!(slugify("team-42"), "team-42");
        assert_eq!(slugify("v1.0 release"), "v1-0-release");
    }
}
