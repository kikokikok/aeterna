# §2.2 Follow-up Findings: the remaining gaps are forward-path gaps

**Context.** PR #128 closed the easy half of §2.2:
- Added a `list_domain_mappings` reader and wired it into the manifest
  renderer so `domainMappings` round-trips.
- Removed `secrets` from `NOT_RENDERED_SECTIONS` with an explanatory doc:
  `secrets` is wire-only input and, by security design, must never be
  round-tripped through GET (plaintext would be exfiltrated).

That leaves three entries in `NOT_RENDERED_SECTIONS`: `hierarchy`,
`roles`, `providers`. While scoping what it would take to close each of
them, I discovered they are **not reverse-renderer gaps — they are
forward-path (apply) gaps**. Documenting the findings here so a future
PR planner doesn't re-do the investigation.

## Finding 1 — `providers`: validated but never persisted

Path through the code:

- `cli/src/server/tenant_api.rs::validate_manifest` (line 3710) walks
  `manifest.providers.{llm,embedding,memory_layers}` and checks `kind`
  non-empty + `secret_ref` resolves in `config.secretReferences`.
- The dry-run `ProvisionPlan` exposes
  `has_providers: manifest.providers.is_some()` (line 4190).
- **But nowhere on the apply path does anything read `manifest.providers`
  and write it to storage.** The LLM/embedding provider state visible
  via `GET /admin/tenants/{tenant}/providers` is populated by the
  dedicated `PUT /admin/tenants/{tenant}/providers/llm` /
  `PUT .../providers/embedding` handlers, which write
  `config_keys::LLM_*` / `EMBEDDING_*` fields directly into
  `tenant_config` (see `memory/src/provider_registry.rs::config_keys`).

**Observed consequence.** A user who submits
`manifest.providers.llm = { kind: "openai", model: "gpt-4" }` and calls
`provision_tenant`, then calls `GET /.../manifest`, sees no `providers`
block. The submitted data was validated and silently dropped.

**What closure requires.** `provision_tenant` must gain an apply path
that translates `ManifestProvider` into `config_keys::*` writes,
mirroring what the dedicated `PUT .../providers/llm` endpoint does
today. Only then can a reverse renderer producing `ManifestProviders`
from `config.fields` round-trip.

Recommended breakdown: **§2.2-A — providers forward-apply parity.**
`memoryLayers` is a separate sub-finding (see Finding 4 below) since
even its forward-apply path has no config-key convention yet.

## Finding 2 — `hierarchy`: no backing storage

Path through the code:

- `TenantManifest.hierarchy: Option<Vec<ManifestCompany>>` expects
  `ManifestCompany { name, orgs }`, `ManifestOrg { name, teams, members }`,
  `ManifestTeam { name, members }`, `ManifestMember { user_id, role }`.
- **There are no `companies`, `organizations`, or `teams` tables in the
  schema.** `grep -rn 'CREATE TABLE' storage/src` produces zero hits
  for these names.
- `company_id`, `org_id`, `team_id` appear only as free-floating `UUID`
  columns on `governance_roles` with no FK to any entity table. They
  are opaque scoping identifiers, not pointers into a populated
  hierarchy.

**Observed consequence.** There is no current state to reverse-render,
and correspondingly the apply path has nowhere to write the
`manifest.hierarchy` data either. This is an unimplemented feature,
not a missing renderer.

**What closure requires.**
1. Storage model: `companies`, `organizations`, `teams`, `team_members`
   tables scoped to `tenant_id`.
2. Apply-path wiring in `provision_tenant` to upsert
   companies → orgs → teams → members from `manifest.hierarchy`,
   idempotently (key on `(tenant_id, name)` at each level).
3. Then — and only then — a reader
   (`list_hierarchy(tenant_id) -> Vec<ManifestCompany>`) followed by
   reverse-rendering.

Recommended breakdown: **§2.2-B — hierarchy storage + apply.**

## Finding 3 — `roles`: sequenced after hierarchy

Path through the code:

- `governance::list_roles(company_id, org_id, team_id) -> Vec<GovernanceRole>`
  exists and already filters by scope UUIDs.
- But without an enumerable hierarchy (Finding 2), there is no way to
  compute the set of `(company_id, org_id, team_id)` tuples belonging
  to a tenant, so the fan-out target is unknown.
- Additionally, `ManifestRoleAssignment.unit` is "hierarchy unit name
  or ID"; the renderer needs hierarchy-resolved names to emit the
  friendly form.

**Observed consequence.** Rendering roles independently would produce
either tenant-id-only rows (no scope) or raw-UUID scopes (no names),
both of which fail the round-trip test against a manifest-authored
input.

Recommended breakdown: **§2.2-C — roles reverse-render**, done after
§2.2-B in the same work stream (not a cross-team dependency — just
ordering inside this engineer's queue).

## Finding 4 — `providers.memoryLayers`: no storage convention at all

Even if Finding 1 is closed for `llm` and `embedding`, the
`providers.memoryLayers: BTreeMap<String, ManifestProvider>` sub-block
has no config-key convention in the codebase. `grep -rn MEMORY_LAYER`
turns up only the `MemoryLayer` enum used for memory-record provenance
(`Session`/`Project`/`Team`/`Org`/`Company`), not a configurable
per-layer provider.

**What closure requires.** Pick a key-prefix convention
(e.g. `memory_layer.{name}.kind`, `.model`, `.secret_ref`, ...),
update forward apply AND reverse render.

Recommended breakdown: **§2.2-D — `providers.memoryLayers` storage
convention.**

## Task graph

```
§2.2-A  providers llm/embedding forward-apply   (independent)
§2.2-B  hierarchy storage + apply               (independent, biggest)
§2.2-C  roles reverse-render                    (done after §2.2-B)
§2.2-D  memoryLayers storage convention         (independent)
```

Closing all four unblocks §2.4 (structural diff endpoint) and §7.3
(`aeterna tenant diff` CLI). Until then, the four entries should stay
in `NOT_RENDERED_SECTIONS` (the test fixture in #128 intentionally
allows this as a parity backstop — each closure reduces that set by
exactly one).
