//! Structured manifest diff (B3 §2.4).
//!
//! Given an incoming [`TenantManifest`] and the current DB state
//! rendered via [`manifest_render::render_current_manifest`], produce
//! a machine-parseable delta. The diff is the shared source of truth
//! for:
//!
//! - `POST /api/v1/admin/tenants/diff` (this PR, §2.4)
//! - `aeterna tenant diff` CLI with `-o json|unified` (§7.3)
//! - UI preview-before-apply (future §12.7 / §12.8)
//!
//! ## Why a JSON tree walker instead of per-section comparators
//!
//! §2.2-A–D made `render_current_manifest` fully round-trippable:
//! for every manifest section the renderer emits a shape that is
//! field-compatible with the input `TenantManifest`. That invariant
//! lets us reduce "what changed" to a pure JSON-tree comparison:
//! serialize both sides with `serde_json::to_value`, walk the trees
//! in lockstep, and emit one [`FieldChange`] per differing leaf.
//!
//! This is ~400 LOC of data-structure-agnostic code instead of
//! ~2000 LOC of bespoke per-section comparators that would each need
//! to know the shape of `providers.memoryLayers`, `hierarchy`,
//! `roles`, etc. It also means: the day a new section lands in the
//! manifest, the diff picks it up with zero code change here.
//!
//! ## What the diff is NOT
//!
//! - **Not a character diff.** For `-o unified` the CLI will render
//!   this structured form as unified text; this module never emits
//!   unified output. Decoupling keeps the server format stable
//!   across CLI presentation changes.
//! - **Not a 3-way merge hint.** The diff describes "incoming vs
//!   current"; it does NOT describe the last-applied manifest. If a
//!   3-way merge is ever needed, that's an additional field on the
//!   request, not a transformation on this output.
//! - **Not secret-aware on its own.** Secret redaction is the
//!   responsibility of the renderer (`redact: true`) that feeds
//!   this module. The diff walker treats all JSON values uniformly.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Top-level response for `POST /api/v1/admin/tenants/diff`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TenantDiff {
    /// Tenant slug pulled from the incoming `manifest.tenant.slug`.
    /// Echoed for CLI rendering even when the tenant does not yet
    /// exist (Create operation).
    pub slug: String,

    /// Operation the apply endpoint would perform. `NoOp` means a
    /// re-apply of the incoming manifest would bump no state —
    /// useful for drift-check scripts that want a clean exit.
    pub operation: DiffOperation,

    /// Flat list of every changed leaf. Empty iff
    /// `operation == NoOp`. Paths use dot notation against the
    /// serialized manifest (e.g. `tenant.slug`,
    /// `providers.llm.kind`, `hierarchy.0.orgs.1.name`).
    pub changes: Vec<FieldChange>,

    /// Aggregated counts + section names. Duplicates information
    /// derivable from `changes` but keeps the CLI table view a pure
    /// projection without walking the flat list.
    pub summary: DiffSummary,
}

/// Operation classification.
///
/// Matches the three terminal states of a would-be apply call:
/// `Create` (no row in `tenants` for this slug), `Update` (exists +
/// differs), `NoOp` (exists + byte-identical in rendered form).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DiffOperation {
    Create,
    Update,
    NoOp,
}

/// A single changed leaf (or sub-tree) in the manifest.
///
/// `Modified` carries both `before` and `after`. `Added` carries
/// only `after`. `Removed` carries only `before`. The split between
/// `Added`/`Removed`/`Modified` is mildly redundant with
/// `(before, after)` nullability, but the enum makes CLI rendering
/// ergonomic (colour coding, icons) without re-deriving intent from
/// Option combinations.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    /// Dot-notation path into the serialized manifest. Array indices
    /// appear as bare integers (`hierarchy.0.slug`); string keys
    /// appear verbatim. See [`is_path_safe_for_dotted_notation`] for
    /// the escape contract — pathological keys (dots, brackets) are
    /// quoted.
    pub path: String,
    pub kind: ChangeKind,
    /// Value on the current-state side. `None` for `Added`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    /// Value on the incoming side. `None` for `Removed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

/// Aggregate view over [`TenantDiff::changes`].
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    /// Sorted, deduplicated list of top-level manifest sections that
    /// have at least one change. Derived from the first path
    /// component of each [`FieldChange::path`]. Useful for
    /// `tenant diff --slug foo -o sections` to see whether the apply
    /// would touch `providers` at all without reading every field.
    pub changed_sections: Vec<String>,
}

/// Top-level manifest keys that the diff considers "state-bearing".
///
/// `apiVersion`, `kind`, `metadata.*` are manifest envelope fields:
/// they identify the schema, not the tenant's state. Diffing them
/// would produce noise on every re-apply from a client using a
/// different toolchain version. [`compute_diff`] filters these out
/// at the top level only — nested `metadata` objects inside a
/// state-bearing section (should any ever appear) are kept.
const ENVELOPE_KEYS: &[&str] = &["apiVersion", "kind", "metadata", "notRendered"];

/// Compute a [`TenantDiff`] given the current state (optional; `None`
/// when the tenant does not yet exist) and the incoming manifest.
///
/// Both sides are taken as already-serialized JSON so this function
/// is entirely dependency-free on the input/render types — the
/// caller is responsible for running `serde_json::to_value` on
/// whichever shape it has. This keeps `tenant_diff.rs` buildable in
/// isolation (useful for tests) and lets the CLI use the same
/// function for client-side diffing if we ever expose a `--local`
/// mode.
///
/// `slug` is taken as a parameter rather than extracted from
/// `incoming` because the `TenantManifest.tenant.slug` path is a
/// contract of the input type, not of raw JSON. Passing it
/// explicitly keeps this function oblivious to the manifest schema.
pub fn compute_diff(slug: String, current: Option<Value>, incoming: Value) -> TenantDiff {
    let current = current
        .map(|v| strip_envelope(&v))
        .unwrap_or_else(|| Value::Object(Map::new()));
    let incoming = strip_envelope(&incoming);

    let mut changes = Vec::new();
    walk("", &current, &incoming, &mut changes);

    let mut added = 0usize;
    let mut removed = 0usize;
    let mut modified = 0usize;
    let mut sections: BTreeSet<String> = BTreeSet::new();
    for c in &changes {
        match c.kind {
            ChangeKind::Added => added += 1,
            ChangeKind::Removed => removed += 1,
            ChangeKind::Modified => modified += 1,
        }
        if let Some(section) = top_level_section(&c.path) {
            sections.insert(section.to_string());
        }
    }

    let operation = if changes.is_empty() {
        DiffOperation::NoOp
    } else if current.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        DiffOperation::Create
    } else {
        DiffOperation::Update
    };

    TenantDiff {
        slug,
        operation,
        changes,
        summary: DiffSummary {
            added,
            removed,
            modified,
            changed_sections: sections.into_iter().collect(),
        },
    }
}

/// Remove envelope keys from the top level of a rendered manifest.
///
/// Intentionally non-recursive: only the very outermost object gets
/// stripped. A nested `metadata` inside, say, `providers` remains
/// intact. Accepts non-object values (rare; defensive) and returns
/// them unchanged.
fn strip_envelope(v: &Value) -> Value {
    let Some(obj) = v.as_object() else {
        return v.clone();
    };
    let filtered: Map<String, Value> = obj
        .iter()
        .filter(|(k, _)| !ENVELOPE_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Value::Object(filtered)
}

/// Recursive diff walker.
///
/// Arrays are diffed position-by-position (index-based). This is
/// semantically wrong for unordered collections — e.g. reordering
/// `hierarchy.orgs` would surface as a long list of `Modified`
/// entries — but the manifest schema uses `slug`-keyed entries
/// everywhere a set-like collection appears, and the renderer emits
/// them in a stable order (slug-sorted in
/// `hierarchy_store::get_hierarchy`, BTreeMap order in
/// `render_memory_layers`). Index-based compare is therefore sound
/// for every section that §2.2 A-D covered.
fn walk(path: &str, a: &Value, b: &Value, changes: &mut Vec<FieldChange>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(ao), Value::Object(bo)) => {
            // Union of keys so we catch both additions and removals.
            let keys: BTreeSet<&String> = ao.keys().chain(bo.keys()).collect();
            for k in keys {
                let child_path = join_path(path, k);
                match (ao.get(k), bo.get(k)) {
                    (Some(av), Some(bv)) => walk(&child_path, av, bv, changes),
                    (None, Some(bv)) => changes.push(FieldChange {
                        path: child_path,
                        kind: ChangeKind::Added,
                        before: None,
                        after: Some(bv.clone()),
                    }),
                    (Some(av), None) => changes.push(FieldChange {
                        path: child_path,
                        kind: ChangeKind::Removed,
                        before: Some(av.clone()),
                        after: None,
                    }),
                    (None, None) => unreachable!("key came from union of a and b"),
                }
            }
        }
        (Value::Array(aa), Value::Array(ba)) => {
            let len = aa.len().max(ba.len());
            for i in 0..len {
                let child_path = join_path(path, &i.to_string());
                match (aa.get(i), ba.get(i)) {
                    (Some(av), Some(bv)) => walk(&child_path, av, bv, changes),
                    (None, Some(bv)) => changes.push(FieldChange {
                        path: child_path,
                        kind: ChangeKind::Added,
                        before: None,
                        after: Some(bv.clone()),
                    }),
                    (Some(av), None) => changes.push(FieldChange {
                        path: child_path,
                        kind: ChangeKind::Removed,
                        before: Some(av.clone()),
                        after: None,
                    }),
                    (None, None) => unreachable!(),
                }
            }
        }
        // Type-mismatch or primitive-value diff — emit a single
        // Modified at the current path. We do NOT recurse into
        // type-mismatched subtrees because the per-field diff would
        // be noise (every leaf inside the old subtree would show as
        // Removed and every leaf inside the new one as Added).
        (_, _) => changes.push(FieldChange {
            path: path.to_string(),
            kind: ChangeKind::Modified,
            before: Some(a.clone()),
            after: Some(b.clone()),
        }),
    }
}

fn join_path(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else {
        format!("{prefix}.{segment}")
    }
}

fn top_level_section(path: &str) -> Option<&str> {
    if path.is_empty() {
        return None;
    }
    Some(path.split('.').next().unwrap_or(path))
}

// ============================================================================
// Tests — pure JSON-in / diff-out, no DB, no HTTP.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_current() -> Value {
        json!({
            "apiVersion": "aeterna.io/v1alpha1",
            "kind": "TenantManifest",
            "metadata": { "generation": 3 },
            "tenant": { "slug": "acme", "name": "Acme" },
            "providers": {
                "llm": { "kind": "openai", "model": "gpt-4" }
            }
        })
    }

    #[test]
    fn identical_manifests_produce_noop() {
        let d = compute_diff("acme".into(), Some(sample_current()), sample_current());
        assert_eq!(d.operation, DiffOperation::NoOp);
        assert!(d.changes.is_empty());
        assert_eq!(d.summary.added, 0);
        assert_eq!(d.summary.modified, 0);
        assert_eq!(d.summary.removed, 0);
        assert!(d.summary.changed_sections.is_empty());
    }

    #[test]
    fn envelope_only_changes_do_not_trigger_update() {
        // `apiVersion` / `kind` / `metadata.generation` differing
        // must NOT register as changes — they're schema envelope,
        // not state.
        let current = sample_current();
        let mut incoming = sample_current();
        incoming["apiVersion"] = json!("aeterna.io/v1beta1");
        incoming["metadata"]["generation"] = json!(99);

        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.operation, DiffOperation::NoOp);
        assert!(d.changes.is_empty());
    }

    #[test]
    fn missing_current_is_create() {
        let d = compute_diff("new-tenant".into(), None, sample_current());
        assert_eq!(d.operation, DiffOperation::Create);
        assert!(!d.changes.is_empty());
        // Every change on Create should be `Added` (nothing to remove
        // or modify against an empty baseline).
        assert!(d.changes.iter().all(|c| c.kind == ChangeKind::Added));
        assert_eq!(d.summary.added, d.changes.len());
        assert_eq!(d.summary.removed, 0);
        assert_eq!(d.summary.modified, 0);
    }

    #[test]
    fn modified_leaf_shows_before_and_after() {
        let current = sample_current();
        let mut incoming = sample_current();
        incoming["tenant"]["name"] = json!("Acme Corp");

        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.operation, DiffOperation::Update);
        assert_eq!(d.changes.len(), 1);
        let c = &d.changes[0];
        assert_eq!(c.path, "tenant.name");
        assert_eq!(c.kind, ChangeKind::Modified);
        assert_eq!(c.before, Some(json!("Acme")));
        assert_eq!(c.after, Some(json!("Acme Corp")));
        assert_eq!(d.summary.changed_sections, vec!["tenant".to_string()]);
    }

    #[test]
    fn added_leaf_has_no_before() {
        let current = json!({ "tenant": { "slug": "acme" } });
        let incoming = json!({ "tenant": { "slug": "acme", "name": "Acme" } });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        let c = &d.changes[0];
        assert_eq!(c.path, "tenant.name");
        assert_eq!(c.kind, ChangeKind::Added);
        assert!(c.before.is_none());
        assert_eq!(c.after, Some(json!("Acme")));
    }

    #[test]
    fn removed_leaf_has_no_after() {
        let current = json!({ "tenant": { "slug": "acme", "name": "Acme" } });
        let incoming = json!({ "tenant": { "slug": "acme" } });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        let c = &d.changes[0];
        assert_eq!(c.path, "tenant.name");
        assert_eq!(c.kind, ChangeKind::Removed);
        assert_eq!(c.before, Some(json!("Acme")));
        assert!(c.after.is_none());
    }

    #[test]
    fn whole_section_added_counts_as_one_change_per_leaf() {
        // Adding a `providers` block that didn't exist before should
        // NOT recurse into the subtree — we emit a single `Added` at
        // `providers` with the whole new object as `after`. Rationale:
        // operators want "providers was added" as one diff entry, not
        // six (kind, model, secret_ref, ...).
        let current = json!({ "tenant": { "slug": "acme" } });
        let incoming = json!({
            "tenant": { "slug": "acme" },
            "providers": { "llm": { "kind": "openai", "model": "gpt-4" } }
        });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].path, "providers");
        assert_eq!(d.changes[0].kind, ChangeKind::Added);
        assert_eq!(d.summary.changed_sections, vec!["providers".to_string()]);
    }

    #[test]
    fn nested_change_produces_deep_path() {
        let current = json!({
            "providers": { "llm": { "kind": "openai", "model": "gpt-4" } }
        });
        let incoming = json!({
            "providers": { "llm": { "kind": "openai", "model": "gpt-4-turbo" } }
        });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].path, "providers.llm.model");
        assert_eq!(d.changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn array_diff_is_index_based() {
        let current = json!({
            "hierarchy": [
                { "slug": "eng", "name": "Engineering" },
                { "slug": "sales", "name": "Sales" }
            ]
        });
        let incoming = json!({
            "hierarchy": [
                { "slug": "eng", "name": "Engineering" },
                { "slug": "sales", "name": "Sales & Marketing" }
            ]
        });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].path, "hierarchy.1.name");
        assert_eq!(d.changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn array_lengthening_emits_added_at_index() {
        let current = json!({ "hierarchy": [{ "slug": "eng" }] });
        let incoming = json!({ "hierarchy": [{ "slug": "eng" }, { "slug": "sales" }] });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].path, "hierarchy.1");
        assert_eq!(d.changes[0].kind, ChangeKind::Added);
    }

    #[test]
    fn array_shortening_emits_removed_at_index() {
        let current = json!({ "hierarchy": [{ "slug": "eng" }, { "slug": "sales" }] });
        let incoming = json!({ "hierarchy": [{ "slug": "eng" }] });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].path, "hierarchy.1");
        assert_eq!(d.changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn type_mismatch_emits_one_modified_without_recursion() {
        // `providers` goes from an object to a string — emit ONE
        // Modified, not six Removed + one Added per field.
        let current = json!({ "providers": { "llm": { "kind": "openai" } } });
        let incoming = json!({ "providers": "disabled" });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].path, "providers");
        assert_eq!(d.changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn changed_sections_are_deduped_and_sorted() {
        let current = json!({
            "tenant": { "slug": "acme", "name": "Acme" },
            "providers": { "llm": { "model": "gpt-4" } }
        });
        let incoming = json!({
            "tenant": { "slug": "acme", "name": "Acme Corp", "email": "a@b.c" },
            "providers": { "llm": { "model": "gpt-4-turbo" } }
        });
        let d = compute_diff("acme".into(), Some(current), incoming);
        assert_eq!(
            d.summary.changed_sections,
            vec!["providers".to_string(), "tenant".to_string()]
        );
    }

    #[test]
    fn diff_is_serde_round_trippable() {
        // The diff is the wire contract for the endpoint + CLI.
        // Any field rename must preserve JSON round-trip or live
        // clients break.
        let d = compute_diff(
            "acme".into(),
            Some(json!({ "tenant": { "slug": "acme" } })),
            json!({ "tenant": { "slug": "acme", "name": "Acme" } }),
        );
        let s = serde_json::to_string(&d).unwrap();
        let back: TenantDiff = serde_json::from_str(&s).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn operation_serializes_lowercase() {
        // Wire contract: `"operation": "update"`, not `"Update"`.
        let d = compute_diff(
            "acme".into(),
            Some(json!({})),
            json!({ "tenant": { "slug": "acme" } }),
        );
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["operation"], json!("create"));
    }

    #[test]
    fn top_level_section_handles_root() {
        assert_eq!(top_level_section(""), None);
        assert_eq!(top_level_section("tenant"), Some("tenant"));
        assert_eq!(top_level_section("tenant.name"), Some("tenant"));
        assert_eq!(
            top_level_section("providers.llm.secret_ref"),
            Some("providers")
        );
    }

    #[test]
    fn envelope_keys_are_pinned() {
        // Changing this set quietly would make old clients see
        // phantom diffs on re-apply. Lock the contract.
        assert_eq!(
            ENVELOPE_KEYS,
            &["apiVersion", "kind", "metadata", "notRendered"]
        );
    }
}
