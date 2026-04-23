//! Concrete [`SecretRefResolver`] implementations per B4 §3.2–3.4.
//!
//! Each backend lives in its own submodule so feature gating and
//! per-backend tests stay local. The [`crate::secret_resolver`] module
//! defines the trait + registry + legacy closure adapter; this module
//! only provides impls.
//!
//! Construction is cheap and lazy — resolvers don't open connections
//! at construction time. They do at first resolve (or per-call,
//! depending on backend semantics).

pub mod env;
pub mod file;
pub mod inline;
pub mod k8s;
pub mod postgres;

#[cfg(feature = "vault")]
pub mod vault;

#[cfg(not(feature = "vault"))]
pub mod vault_stub;
#[cfg(not(feature = "vault"))]
pub use vault_stub as vault;

pub use env::EnvRefResolver;
pub use file::FileRefResolver;
pub use inline::InlineRefResolver;
pub use k8s::{K8sRefResolver, K8sSecretFetcher, PodDownwardApiFetcher};
pub use postgres::PostgresRefResolver;
pub use vault::VaultRefResolver;
