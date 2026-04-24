//! HashiCorp Vault backend — compiled only under `feature = "vault"`.
//!
//! `mod.rs` declares this module behind `#[cfg(feature = "vault")]`;
//! without the feature, `vault_stub` is re-exported as `vault`
//! instead. This file exists so `rustfmt`'s module traversal
//! (which, unlike `cargo`, does not honour cfg gates) can succeed
//! even when the feature is off.
//!
//! The real Vault implementation is tracked under B4 §3.4.
