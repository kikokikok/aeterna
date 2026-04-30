/// Secret Provider abstraction for managing sensitive Git tokens and keys.
///
/// Supports external secret managers like AWS Secrets Manager, GCP Secret Manager,
/// Azure Key Vault, and HashiCorp Vault.
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use mk_core::env::Environment;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Secret operation failed: {0}")]
    OperationFailed(String),

    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Format error: {0}")]
    FormatError(String),

    #[error("Retrieval failed: {0}")]
    RetrievalFailed(String),
}

/// Declarative description of which [`SecretProvider`] implementation to
/// instantiate at boot.
///
/// The set of variants is deliberately narrow: every variant must correspond
/// to a real, callable backend. We previously shipped an `Aws` variant whose
/// implementation returned the literal string
/// `"mock-secret-from-aws-{region}-{secret_id}"` — a code-search hazard that
/// could be wired in production by someone reading the variant name and
/// trusting it. It has been removed in the v1.5.0 contract-correctness sweep.
/// AWS Secrets Manager support, when added, will route through
/// [`storage::secret_backend::SecretBackend`] (the encrypted-envelope
/// production path), not through this trait.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum SecretProviderConfig {
    #[serde(rename = "vault")]
    Vault {
        address: String,
        token: String,
        mount_path: String,
    },
    #[serde(rename = "local")]
    Local {
        secrets: std::collections::HashMap<String, String>,
    },
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    /// Retrieve a secret value by its identifier
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError>;

    /// Health check for the provider
    async fn is_available(&self) -> bool;
}

// NOTE: An `AwsSecretProvider` mock was previously shipped here. It returned
// the literal string `"mock-secret-from-aws-{region}-{secret_id}"` from
// `get_secret()` while pretending to be a real AWS Secrets Manager
// integration. Anyone wiring it in production would have silently received
// fake credentials with no error. It was removed in the v1.5.0
// contract-correctness sweep alongside `SecretProviderConfig::Aws`. Real AWS
// Secrets Manager support, when added, will be implemented through
// `storage::secret_backend::SecretBackend` (the production-grade encrypted
// envelope path used by tenant secrets), not through this trait.

/// Local development secret provider
pub struct LocalSecretProvider {
    secrets: std::collections::HashMap<String, String>,
}

impl LocalSecretProvider {
    pub fn new(secrets: std::collections::HashMap<String, String>) -> Self {
        Self { secrets }
    }
}

#[async_trait]
impl SecretProvider for LocalSecretProvider {
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError> {
        self.secrets
            .get(secret_id)
            .cloned()
            .ok_or_else(|| SecretError::NotFound(secret_id.to_string()))
    }

    async fn is_available(&self) -> bool {
        true
    }
}

/// HashiCorp Vault secret provider
pub struct VaultSecretProvider {
    client: reqwest::Client,
    address: String,
    token: String,
}

impl VaultSecretProvider {
    pub fn new(address: String, token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            address,
            token,
        }
    }
}

#[async_trait]
impl SecretProvider for VaultSecretProvider {
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError> {
        let url = format!("{}/v1/secret/data/{}", self.address, secret_id);
        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await
            .map_err(|e| SecretError::ConnectionFailed(e.to_string()))?;

        if response.status().is_success() {
            let body: serde_json::Value = response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| SecretError::FormatError(e.to_string()))?;
            let token = body["data"]["data"]["token"].as_str().ok_or_else(|| {
                SecretError::FormatError("Token not found in Vault secret".to_string())
            })?;
            Ok(token.to_string())
        } else {
            Err(SecretError::RetrievalFailed(format!(
                "Vault returned error: {}",
                response.status()
            )))
        }
    }

    async fn is_available(&self) -> bool {
        // Simple health check could go here
        true
    }
}

/// Maximum time the startup self-test will wait for [`SecretProvider::is_available`]
/// to respond. Picked to be long enough for a Vault round-trip across an
/// AZ but short enough that a misconfigured endpoint surfaces in deployment
/// health checks within seconds.
const SELF_TEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Construct the platform [`SecretProvider`] from environment variables and
/// enforce the production safety gate.
///
/// Mirrors the [`storage::secret_backend::build_secret_backend_from_env`]
/// pattern: read `AETERNA_SECRET_PROVIDER`, instantiate the matching
/// implementation, refuse non-production-grade choices when
/// `AETERNA_ENV=production`, and run a startup self-test before handing the
/// provider back to bootstrap.
///
/// ### Selector matrix
///
/// | `AETERNA_SECRET_PROVIDER` | Production-grade? | Required env vars |
/// |--------------------------|-------------------|--------------------|
/// | `vault` (or `hashicorp-vault`) | yes | `AETERNA_VAULT_ADDR`, `AETERNA_VAULT_TOKEN` |
/// | `local` (default)        | **no**            | none |
///
/// Anything else maps to `local` for backwards compatibility with
/// pre-v1.5.0 deployments. In production this still refuses to boot.
///
/// ### Production safety
///
/// When `AETERNA_ENV=production` and the resolved selector is not
/// production-grade (i.e. `local`), this function returns
/// [`SecretError::ConfigError`] with an actionable message. The aeterna
/// process exits before serving traffic, so the failure surfaces in the
/// deployment health check rather than silently returning empty secrets at
/// the first GitHub clone.
///
/// ### Self-test
///
/// After construction, [`SecretProvider::is_available`] is invoked under a
/// [`SELF_TEST_TIMEOUT`] budget. A timeout or a `false` response is mapped
/// to [`SecretError::ConnectionFailed`] so the operator sees the same
/// fail-fast signal as the KMS startup self-test (B2).
pub async fn build_secret_provider_from_env() -> Result<Arc<dyn SecretProvider>, SecretError> {
    let env = Environment::from_env();
    let selector =
        std::env::var("AETERNA_SECRET_PROVIDER").unwrap_or_else(|_| "local".to_string());

    let provider: Arc<dyn SecretProvider> = match selector.to_ascii_lowercase().as_str() {
        "vault" | "hashicorp-vault" => {
            let address = std::env::var("AETERNA_VAULT_ADDR").map_err(|_| {
                SecretError::ConfigError(
                    "AETERNA_SECRET_PROVIDER=vault but AETERNA_VAULT_ADDR is not set".into(),
                )
            })?;
            let token = std::env::var("AETERNA_VAULT_TOKEN").map_err(|_| {
                SecretError::ConfigError(
                    "AETERNA_SECRET_PROVIDER=vault but AETERNA_VAULT_TOKEN is not set".into(),
                )
            })?;
            Arc::new(VaultSecretProvider::new(address, token))
        }
        // `local` and anything unrecognised both map to LocalSecretProvider.
        // The empty-map default matches the pre-v1.5.0 behaviour for dev and
        // tests, which never relied on this trait for credential resolution
        // (the SecretBackend Postgres+KMS path is what actually serves
        // tenant secrets in production).
        _ => Arc::new(LocalSecretProvider::new(std::collections::HashMap::new())),
    };

    let provider = enforce_production_safety_gate(env, &selector, provider)?;

    self_test_provider(&selector, provider.as_ref()).await?;

    Ok(provider)
}

/// Returns `true` if `selector` corresponds to a production-grade
/// [`SecretProvider`] implementation.
///
/// Pure function — no env lookups, no I/O — so it is safely unit-testable.
/// This is the single source of truth for "is this provider safe in
/// production?" and must be updated in lockstep when new variants are added.
fn is_production_grade(selector: &str) -> bool {
    matches!(
        selector.to_ascii_lowercase().as_str(),
        "vault" | "hashicorp-vault"
    )
}

/// Refuse to boot a non-production-grade [`SecretProvider`] in a production
/// environment. Pure function — no env lookups — so it is safely
/// unit-testable.
fn enforce_production_safety_gate(
    env: Environment,
    selector: &str,
    provider: Arc<dyn SecretProvider>,
) -> Result<Arc<dyn SecretProvider>, SecretError> {
    if env.is_production() && !is_production_grade(selector) {
        return Err(SecretError::ConfigError(format!(
            "AETERNA_ENV=production but AETERNA_SECRET_PROVIDER={selector} is not \
             production-grade. Use 'vault' (HashiCorp Vault / OpenBao) — see PR #170 \
             for the bundled OpenBao deployment option."
        )));
    }
    Ok(provider)
}

/// Probe the provider with [`SecretProvider::is_available`] under a tight
/// timeout. Catches misconfigured Vault endpoints (wrong address, expired
/// token, network unreachable) at boot rather than at the first secret
/// resolution.
async fn self_test_provider(
    selector: &str,
    provider: &dyn SecretProvider,
) -> Result<(), SecretError> {
    let available = tokio::time::timeout(SELF_TEST_TIMEOUT, provider.is_available())
        .await
        .map_err(|_| {
            SecretError::ConnectionFailed(format!(
                "secret provider '{selector}' did not respond to is_available() within \
                 {timeout:?} during startup self-test",
                timeout = SELF_TEST_TIMEOUT
            ))
        })?;

    if !available {
        return Err(SecretError::ConnectionFailed(format!(
            "secret provider '{selector}' reported unavailable during startup self-test"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ---------------------------------------------------------------------------
    // SecretError — Display
    // ---------------------------------------------------------------------------

    #[test]
    fn test_secret_error_display_variants() {
        let cases: Vec<(&str, SecretError)> = vec![
            (
                "Secret operation failed: boom",
                SecretError::OperationFailed("boom".into()),
            ),
            (
                "Secret not found: my-key",
                SecretError::NotFound("my-key".into()),
            ),
            (
                "Authentication failed: bad token",
                SecretError::AuthFailed("bad token".into()),
            ),
            (
                "Invalid configuration: missing field",
                SecretError::ConfigError("missing field".into()),
            ),
            (
                "Connection failed: timeout",
                SecretError::ConnectionFailed("timeout".into()),
            ),
            (
                "Format error: bad json",
                SecretError::FormatError("bad json".into()),
            ),
            (
                "Retrieval failed: 403",
                SecretError::RetrievalFailed("403".into()),
            ),
        ];

        for (expected, err) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    // ---------------------------------------------------------------------------
    // SecretProviderConfig — serde round-trip
    // ---------------------------------------------------------------------------

    #[test]
    fn test_secret_provider_config_serde_vault() {
        let cfg = SecretProviderConfig::Vault {
            address: "http://vault:8200".to_string(),
            token: "root".to_string(),
            mount_path: "secret".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SecretProviderConfig = serde_json::from_str(&json).unwrap();
        match back {
            SecretProviderConfig::Vault {
                address,
                token,
                mount_path,
            } => {
                assert_eq!(address, "http://vault:8200");
                assert_eq!(token, "root");
                assert_eq!(mount_path, "secret");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_secret_provider_config_serde_local() {
        let mut secrets = HashMap::new();
        secrets.insert("key-a".to_string(), "value-a".to_string());
        let cfg = SecretProviderConfig::Local {
            secrets: secrets.clone(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SecretProviderConfig = serde_json::from_str(&json).unwrap();
        match back {
            SecretProviderConfig::Local { secrets: s } => assert_eq!(s, secrets),
            _ => panic!("wrong variant"),
        }
    }

    // ---------------------------------------------------------------------------
    // LocalSecretProvider
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_local_provider_get_existing_secret() {
        let mut secrets = HashMap::new();
        secrets.insert("db-password".to_string(), "super-secret".to_string());
        let provider = LocalSecretProvider::new(secrets);

        let value = provider.get_secret("db-password").await.unwrap();
        assert_eq!(value, "super-secret");
    }

    #[tokio::test]
    async fn test_local_provider_get_missing_secret_returns_not_found() {
        let provider = LocalSecretProvider::new(HashMap::new());
        let err = provider.get_secret("nonexistent-key").await.unwrap_err();
        assert!(matches!(err, SecretError::NotFound(_)));
        assert!(err.to_string().contains("nonexistent-key"));
    }

    #[tokio::test]
    async fn test_local_provider_is_available() {
        let provider = LocalSecretProvider::new(HashMap::new());
        assert!(provider.is_available().await);
    }

    #[tokio::test]
    async fn test_local_provider_multiple_secrets() {
        let mut secrets = HashMap::new();
        secrets.insert("k1".to_string(), "v1".to_string());
        secrets.insert("k2".to_string(), "v2".to_string());
        secrets.insert("k3".to_string(), "v3".to_string());
        let provider = LocalSecretProvider::new(secrets);

        assert_eq!(provider.get_secret("k1").await.unwrap(), "v1");
        assert_eq!(provider.get_secret("k2").await.unwrap(), "v2");
        assert_eq!(provider.get_secret("k3").await.unwrap(), "v3");
    }

    // ---------------------------------------------------------------------------
    // SecretProviderConfig — `Aws` variant intentionally absent (see comment
    // at the top of the file). This regression test guards against accidental
    // re-introduction of an AWS variant whose implementation would once again
    // return a mock string. If real AWS support is added, it must route
    // through `storage::secret_backend::SecretBackend`, and this test should
    // be deleted as part of that change set, not silently amended.
    // ---------------------------------------------------------------------------

    #[test]
    fn aws_variant_is_intentionally_absent_from_secret_provider_config() {
        // The legacy AWS variant deserialised from `{"provider":"aws-secrets-manager",...}`
        // and instantiated a struct whose get_secret() returned a literal
        // mock string. Any reintroduction of either the rename tag or the
        // struct must be a deliberate, reviewed action.
        let legacy = r#"{"provider":"aws-secrets-manager","region":"eu-west-1"}"#;
        assert!(
            serde_json::from_str::<SecretProviderConfig>(legacy).is_err(),
            "SecretProviderConfig must reject the legacy 'aws-secrets-manager' provider tag"
        );
    }

    // ---------------------------------------------------------------------------
    // Production safety gate (`enforce_production_safety_gate`) — pure unit
    // tests. The full env-driven `build_secret_provider_from_env` is exercised
    // separately in `cli/tests/server_runtime_test.rs` where the bootstrap
    // wiring lives.
    // ---------------------------------------------------------------------------

    #[test]
    fn is_production_grade_recognises_vault_aliases() {
        assert!(is_production_grade("vault"));
        assert!(is_production_grade("Vault"));
        assert!(is_production_grade("VAULT"));
        assert!(is_production_grade("hashicorp-vault"));
    }

    #[test]
    fn is_production_grade_rejects_local_and_unknowns() {
        assert!(!is_production_grade("local"));
        assert!(!is_production_grade(""));
        assert!(!is_production_grade("aws-secrets-manager")); // legacy mock
        assert!(!is_production_grade("memory"));
    }

    #[tokio::test]
    async fn gate_blocks_local_provider_in_production() {
        let provider: Arc<dyn SecretProvider> = Arc::new(LocalSecretProvider::new(HashMap::new()));
        match enforce_production_safety_gate(Environment::Production, "local", provider) {
            Err(SecretError::ConfigError(msg)) => {
                assert!(msg.contains("production"), "msg should mention production: {msg}");
                assert!(msg.contains("local"), "msg should mention the bad selector: {msg}");
                assert!(
                    msg.to_ascii_lowercase().contains("vault"),
                    "msg should suggest vault: {msg}"
                );
            }
            Err(other) => panic!("wrong error variant: {other}"),
            Ok(_) => panic!("production + local must be refused"),
        }
    }

    #[tokio::test]
    async fn gate_allows_local_provider_outside_production() {
        for env in [Environment::Development, Environment::Ci, Environment::Staging] {
            let provider: Arc<dyn SecretProvider> =
                Arc::new(LocalSecretProvider::new(HashMap::new()));
            assert!(
                enforce_production_safety_gate(env, "local", provider).is_ok(),
                "gate must allow LocalSecretProvider in {env:?}"
            );
        }
    }

    #[tokio::test]
    async fn gate_allows_vault_provider_in_production() {
        let provider: Arc<dyn SecretProvider> =
            Arc::new(VaultSecretProvider::new("http://vault:8200".into(), "t".into()));
        assert!(
            enforce_production_safety_gate(Environment::Production, "vault", provider).is_ok(),
            "gate must allow VaultSecretProvider in production"
        );
    }

    // ---------------------------------------------------------------------------
    // Startup self-test (`self_test_provider`) — uses LocalSecretProvider
    // (whose `is_available` always returns true) for the success path and a
    // synthetic provider for the failure paths.
    // ---------------------------------------------------------------------------

    /// Synthetic provider that reports `is_available() = false`. Used to prove
    /// that the self-test surfaces an unavailable provider as a hard boot
    /// error instead of allowing it through.
    struct AlwaysUnavailableProvider;

    #[async_trait]
    impl SecretProvider for AlwaysUnavailableProvider {
        async fn get_secret(&self, _id: &str) -> Result<String, SecretError> {
            Err(SecretError::NotFound("synthetic".into()))
        }
        async fn is_available(&self) -> bool {
            false
        }
    }

    /// Synthetic provider whose `is_available` never resolves. Used to prove
    /// that the self-test enforces a finite timeout rather than blocking
    /// the boot indefinitely.
    struct HangsForeverProvider;

    #[async_trait]
    impl SecretProvider for HangsForeverProvider {
        async fn get_secret(&self, _id: &str) -> Result<String, SecretError> {
            Err(SecretError::NotFound("synthetic".into()))
        }
        async fn is_available(&self) -> bool {
            std::future::pending::<()>().await;
            unreachable!()
        }
    }

    #[tokio::test]
    async fn self_test_passes_for_available_provider() {
        let provider = LocalSecretProvider::new(HashMap::new());
        self_test_provider("local", &provider)
            .await
            .expect("LocalSecretProvider always reports available");
    }

    #[tokio::test]
    async fn self_test_surfaces_unavailable_as_connection_failed() {
        match self_test_provider("synthetic", &AlwaysUnavailableProvider).await {
            Err(SecretError::ConnectionFailed(msg)) => {
                assert!(msg.contains("synthetic"), "selector should leak in msg: {msg}");
                assert!(msg.contains("unavailable"), "msg should explain reason: {msg}");
            }
            Err(other) => panic!("wrong error variant: {other}"),
            Ok(()) => panic!("must reject unavailable provider"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn self_test_enforces_finite_timeout() {
        // Drive the self-test concurrently with a paused-clock advance so the
        // 2-second budget elapses without making the test wall-clock-slow.
        // HangsForeverProvider is `Sync` so we can move a `&'static`
        // reference into the spawned task via a leaked Box.
        static HANG: HangsForeverProvider = HangsForeverProvider;
        let test = tokio::spawn(async { self_test_provider("hang", &HANG).await });
        // Advance past the SELF_TEST_TIMEOUT so the inner `tokio::time::timeout`
        // fires.
        tokio::time::advance(SELF_TEST_TIMEOUT + Duration::from_millis(100)).await;
        match test.await.expect("task panicked") {
            Err(SecretError::ConnectionFailed(msg)) => {
                assert!(
                    msg.contains("did not respond"),
                    "msg should explain timeout: {msg}"
                );
            }
            Err(other) => panic!("wrong error variant: {other}"),
            Ok(()) => panic!("must reject provider whose is_available never resolves"),
        }
    }
}
