//! Canonical JSON serialization + SHA-256 fingerprint of a [`TenantManifest`].
//!
//! This module exists to give `provision_tenant` an idempotent short-circuit:
//! if the caller submits a manifest whose canonical hash equals the row's
//! `last_applied_manifest_hash`, we can skip the entire apply pipeline and
//! return `{status: "unchanged"}` in O(1).
//!
//! # Canonicalization rules
//!
//! 1. **Sorted object keys.** `serde_json::Value::Object` uses `BTreeMap`
//!    under the `preserve_order` feature flag. We re-serialize through a
//!    recursive pass that emits keys in lexicographic order, guaranteeing
//!    a single byte sequence independent of input order.
//! 2. **Inline plaintext stripped.** Every `secrets[].secretValue` and every
//!    inline `config.fields[*].secretValue` is replaced with the literal
//!    string `"<stripped>"` before hashing. Plaintext must never influence
//!    the hash — otherwise rotating a secret value with no other change
//!    would force a full re-apply AND would leak the secret into the hash's
//!    input material (defence in depth).
//! 3. **No whitespace.** Compact JSON, `serde_json::to_vec(..)`.
//! 4. **UTF-8.** `serde_json` already enforces this.
//!
//! # Output format
//!
//! `"sha256:" + lowercase hex`. 64 hex chars + 7-char prefix = 71 chars total.
//! Prefix makes the algorithm explicit in DB rows and logs, and lets us
//! migrate to a different digest in the future by namespace without
//! ambiguity.
//!
//! # Stability
//!
//! The hash is part of the wire contract between `provision_tenant` calls on
//! different pods. Changing canonicalization rules is a breaking change and
//! MUST be accompanied by a bump of a `HASH_VERSION` constant or, more
//! likely, a new prefix (`sha256v2:...`). Today there is only one version.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Prefix identifying the canonicalization rule set and digest algorithm.
/// Part of the wire contract — see module docs.
pub const HASH_PREFIX: &str = "sha256:";

/// Compute the canonical SHA-256 fingerprint of an already-deserialized
/// [`serde_json::Value`] representation of a tenant manifest.
///
/// Callers that have a typed `TenantManifest` in hand should go through
/// [`hash_manifest_value`] by first `serde_json::to_value(&manifest)`. We do
/// not offer a typed-input convenience function in this module to keep the
/// dependency graph light: this module is dialect-only.
///
/// # Errors
///
/// Returns an error only if the resulting JSON cannot be serialized, which
/// for `Value` input is essentially impossible (no custom Serialize
/// implementations, no `Box<dyn Error>` weirdness). The `Result` is kept for
/// forward compatibility in case we later canonicalize over typed input.
pub fn hash_manifest_value(raw: &Value) -> Result<String, serde_json::Error> {
    let stripped = strip_plaintext(raw.clone());
    let canonical_bytes = canonical_bytes(&stripped)?;
    let mut hasher = Sha256::new();
    hasher.update(&canonical_bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(HASH_PREFIX.len() + 64);
    out.push_str(HASH_PREFIX);
    for byte in digest {
        use std::fmt::Write as _;
        // we control the formatter, write! to String is infallible
        let _ = write!(out, "{byte:02x}");
    }
    Ok(out)
}

/// Recursively replace every `secretValue` field in `secrets` entries and
/// `config.fields` entries with the sentinel string `<stripped>`.
///
/// We strip by **key name**, not by path, because the same key appears in
/// two places and we want both covered without special-casing.
fn strip_plaintext(mut v: Value) -> Value {
    strip_plaintext_in_place(&mut v);
    v
}

fn strip_plaintext_in_place(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for (k, child) in map.iter_mut() {
                if k == "secretValue" || k == "secret_value" {
                    *child = Value::String("<stripped>".to_string());
                } else {
                    strip_plaintext_in_place(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                strip_plaintext_in_place(item);
            }
        }
        _ => {}
    }
}

/// Emit canonical JSON bytes: compact, lexicographic object-key order,
/// recursively.
///
/// We do this by walking the `Value` tree and rebuilding each `Object` as a
/// `Map` whose keys are inserted in sorted order. `serde_json::Map` preserves
/// insertion order when the `preserve_order` feature is on (it is), so
/// controlling insertion order controls output order.
fn canonical_bytes(v: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let canonical = canonicalize(v);
    serde_json::to_vec(&canonical)
}

fn canonicalize(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let mut out = Map::with_capacity(keys.len());
            for k in keys {
                out.insert(k.clone(), canonicalize(&m[k]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hash_has_prefix_and_correct_length() {
        let h = hash_manifest_value(&json!({"apiVersion": "aeterna.io/v1"})).unwrap();
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
        assert!(h["sha256:".len()..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_is_stable_across_key_order() {
        let a = json!({"a": 1, "b": {"x": 1, "y": 2}, "c": [3, 2, 1]});
        let b = json!({"c": [3, 2, 1], "b": {"y": 2, "x": 1}, "a": 1});
        assert_eq!(
            hash_manifest_value(&a).unwrap(),
            hash_manifest_value(&b).unwrap(),
            "key order must not affect the hash"
        );
    }

    #[test]
    fn hash_preserves_array_order() {
        // arrays are ORDERED collections — [1,2,3] and [3,2,1] are different
        // manifests (e.g. role-assignment order matters for audit)
        let a = json!({"x": [1, 2, 3]});
        let b = json!({"x": [3, 2, 1]});
        assert_ne!(
            hash_manifest_value(&a).unwrap(),
            hash_manifest_value(&b).unwrap()
        );
    }

    #[test]
    fn hash_strips_inline_plaintext_secret_values() {
        // Rotating the plaintext of an inline secret MUST NOT change the hash.
        // If it did, a rotation-only manifest would look like a full re-apply
        // to provision_tenant, and the secret material would leak into the
        // hash's input.
        let base = json!({
            "secrets": [
                {"logicalName": "repo.token", "ownership": "Tenant", "secretValue": "aaa"}
            ]
        });
        let rotated = json!({
            "secrets": [
                {"logicalName": "repo.token", "ownership": "Tenant", "secretValue": "bbb"}
            ]
        });
        assert_eq!(
            hash_manifest_value(&base).unwrap(),
            hash_manifest_value(&rotated).unwrap(),
            "inline secret_value rotation must not change the hash"
        );
    }

    #[test]
    fn hash_strips_both_casings_of_secret_value_key() {
        // Stripping is by KEY NAME, and we cover both `secretValue` (the
        // wire shape) and `secret_value` (which serde accepts on input).
        // The hash still differs between camelCase and snake_case submissions
        // because the key names themselves are part of the canonical bytes
        // — that is expected and correct: two different JSON documents
        // should produce two different hashes. What we test here is that
        // rotating the plaintext on EITHER key name produces a stable
        // hash within that key's casing.
        let camel_a = json!({"secrets": [{"secretValue": "aaa"}]});
        let camel_b = json!({"secrets": [{"secretValue": "bbb"}]});
        assert_eq!(
            hash_manifest_value(&camel_a).unwrap(),
            hash_manifest_value(&camel_b).unwrap(),
            "camelCase secretValue rotation must not change the hash"
        );

        let snake_a = json!({"secrets": [{"secret_value": "aaa"}]});
        let snake_b = json!({"secrets": [{"secret_value": "bbb"}]});
        assert_eq!(
            hash_manifest_value(&snake_a).unwrap(),
            hash_manifest_value(&snake_b).unwrap(),
            "snake_case secret_value rotation must not change the hash"
        );

        // Sanity: the two casings are different wire shapes and thus
        // different hashes. Callers should normalize to camelCase before
        // submit; we do not silently treat them as identical.
        assert_ne!(
            hash_manifest_value(&camel_a).unwrap(),
            hash_manifest_value(&snake_a).unwrap(),
        );
    }

    #[test]
    fn hash_changes_when_non_secret_content_changes() {
        let v1 = json!({"tenant": {"slug": "a", "name": "A"}});
        let v2 = json!({"tenant": {"slug": "a", "name": "B"}});
        assert_ne!(
            hash_manifest_value(&v1).unwrap(),
            hash_manifest_value(&v2).unwrap()
        );
    }

    #[test]
    fn hash_changes_when_generation_bumps() {
        // generation is NOT stripped — bumping the revision counter is a
        // legitimate manifest change and must produce a new hash, otherwise
        // provision_tenant would short-circuit a version bump.
        let v1 = json!({"metadata": {"generation": 1}, "tenant": {"slug": "a", "name": "A"}});
        let v2 = json!({"metadata": {"generation": 2}, "tenant": {"slug": "a", "name": "A"}});
        assert_ne!(
            hash_manifest_value(&v1).unwrap(),
            hash_manifest_value(&v2).unwrap()
        );
    }

    #[test]
    fn hash_is_deterministic_byte_for_byte() {
        let v = json!({
            "apiVersion": "aeterna.io/v1",
            "kind": "TenantManifest",
            "metadata": {"generation": 1, "labels": {"b": "2", "a": "1"}},
            "tenant": {"slug": "x", "name": "X"}
        });
        let h1 = hash_manifest_value(&v).unwrap();
        let h2 = hash_manifest_value(&v).unwrap();
        assert_eq!(h1, h2);
        // Also: hashing twice in a row must not mutate the input.
        // (strip_plaintext takes ownership of a clone, so this is already
        // structurally guaranteed; this assertion documents the contract.)
        assert_eq!(v["tenant"]["slug"], "x");
    }

    #[test]
    fn hash_treats_nested_empty_and_missing_as_different() {
        // {} and absent are distinguishable on the wire and must be
        // distinguishable in the hash, otherwise "clear the labels" and
        // "omit labels" become indistinguishable operations.
        let absent = json!({"tenant": {"slug": "x", "name": "X"}});
        let empty = json!({"tenant": {"slug": "x", "name": "X"}, "metadata": {}});
        assert_ne!(
            hash_manifest_value(&absent).unwrap(),
            hash_manifest_value(&empty).unwrap()
        );
    }

    #[test]
    fn hash_known_vector_locks_the_algorithm() {
        // Golden hash. If this breaks, it means canonicalization rules
        // changed — which is a wire-contract break. Bump HASH_PREFIX
        // (introduce sha256v2:) instead of silently changing this value.
        let v = json!({
            "apiVersion": "aeterna.io/v1",
            "kind": "TenantManifest",
            "tenant": {"slug": "acme", "name": "Acme"}
        });
        let h = hash_manifest_value(&v).unwrap();
        // Recomputed from the canonical bytes of the above object:
        //   {"apiVersion":"aeterna.io/v1","kind":"TenantManifest","tenant":{"name":"Acme","slug":"acme"}}
        assert_eq!(
            h, "sha256:c262fb0d40d2fafbef7d4f4bf277b74760ca87ead31efcbf1549650551c5a483",
            "if this assertion fails, read the test body before changing the expected value"
        );
    }
}
