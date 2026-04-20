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
use utoipa::ToSchema;
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

/// A reference to secret material stored by a [`SecretBackend`].
///
/// In B1 only the `Postgres` variant exists. The enum shape is intentional
/// so future backends (external secret managers, cloud KV stores, etc.)
/// can be added as additive variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SecretReference {
    /// Encrypted blob stored in the `tenant_secrets` Postgres table. The row
    /// holds a KMS-wrapped DEK and an AES-256-GCM ciphertext.
    Postgres {
        /// Primary key of the `tenant_secrets` row.
        secret_id: Uuid,
    },
}

impl SecretReference {
    /// Short, log-safe description of the reference (no secret material).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            SecretReference::Postgres { .. } => "postgres",
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
    fn reference_kind() {
        let r = SecretReference::Postgres { secret_id: Uuid::nil() };
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
}
