//! Secret handling primitives: [`SecretBytes`] and [`SecretReference`].
//!
//! These types are the foundation of the unified secret-management model
//! designed in `openspec/changes/harden-tenant-provisioning/design.md`.
//!
//! # Design notes
//!
//! - [`SecretBytes`] wraps a byte buffer whose contents are zeroized on drop.
//!   Its `Debug` and `Display` implementations always print `<redacted>` so
//!   secret material cannot leak through `tracing`, `format!`, or JSON
//!   serialization.
//! - [`SecretReference`] is a tagged enum so future backends (external secret
//!   managers, etc.) can be added as additive variants without breaking the
//!   serialized representation of existing data.
//! - In B1 only the [`SecretReference::Postgres`] variant exists. It points
//!   at a row in the `tenant_secrets` table whose ciphertext is envelope
//!   encrypted with a KMS-wrapped data encryption key.

use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::{PartialSchema, ToSchema};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// An opaque container for in-memory secret material.
///
/// The inner buffer is zeroized when the value is dropped. The `Debug`,
/// `Display`, and `serde::Serialize` implementations deliberately emit
/// `<redacted>` so the bytes cannot accidentally be written to logs, error
/// messages, or API responses.
///
/// # Example
///
/// ```
/// use mk_core::SecretBytes;
/// let s = SecretBytes::from(b"hunter2".to_vec());
/// assert_eq!(format!("{s:?}"), "SecretBytes(<redacted>)");
/// assert_eq!(format!("{s}"), "<redacted>");
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    /// Construct a new `SecretBytes` from a byte vector.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Construct from a UTF-8 string. The input string is consumed and the
    /// resulting secret is zeroized on drop; the caller is responsible for
    /// ensuring the source string has no lingering copies.
    #[must_use]
    pub fn from_string(value: String) -> Self {
        Self(value.into_bytes())
    }

    /// Borrow the secret material as a byte slice. Callers must not log,
    /// copy, or persist the returned slice beyond its immediate use.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Consume `self` and return the owned byte buffer. Prefer [`Self::expose`]
    /// in almost all cases; this exists for interop with APIs that require
    /// ownership (e.g. AES-GCM's `encrypt` signature).
    #[must_use]
    pub fn into_bytes(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }

    /// Length of the underlying byte buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the underlying buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for SecretBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<String> for SecretBytes {
    fn from(value: String) -> Self {
        Self::from_string(value)
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretBytes(<redacted>)")
    }
}

impl fmt::Display for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl Serialize for SecretBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("<redacted>")
    }
}

/// Deserialize a plaintext string from the input into a `SecretBytes`.
///
/// This is the deliberate shape for **API input boundaries** (tenant secret
/// writes in the admin API): the client sends the plaintext secret as a JSON
/// string and the server wraps it into a [`SecretBytes`] immediately, so it
/// zeroizes on drop and never leaks via `Debug`/`Display`/`Serialize`.
///
/// **Do not** use `Deserialize` to round-trip a previously serialized
/// `SecretBytes`: `serialize` writes `"<redacted>"` on purpose; the only
/// correct round-trip path for stored secrets is through [`SecretReference`]
/// + a [`storage::secret_backend::SecretBackend`].
impl<'de> Deserialize<'de> for SecretBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SecretBytes::from(s.into_bytes()))
    }
}

impl PartialEq for SecretBytes {
    /// Constant-time equality on the underlying bytes.
    ///
    /// Uses a simple constant-time comparison to avoid timing attacks when
    /// comparing two secret values (e.g. in tests or during rotation
    /// verification).
    fn eq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        let mut diff: u8 = 0;
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            diff |= a ^ b;
        }
        diff == 0
    }
}

impl Eq for SecretBytes {}

// OpenAPI + JSON Schema: expose `SecretBytes` as a plain string on the wire.
// This is accurate for both read and write shapes: inbound requests carry
// plaintext strings, outbound responses redact to the literal `"<redacted>"`.
impl PartialSchema for SecretBytes {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        String::schema()
    }
}

impl ToSchema for SecretBytes {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SecretBytes")
    }
}

// `Uuid` does not implement `schemars::JsonSchema` in this workspace's
// schemars feature set. We describe `SecretReference` manually as an
// externally-tagged object: `{ "kind": "postgres", "secretId": "<uuid>" }`.
impl schemars::JsonSchema for SecretReference {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SecretReference")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["kind", "secretId"],
            "properties": {
                "kind": { "type": "string", "enum": ["postgres"] },
                "secretId": { "type": "string", "format": "uuid" }
            }
        })
    }
}

impl schemars::JsonSchema for SecretBytes {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SecretBytes")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

/// A reference to secret material.
///
/// Covers every way a tenant manifest can describe "here is where this
/// secret lives": inline plaintext on the wire, encrypted-at-rest in our
/// own database, or an external reference that a backend resolves at
/// runtime (env var, file on disk, Kubernetes `Secret`, HashiCorp Vault).
///
/// Serialized as a `#[serde(tag = "kind")]` tagged union, so the wire
/// shape is self-describing and future variants add non-colliding shapes.
///
/// # Equality & hashing
///
/// `PartialEq`/`Eq` derive by-variant; notably, two `Inline` values with
/// different plaintext are not equal. That is correct for config diffing
/// (a secret rotation is a real change) but means you must not use
/// `SecretReference` as a map key in places where the comparison would
/// cross a hash boundary (we have no such usage today).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SecretReference {
    /// **Wire-only**: caller supplies plaintext directly in the manifest.
    ///
    /// The server accepts `Inline` on **input** (the CLI and REST callers
    /// need a way to express "please store this new secret"), stores the
    /// plaintext via [`crate::traits::SecretBackend::put`], and replaces
    /// the `Inline` reference with [`SecretReference::Postgres`] before
    /// the owning [`crate::types::TenantSecretReference`] is persisted.
    ///
    /// A rendered manifest (see `manifest_render`) **never** contains
    /// `Inline` — the server has no way to recover plaintext from
    /// encrypted storage. If you see `Inline` on the way out of a
    /// rendered manifest, something is wrong.
    ///
    /// `plaintext` is [`SecretBytes`], which serializes as `"<redacted>"`.
    /// That is a safety default: accidentally reserializing an `Inline`
    /// reference cannot leak the plaintext. Use
    /// [`Self::expose_inline_plaintext`] when you intentionally need the
    /// bytes on the storage path.
    Inline { plaintext: SecretBytes },

    /// Encrypted blob stored in the `tenant_secrets` Postgres table. The
    /// row holds a KMS-wrapped DEK and an AES-256-GCM ciphertext. This is
    /// the only variant produced by the default storage backend.
    Postgres {
        /// Primary key of the `tenant_secrets` row.
        secret_id: Uuid,
    },

    /// Environment variable on the server process, resolved at read time.
    ///
    /// Suited to platform-wide secrets injected via a container runtime
    /// (`DATABASE_URL`, `SLACK_BOT_TOKEN`, etc.). The variable name is
    /// **not** confidential; the value never touches our database.
    Env {
        /// Env-var name (e.g. `"DATABASE_PASSWORD"`). Case-sensitive on
        /// POSIX; validated non-empty and no embedded `=`/null bytes.
        var: String,
    },

    /// File on disk readable by the server process, resolved at read time.
    ///
    /// Intended for Kubernetes / Docker secret volume mounts where the
    /// platform injects the secret as a file.
    File {
        /// Absolute path. Validated non-empty and absolute at apply time.
        path: String,
    },

    /// Reference to a Kubernetes `Secret` resource, resolved via the
    /// cluster API at read time.
    K8s {
        /// `metadata.name` of the `Secret`.
        name: String,
        /// Key within the Secret's `data` map.
        key: String,
        /// `metadata.namespace`. `None` = the server process's own
        /// namespace (derived from the pod's downward API at runtime).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        namespace: Option<String>,
    },

    /// Reference to a HashiCorp Vault KV-v2 secret.
    Vault {
        /// Mount point of the KV-v2 engine (e.g. `"secret"`).
        mount: String,
        /// Path under the mount (e.g. `"tenants/acme/db"`).
        path: String,
        /// Field name within the secret document.
        field: String,
    },
}

impl SecretReference {
    /// Short, log-safe description of the reference (no secret material).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            SecretReference::Inline { .. } => "inline",
            SecretReference::Postgres { .. } => "postgres",
            SecretReference::Env { .. } => "env",
            SecretReference::File { .. } => "file",
            SecretReference::K8s { .. } => "k8s",
            SecretReference::Vault { .. } => "vault",
        }
    }

    /// True when this reference carries secret material **in the value
    /// itself** (only `Inline`). Callers use this to decide whether they
    /// must route through [`crate::traits::SecretBackend::put`] before
    /// persisting the reference.
    #[must_use]
    pub fn carries_plaintext(&self) -> bool {
        matches!(self, SecretReference::Inline { .. })
    }

    /// Extract the plaintext from an `Inline` variant. Returns `None` for
    /// every other variant. Callers must not log the result.
    #[must_use]
    pub fn expose_inline_plaintext(&self) -> Option<&[u8]> {
        match self {
            SecretReference::Inline { plaintext } => Some(plaintext.expose()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_leaks_bytes() {
        let s = SecretBytes::from(b"hunter2".to_vec());
        let debug = format!("{s:?}");
        assert_eq!(debug, "SecretBytes(<redacted>)");
        assert!(!debug.contains("hunter2"));
    }

    #[test]
    fn display_never_leaks_bytes() {
        let s = SecretBytes::from_string("hunter2".to_string());
        assert_eq!(format!("{s}"), "<redacted>");
    }

    #[test]
    fn serialize_redacts() {
        let s = SecretBytes::from(b"hunter2".to_vec());
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"<redacted>\"");
    }

    #[test]
    fn constant_time_eq_matches_lengths() {
        let a = SecretBytes::from(b"abc".to_vec());
        let b = SecretBytes::from(b"abc".to_vec());
        let c = SecretBytes::from(b"abcd".to_vec());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn expose_returns_slice() {
        let s = SecretBytes::from(b"plaintext".to_vec());
        assert_eq!(s.expose(), b"plaintext");
    }

    #[test]
    fn reference_kind_for_every_variant() {
        use std::collections::HashSet;
        let all = [
            SecretReference::Inline {
                plaintext: SecretBytes::from(b"x".to_vec()),
            },
            SecretReference::Postgres {
                secret_id: Uuid::nil(),
            },
            SecretReference::Env { var: "X".into() },
            SecretReference::File { path: "/x".into() },
            SecretReference::K8s {
                name: "n".into(),
                key: "k".into(),
                namespace: None,
            },
            SecretReference::Vault {
                mount: "m".into(),
                path: "p".into(),
                field: "f".into(),
            },
        ];
        let kinds: HashSet<&str> = all.iter().map(SecretReference::kind).collect();
        // Every variant has a distinct non-empty kind discriminator.
        assert_eq!(kinds.len(), all.len(), "kind() must be unique per variant");
        assert!(!kinds.iter().any(|k| k.is_empty()));
    }

    #[test]
    fn carries_plaintext_is_only_inline() {
        let cases = [
            (
                SecretReference::Inline {
                    plaintext: SecretBytes::from(b"x".to_vec()),
                },
                true,
            ),
            (
                SecretReference::Postgres {
                    secret_id: Uuid::nil(),
                },
                false,
            ),
            (SecretReference::Env { var: "X".into() }, false),
            (SecretReference::File { path: "/x".into() }, false),
        ];
        for (r, expected) in cases {
            assert_eq!(r.carries_plaintext(), expected, "{:?}", r);
        }
    }

    #[test]
    fn expose_inline_plaintext_only_on_inline() {
        let inline = SecretReference::Inline {
            plaintext: SecretBytes::from(b"hunter2".to_vec()),
        };
        assert_eq!(inline.expose_inline_plaintext(), Some(&b"hunter2"[..]));
        let pg = SecretReference::Postgres {
            secret_id: Uuid::nil(),
        };
        assert_eq!(pg.expose_inline_plaintext(), None);
    }

    #[test]
    fn roundtrip_postgres_wire_shape() {
        let r = SecretReference::Postgres {
            secret_id: Uuid::nil(),
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["kind"], "postgres");
        assert!(j["secretId"].is_string());
        let back: SecretReference = serde_json::from_value(j).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn roundtrip_inline_redacts_on_serialize_preserves_on_deserialize() {
        // Deserialize accepts plaintext …
        let j = serde_json::json!({ "kind": "inline", "plaintext": "hunter2" });
        let r: SecretReference = serde_json::from_value(j).unwrap();
        assert_eq!(r.expose_inline_plaintext(), Some(&b"hunter2"[..]));

        // … but serialize always emits <redacted> (safety default).
        let j2 = serde_json::to_value(&r).unwrap();
        assert_eq!(j2["kind"], "inline");
        assert_eq!(j2["plaintext"], "<redacted>");
    }

    #[test]
    fn roundtrip_env_wire_shape() {
        let j = serde_json::json!({ "kind": "env", "var": "DATABASE_URL" });
        let r: SecretReference = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(r.kind(), "env");
        let back = serde_json::to_value(&r).unwrap();
        assert_eq!(back, j);
    }

    #[test]
    fn roundtrip_file_wire_shape() {
        let j = serde_json::json!({ "kind": "file", "path": "/run/secrets/db" });
        let r: SecretReference = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(r.kind(), "file");
        assert_eq!(serde_json::to_value(&r).unwrap(), j);
    }

    #[test]
    fn roundtrip_k8s_namespace_omitted_when_none() {
        // namespace absent on wire when None (serde skip_serializing_if).
        let r = SecretReference::K8s {
            name: "db".into(),
            key: "password".into(),
            namespace: None,
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["kind"], "k8s");
        assert_eq!(j["name"], "db");
        assert_eq!(j["key"], "password");
        assert!(
            j.get("namespace").is_none(),
            "namespace=None must omit the field, got {j}"
        );

        // namespace present when Some.
        let r2 = SecretReference::K8s {
            name: "db".into(),
            key: "password".into(),
            namespace: Some("aeterna".into()),
        };
        let j2 = serde_json::to_value(&r2).unwrap();
        assert_eq!(j2["namespace"], "aeterna");
        let back: SecretReference = serde_json::from_value(j2).unwrap();
        assert_eq!(back, r2);
    }

    #[test]
    fn roundtrip_vault_wire_shape() {
        let j = serde_json::json!({
            "kind": "vault",
            "mount": "secret",
            "path": "tenants/acme/db",
            "field": "password"
        });
        let r: SecretReference = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(r.kind(), "vault");
        assert_eq!(serde_json::to_value(&r).unwrap(), j);
    }

    #[test]
    fn deserialize_rejects_unknown_kind() {
        // #[serde(tag = "kind")] rejects tags it does not know about at
        // deserialize time; validate_manifest never sees a reference it
        // cannot classify. This test locks that contract so a future
        // change to the enum's serde attributes (e.g. adding
        // #[serde(other)]) is a deliberate decision and breaks this test.
        let j = serde_json::json!({ "kind": "mysterybox", "magic": 42 });
        let r: Result<SecretReference, _> = serde_json::from_value(j);
        assert!(r.is_err(), "unknown kind must not deserialize");
    }

    // Legacy single-variant test, kept to lock the original wire shape.
    #[test]
    fn reference_kind() {
        let r = SecretReference::Postgres {
            secret_id: Uuid::nil(),
        };
        assert_eq!(r.kind(), "postgres");
    }

    #[test]
    fn reference_roundtrip_json() {
        let r = SecretReference::Postgres {
            secret_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"kind\":\"postgres\""));
        let parsed: SecretReference = serde_json::from_str(&j).unwrap();
        assert_eq!(r, parsed);
    }

    /// Regression: the wire shape of every `SecretReference` variant field
    /// must be camelCase, matching the outer `kind` tag and the flattening
    /// context (`TenantSecretReference`). Without `rename_all_fields =
    /// "camelCase"` on the enum, variant fields leak as snake_case, which
    /// broke [`cli::server::tenant_api`] deserialization of
    /// `UpsertTenantConfigRequest.secretReferences`.
    #[test]
    fn reference_variant_fields_are_camel_case_on_wire() {
        let r = SecretReference::Postgres {
            secret_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(
            j.contains("\"secretId\""),
            "expected camelCase secretId on wire, got: {j}"
        );
        assert!(
            !j.contains("\"secret_id\""),
            "snake_case secret_id leaked on wire: {j}"
        );

        // Deserialize MUST accept camelCase (the canonical shape) and
        // MUST reject snake_case so we do not have two valid wire shapes.
        let good = serde_json::json!({
            "kind": "postgres",
            "secretId": "22222222-2222-2222-2222-222222222222"
        });
        assert!(serde_json::from_value::<SecretReference>(good).is_ok());

        let bad = serde_json::json!({
            "kind": "postgres",
            "secret_id": "22222222-2222-2222-2222-222222222222"
        });
        assert!(
            serde_json::from_value::<SecretReference>(bad).is_err(),
            "snake_case secret_id must not be accepted (we ship only one wire shape)"
        );
    }
}
