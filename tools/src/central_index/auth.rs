//! Authentication, rate limiting, and webhook signature verification for the
//! Central Index Service.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Validates API keys from the `Authorization: Bearer <token>` header against
/// the `AETERNA_API_KEY` environment variable.
#[derive(Debug, Clone)]
pub struct ApiKeyGuard {
    /// The expected API key value.
    pub api_key: String,
}

impl ApiKeyGuard {
    /// Build a guard from the `AETERNA_API_KEY` environment variable.
    /// Returns `None` if the variable is not set.
    pub fn from_env() -> Option<Self> {
        std::env::var("AETERNA_API_KEY")
            .ok()
            .map(|k| Self { api_key: k })
    }

    /// Constant-time–ish comparison (falls back to basic equality; for true
    /// constant-time use a dedicated crate). Good enough when the key is not
    /// a cryptographic secret with timing side-channels in scope.
    pub fn validate(&self, token: &str) -> bool {
        // Length check first to short-circuit obviously wrong tokens.
        if token.len() != self.api_key.len() {
            return false;
        }
        // Byte-by-byte OR to avoid early-exit optimisation.
        let mut diff: u8 = 0;
        for (a, b) in token.bytes().zip(self.api_key.bytes()) {
            diff |= a ^ b;
        }
        diff == 0
    }

    /// Extract the bearer token from an `Authorization` header value.
    pub fn extract_from_header(auth_header: &str) -> Option<&str> {
        auth_header.strip_prefix("Bearer ")
    }
}

/// Simple sliding-window rate limiter: `max_per_minute` requests per key
/// within a 60-second window.
pub struct RateLimiter {
    state: Arc<parking_lot::Mutex<HashMap<String, (usize, Instant)>>>,
    max_per_minute: usize,
}

impl RateLimiter {
    /// Create a new limiter allowing `max_per_minute` requests per key per
    /// 60-second window.
    pub fn new(max_per_minute: usize) -> Self {
        Self {
            state: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            max_per_minute,
        }
    }

    /// Check whether the request should be allowed. Returns `true` if within
    /// limits, `false` if the caller should be throttled.
    pub fn check_and_increment(&self, key: &str) -> bool {
        let mut map = self.state.lock();
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert_with(|| (0, now));

        // Reset counter when the window has elapsed.
        if now.duration_since(entry.1).as_secs() >= 60 {
            *entry = (0, now);
        }

        if entry.0 >= self.max_per_minute {
            return false;
        }
        entry.0 += 1;
        true
    }
}

/// Verify an HMAC-SHA256 webhook signature of the form `sha256=<hex>`.
///
/// If the `sha2` crate is available (it is), we compute a real HMAC-SHA256 via
/// the standard `H(key ^ opad || H(key ^ ipad || message))` construction.
pub fn verify_webhook_signature(body: &[u8], signature: &str, secret: &str) -> bool {
    let hex_digest = match signature.strip_prefix("sha256=") {
        Some(h) => h,
        None => return false,
    };

    let expected = hmac_sha256(secret.as_bytes(), body);
    let expected_hex = hex_encode(&expected);

    // Constant-time comparison
    if expected_hex.len() != hex_digest.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in expected_hex.bytes().zip(hex_digest.bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// HMAC-SHA256 implemented with `sha2::Sha256`.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;

    // If key is longer than block size, hash it first.
    let key = if key.len() > BLOCK_SIZE {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let h: [u8; 32] = hasher.finalize().into();
        h.to_vec()
    } else {
        key.to_vec()
    };

    // Pad key to block size.
    let mut padded_key = vec![0u8; BLOCK_SIZE];
    padded_key[..key.len()].copy_from_slice(&key);

    // Inner and outer pads.
    let mut ipad = vec![0x36u8; BLOCK_SIZE];
    let mut opad = vec![0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= padded_key[i];
        opad[i] ^= padded_key[i];
    }

    // Inner hash.
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(message);
    let inner_hash: [u8; 32] = inner.finalize().into();

    // Outer hash.
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_guard_validates_correct_key() {
        let guard = ApiKeyGuard {
            api_key: "secret-key-123".into(),
        };
        assert!(guard.validate("secret-key-123"));
    }

    #[test]
    fn api_key_guard_rejects_wrong_key() {
        let guard = ApiKeyGuard {
            api_key: "secret-key-123".into(),
        };
        assert!(!guard.validate("wrong-key"));
        assert!(!guard.validate("secret-key-124"));
        assert!(!guard.validate(""));
    }

    #[test]
    fn extract_bearer_token() {
        assert_eq!(
            ApiKeyGuard::extract_from_header("Bearer my-token"),
            Some("my-token")
        );
        assert_eq!(ApiKeyGuard::extract_from_header("Basic abc123"), None);
        assert_eq!(ApiKeyGuard::extract_from_header(""), None);
    }

    #[test]
    fn rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(3);
        assert!(limiter.check_and_increment("k1"));
        assert!(limiter.check_and_increment("k1"));
        assert!(limiter.check_and_increment("k1"));
        // 4th should be denied
        assert!(!limiter.check_and_increment("k1"));
        // Different key is independent
        assert!(limiter.check_and_increment("k2"));
    }

    #[test]
    fn webhook_signature_valid() {
        let body = b"hello world";
        let secret = "my-secret";
        let expected = hmac_sha256(secret.as_bytes(), body);
        let sig = format!("sha256={}", hex_encode(&expected));
        assert!(verify_webhook_signature(body, &sig, secret));
    }

    #[test]
    fn webhook_signature_invalid() {
        assert!(!verify_webhook_signature(b"body", "sha256=0000", "secret"));
        assert!(!verify_webhook_signature(b"body", "bad-format", "secret"));
    }

    #[test]
    fn hex_encode_works() {
        assert_eq!(hex_encode(&[0x00, 0xff, 0x0a]), "00ff0a");
    }
}
