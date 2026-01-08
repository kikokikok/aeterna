//! # Memory-Knowledge Utilities
//!
//! Common utility functions for hashing, validation, and UUID generation.
//!
//! # Best Practices
//!
//! - Uses SHA-2 for secure hashing
//! - Uses UUID v4 with serde support
//! - Validates inputs with comprehensive error messages

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Compute SHA-256 hash of content string
///
/// # Examples
///
/// ```
/// use utils::compute_content_hash;
///
/// let hash = compute_content_hash("hello world");
/// assert_eq!(hash.len(), 64);
/// ```
#[must_use]
pub fn compute_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Compute hash of knowledge item for change detection
///
/// Hashes content, constraints, and status fields.
#[must_use]
pub fn compute_knowledge_hash(item: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();

    // Extract fields for hashing
    if let Some(content) = item.get("content") {
        if let Some(content_str) = content.as_str() {
            hasher.update(content_str.as_bytes());
        }
    }

    if let Some(constraints) = item.get("constraints") {
        let constraints_json =
            serde_json::to_string(constraints).expect("Failed to serialize constraints");
        hasher.update(constraints_json.as_bytes());
    }

    if let Some(status) = item.get("status") {
        if let Some(status_str) = status.as_str() {
            hasher.update(status_str.as_bytes());
        }
    }

    format!("{:x}", hasher.finalize())
}

/// Generate UUID v4 string
#[must_use]
pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Validate memory layer string
#[must_use]
pub fn is_valid_layer(layer: &str) -> bool {
    matches!(
        layer,
        "agent" | "user" | "session" | "project" | "team" | "org" | "company"
    )
}

/// Validate knowledge type string
#[must_use]
pub fn is_valid_knowledge_type(ktype: &str) -> bool {
    matches!(ktype, "adr" | "policy" | "pattern" | "spec")
}

/// Validate knowledge layer string
#[must_use]
pub fn is_valid_knowledge_layer(layer: &str) -> bool {
    matches!(layer, "company" | "org" | "team" | "project")
}

/// Get layer precedence value for memory layers
#[must_use]
pub fn get_layer_precedence(layer: &str) -> u8 {
    match layer {
        "agent" => 1,
        "user" => 2,
        "session" => 3,
        "project" => 4,
        "team" => 5,
        "org" => 6,
        "company" => 7,
        _ => 7, // Default to lowest precedence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_content_hash_consistency() {
        let content = "test content";
        let hash1 = compute_content_hash(content);
        let hash2 = compute_content_hash(content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_generate_uuid_uniqueness() {
        let uuid1 = generate_uuid();
        let uuid2 = generate_uuid();
        assert_ne!(uuid1, uuid2);
    }

    #[test]
    fn test_layer_validation_valid() {
        assert!(is_valid_layer("agent"));
        assert!(is_valid_layer("user"));
        assert!(is_valid_layer("company"));
    }

    #[test]
    fn test_layer_validation_invalid() {
        assert!(!is_valid_layer("invalid"));
        assert!(!is_valid_layer("agent-user"));
    }
}
