//! B4 §3.4 — [`VaultRefResolver`].
//!
//! Resolves `SecretReference::Vault { mount, path, field }` against a
//! Vault-compatible KV-v2 API such as OpenBao.
//!
//! Authentication modes:
//!
//! * `VAULT_TOKEN` — use the provided token directly.
//! * Kubernetes auth — when `VAULT_TOKEN` is absent, use
//!   `VAULT_K8S_AUTH_ROLE` plus the pod's projected service-account JWT to
//!   log in at `VAULT_K8S_AUTH_PATH` (default: `auth/kubernetes`).
//!
//! Path conventions:
//!
//! * Global/shared secrets can live under a stable path such as
//!   `global/<domain>/<name>`.
//! * Tenant-scoped secrets can use `tenants/{tenant_id}/...`; the resolver
//!   substitutes `{tenant_id}` (and the aliases `{tenant}` / `{tenantId}`)
//!   from the current [`TenantId`] before issuing the KV read.
//!
//! Security notes:
//!
//! * Secret values and tokens are carried as [`mk_core::SecretBytes`] and
//!   never logged.
//! * Error messages include only status codes / structural diagnostics,
//!   never response bodies.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::secret_resolver::{ResolveError, SecretRefResolver};

const DEFAULT_K8S_AUTH_PATH: &str = "auth/kubernetes";
const DEFAULT_K8S_JWT_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
const DEFAULT_TIMEOUT_SECS: u64 = 5;
const TOKEN_REFRESH_BUFFER_SECS: u64 = 30;
const TOKEN_REFRESH_BUFFER_DIVISOR: u64 = 10;

#[derive(Debug, Clone)]
enum VaultAuthConfig {
    StaticToken(SecretBytes),
    Kubernetes {
        auth_path: String,
        role: String,
        jwt_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
struct VaultConfig {
    address: String,
    auth: VaultAuthConfig,
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: SecretBytes,
    expires_at: Instant,
}

#[derive(Debug)]
pub struct VaultRefResolver {
    client: reqwest::Client,
    config: Option<VaultConfig>,
    cached_token: Mutex<Option<CachedToken>>,
}

impl Default for VaultRefResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultRefResolver {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            config: Self::config_from_env().ok(),
            cached_token: Mutex::new(None),
        }
    }

    fn config_from_env() -> Result<VaultConfig, ResolveError> {
        let address =
            std::env::var("VAULT_ADDR").map_err(|_| ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "VAULT_ADDR is not set".to_string(),
            })?;
        let address = address.trim().trim_end_matches('/').to_string();
        if address.is_empty() {
            return Err(ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "VAULT_ADDR is empty".to_string(),
            });
        }

        if let Ok(token) = std::env::var("VAULT_TOKEN")
            && !token.trim().is_empty()
        {
            return Ok(VaultConfig {
                address,
                auth: VaultAuthConfig::StaticToken(SecretBytes::from_string(token)),
            });
        }

        let role =
            std::env::var("VAULT_K8S_AUTH_ROLE").map_err(|_| ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "VAULT_TOKEN is unset and VAULT_K8S_AUTH_ROLE is not set".to_string(),
            })?;
        if role.trim().is_empty() {
            return Err(ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "VAULT_K8S_AUTH_ROLE is empty".to_string(),
            });
        }

        let auth_path = std::env::var("VAULT_K8S_AUTH_PATH")
            .unwrap_or_else(|_| DEFAULT_K8S_AUTH_PATH.to_string());
        let auth_path = auth_path.trim().trim_matches('/').to_string();
        if auth_path.is_empty() {
            return Err(ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "VAULT_K8S_AUTH_PATH is empty".to_string(),
            });
        }

        let jwt_path = std::env::var("VAULT_K8S_JWT_PATH")
            .unwrap_or_else(|_| DEFAULT_K8S_JWT_PATH.to_string());
        let jwt_path = PathBuf::from(jwt_path);

        Ok(VaultConfig {
            address,
            auth: VaultAuthConfig::Kubernetes {
                auth_path,
                role,
                jwt_path,
            },
        })
    }

    async fn access_token(&self) -> Result<SecretBytes, ResolveError> {
        let Some(config) = &self.config else {
            return Err(ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "Vault resolver is not configured from environment".to_string(),
            });
        };

        match &config.auth {
            VaultAuthConfig::StaticToken(token) => Ok(token.clone()),
            VaultAuthConfig::Kubernetes {
                auth_path,
                role,
                jwt_path,
            } => {
                {
                    let cached = self.cached_token.lock().await;
                    if let Some(entry) = &*cached
                        && entry.expires_at > Instant::now()
                    {
                        return Ok(entry.token.clone());
                    }
                }

                let jwt = Self::read_jwt(jwt_path)?;
                let url = format!(
                    "{}/v1/{}/login",
                    config.address,
                    auth_path.trim_matches('/')
                );
                let jwt_str = std::str::from_utf8(jwt.expose()).map_err(|_| {
                    ResolveError::BackendUnavailable {
                        kind: "vault",
                        reason: "projected Kubernetes JWT is not valid UTF-8".to_string(),
                    }
                })?;

                let response = self
                    .client
                    .post(url)
                    .json(&json!({
                        "role": role,
                        "jwt": jwt_str,
                    }))
                    .send()
                    .await
                    .map_err(|e| ResolveError::BackendUnavailable {
                        kind: "vault",
                        reason: format!("Vault Kubernetes login request failed: {e}"),
                    })?;

                match response.status().as_u16() {
                    200 => {}
                    401 | 403 => {
                        return Err(ResolveError::PermissionDenied {
                            kind: "vault",
                            reason: format!(
                                "Vault Kubernetes login denied (HTTP {}) for role {role}",
                                response.status().as_u16()
                            ),
                        });
                    }
                    status => {
                        return Err(ResolveError::BackendUnavailable {
                            kind: "vault",
                            reason: format!(
                                "Vault Kubernetes login failed with HTTP {status} at {auth_path}"
                            ),
                        });
                    }
                }

                let body = response.json::<LoginResponse>().await.map_err(|e| {
                    ResolveError::BackendUnavailable {
                        kind: "vault",
                        reason: format!("Vault Kubernetes login response parse failed: {e}"),
                    }
                })?;

                let auth = body.auth.ok_or_else(|| ResolveError::BackendUnavailable {
                    kind: "vault",
                    reason: "Vault Kubernetes login response missing auth block".to_string(),
                })?;
                if auth.client_token.trim().is_empty() {
                    return Err(ResolveError::BackendUnavailable {
                        kind: "vault",
                        reason: "Vault Kubernetes login response returned an empty token"
                            .to_string(),
                    });
                }

                let token = SecretBytes::from_string(auth.client_token);
                let ttl = auth.lease_duration.unwrap_or(300);
                let refresh_after = std::cmp::max(
                    1,
                    ttl.saturating_sub(std::cmp::min(
                        TOKEN_REFRESH_BUFFER_SECS,
                        ttl / TOKEN_REFRESH_BUFFER_DIVISOR,
                    )),
                );
                let cached = CachedToken {
                    token: token.clone(),
                    expires_at: Instant::now() + Duration::from_secs(refresh_after),
                };
                *self.cached_token.lock().await = Some(cached);
                Ok(token)
            }
        }
    }

    fn read_jwt(path: &Path) -> Result<SecretBytes, ResolveError> {
        let raw = std::fs::read(path).map_err(|e| ResolveError::BackendUnavailable {
            kind: "vault",
            reason: format!("read projected Kubernetes JWT {}: {e}", path.display()),
        })?;
        if raw.is_empty() {
            return Err(ResolveError::BackendUnavailable {
                kind: "vault",
                reason: format!("projected Kubernetes JWT {} is empty", path.display()),
            });
        }
        Ok(SecretBytes::from(raw))
    }

    fn ensure_reference(reference: &SecretReference) -> Result<(&str, &str, &str), ResolveError> {
        let SecretReference::Vault { mount, path, field } = reference else {
            return Err(ResolveError::WrongKind {
                expected: "vault",
                actual: reference.kind(),
            });
        };

        for (label, value) in [
            ("mount", mount.as_str()),
            ("path", path.as_str()),
            ("field", field.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ResolveError::MalformedReference {
                    kind: "vault",
                    reason: format!("{label} is empty"),
                });
            }
        }

        Ok((
            mount.trim_matches('/'),
            path.trim_start_matches('/'),
            field.as_str(),
        ))
    }

    fn expand_path(path: &str, tenant: &TenantId) -> String {
        let tenant_id = tenant.as_str();
        path.replace("{tenant_id}", tenant_id)
            .replace("{tenantId}", tenant_id)
            .replace("{tenant}", tenant_id)
    }
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    auth: Option<LoginAuth>,
}

#[derive(Debug, Deserialize)]
struct LoginAuth {
    client_token: String,
    lease_duration: Option<u64>,
}

#[async_trait]
impl SecretRefResolver for VaultRefResolver {
    fn kind(&self) -> &'static str {
        "vault"
    }

    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        let (mount, path, field) = Self::ensure_reference(reference)?;
        let Some(config) = &self.config else {
            return Err(ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "Vault resolver is not configured from environment".to_string(),
            });
        };

        let token = self.access_token().await?;
        let token_str =
            std::str::from_utf8(token.expose()).map_err(|_| ResolveError::BackendUnavailable {
                kind: "vault",
                reason: "Vault token is not valid UTF-8".to_string(),
            })?;
        let path = Self::expand_path(path, tenant);
        let url = format!("{}/v1/{}/data/{}", config.address, mount, path);

        let response = self
            .client
            .get(url)
            .header("X-Vault-Token", token_str)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ResolveError::BackendUnavailable {
                kind: "vault",
                reason: format!("Vault KV read failed: {e}"),
            })?;

        match response.status().as_u16() {
            200 => {}
            401 | 403 => {
                return Err(ResolveError::PermissionDenied {
                    kind: "vault",
                    reason: format!("Vault KV read denied (HTTP {})", response.status().as_u16()),
                });
            }
            404 => {
                return Err(ResolveError::NotFound {
                    tenant: tenant.clone(),
                    kind: "vault",
                });
            }
            status => {
                return Err(ResolveError::BackendUnavailable {
                    kind: "vault",
                    reason: format!("Vault KV read failed with HTTP {status}"),
                });
            }
        }

        let body = response.json::<serde_json::Value>().await.map_err(|e| {
            ResolveError::BackendUnavailable {
                kind: "vault",
                reason: format!("Vault KV response parse failed: {e}"),
            }
        })?;

        let Some(value) = body
            .get("data")
            .and_then(|data| data.get("data"))
            .and_then(|data| data.get(field))
        else {
            return Err(ResolveError::NotFound {
                tenant: tenant.clone(),
                kind: "vault",
            });
        };

        if let Some(s) = value.as_str() {
            return Ok(SecretBytes::from_string(s.to_string()));
        }

        serde_json::to_vec(value)
            .map(SecretBytes::from)
            .map_err(|e| ResolveError::BackendUnavailable {
                kind: "vault",
                reason: format!("Vault field serialization failed: {e}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use tempfile::NamedTempFile;
    use tokio::sync::Mutex as TokioMutex;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn vault_ref() -> SecretReference {
        SecretReference::Vault {
            mount: "secret".to_string(),
            path: "apps/demo".to_string(),
            field: "api_key".to_string(),
        }
    }

    static ENV_LOCK: std::sync::LazyLock<Arc<TokioMutex<()>>> =
        std::sync::LazyLock::new(|| Arc::new(TokioMutex::new(())));

    fn clear_vault_env() {
        for key in [
            "VAULT_ADDR",
            "VAULT_TOKEN",
            "VAULT_K8S_AUTH_ROLE",
            "VAULT_K8S_AUTH_PATH",
            "VAULT_K8S_JWT_PATH",
        ] {
            unsafe { std::env::remove_var(key) };
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reports_kind_vault() {
        assert_eq!(VaultRefResolver::new().kind(), "vault");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resolves_with_static_token() {
        let _guard = ENV_LOCK.lock().await;
        clear_vault_env();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/secret/data/apps/demo"))
            .and(header("x-vault-token", "root-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "data": { "api_key": "hunter2" } }
            })))
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var("VAULT_ADDR", server.uri());
            std::env::set_var("VAULT_TOKEN", "root-token");
        }

        let resolver = VaultRefResolver::new();
        let out = resolver.resolve(&tid(), &vault_ref()).await.unwrap();
        assert_eq!(out.expose(), b"hunter2");

        clear_vault_env();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resolves_with_kubernetes_auth_and_caches_token() {
        let _guard = ENV_LOCK.lock().await;
        clear_vault_env();

        let jwt_file = NamedTempFile::new().unwrap();
        std::fs::write(jwt_file.path(), "jwt-token").unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/auth/kubernetes/login"))
            .and(body_json(json!({
                "role": "aeterna",
                "jwt": "jwt-token",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "auth": {
                    "client_token": "issued-token",
                    "lease_duration": 600
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/secret/data/apps/demo"))
            .and(header("x-vault-token", "issued-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "data": { "api_key": "hunter2" } }
            })))
            .expect(2)
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var("VAULT_ADDR", server.uri());
            std::env::set_var("VAULT_K8S_AUTH_ROLE", "aeterna");
            std::env::set_var("VAULT_K8S_AUTH_PATH", "auth/kubernetes");
            std::env::set_var("VAULT_K8S_JWT_PATH", jwt_file.path());
        }

        let resolver = VaultRefResolver::new();
        let first = resolver.resolve(&tid(), &vault_ref()).await.unwrap();
        let second = resolver.resolve(&tid(), &vault_ref()).await.unwrap();
        assert_eq!(first.expose(), b"hunter2");
        assert_eq!(second.expose(), b"hunter2");

        clear_vault_env();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn expands_tenant_placeholders_in_secret_path() {
        let _guard = ENV_LOCK.lock().await;
        clear_vault_env();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/v1/secret/data/tenants/11111111-1111-1111-1111-111111111111/llm/openai",
            ))
            .and(header("x-vault-token", "root-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "data": { "api_key": "tenant-token" } }
            })))
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var("VAULT_ADDR", server.uri());
            std::env::set_var("VAULT_TOKEN", "root-token");
        }

        let resolver = VaultRefResolver::new();
        let out = resolver
            .resolve(
                &tid(),
                &SecretReference::Vault {
                    mount: "secret".to_string(),
                    path: "tenants/{tenant_id}/llm/openai".to_string(),
                    field: "api_key".to_string(),
                },
            )
            .await
            .unwrap();
        assert_eq!(out.expose(), b"tenant-token");

        clear_vault_env();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn expands_all_supported_tenant_placeholder_aliases() {
        let _guard = ENV_LOCK.lock().await;
        clear_vault_env();

        let server = MockServer::start().await;
        for suffix in ["{tenant_id}", "{tenantId}", "{tenant}"] {
            Mock::given(method("GET"))
                .and(path(
                    "/v1/secret/data/tenants/11111111-1111-1111-1111-111111111111/shared/key",
                ))
                .and(header("x-vault-token", "root-token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "data": { "data": { "value": "ok" } }
                })))
                .up_to_n_times(1)
                .mount(&server)
                .await;

            unsafe {
                std::env::set_var("VAULT_ADDR", server.uri());
                std::env::set_var("VAULT_TOKEN", "root-token");
            }

            let resolver = VaultRefResolver::new();
            let out = resolver
                .resolve(
                    &tid(),
                    &SecretReference::Vault {
                        mount: "secret".to_string(),
                        path: format!("tenants/{suffix}/shared/key"),
                        field: "value".to_string(),
                    },
                )
                .await
                .unwrap();
            assert_eq!(out.expose(), b"ok");
            clear_vault_env();
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_field_is_not_found() {
        let _guard = ENV_LOCK.lock().await;
        clear_vault_env();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/secret/data/apps/demo"))
            .and(header("x-vault-token", "root-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "data": { "other": "value" } }
            })))
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var("VAULT_ADDR", server.uri());
            std::env::set_var("VAULT_TOKEN", "root-token");
        }

        let resolver = VaultRefResolver::new();
        let err = resolver.resolve(&tid(), &vault_ref()).await.unwrap_err();
        assert!(matches!(err, ResolveError::NotFound { kind: "vault", .. }));

        clear_vault_env();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn empty_mount_is_malformed() {
        let _guard = ENV_LOCK.lock().await;
        clear_vault_env();
        unsafe {
            std::env::set_var("VAULT_ADDR", "http://vault.local");
            std::env::set_var("VAULT_TOKEN", "root-token");
        }

        let resolver = VaultRefResolver::new();
        let err = resolver
            .resolve(
                &tid(),
                &SecretReference::Vault {
                    mount: "".to_string(),
                    path: "apps/demo".to_string(),
                    field: "api_key".to_string(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ResolveError::MalformedReference { kind: "vault", .. }
        ));

        clear_vault_env();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wrong_kind_is_rejected() {
        let resolver = VaultRefResolver::new();
        let env = SecretReference::Env {
            var: "X".to_string(),
        };
        let err = resolver.resolve(&tid(), &env).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::WrongKind {
                expected: "vault",
                actual: "env"
            }
        ));
    }
}
