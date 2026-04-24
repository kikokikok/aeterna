use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::config::{RoleMapping, TrustedIdentityConfig};

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub user_email: Option<String>,
    pub groups: Vec<String>,
    pub roles: Vec<String>,
}

impl TenantContext {
    pub fn from_headers(request: &Request) -> Option<Self> {
        Self::from_standard_headers(request.headers())
    }

    pub fn from_standard_headers(headers: &HeaderMap) -> Option<Self> {
        let tenant_id = header_value(headers, "x-tenant-id")?;

        let user_id = header_value(headers, "x-user-id");

        let agent_id = header_value(headers, "x-agent-id");

        Some(Self {
            tenant_id,
            user_id,
            agent_id,
            user_email: None,
            groups: Vec::new(),
            roles: Vec::new(),
        })
    }

    pub fn from_trusted_identity_headers(
        headers: &HeaderMap,
        config: &TrustedIdentityConfig,
    ) -> Option<Self> {
        let tenant_hint = header_value(headers, &config.tenant_header)?;
        let user_id = header_value(headers, &config.user_header)?;
        let user_email = header_value(headers, &config.email_header)?;
        let groups = header_value(headers, &config.groups_header)
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|group| !group.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let tenant_id = config.resolve_tenant(&tenant_hint, &user_email)?;
        let roles = config.resolve_roles(&groups)?;

        Some(Self {
            tenant_id,
            user_id: Some(user_id),
            agent_id: None,
            user_email: Some(user_email),
            groups,
            roles,
        })
    }

    pub fn has_trusted_identity_headers(
        headers: &HeaderMap,
        config: &TrustedIdentityConfig,
    ) -> bool {
        [
            config.proxy_header.as_str(),
            config.tenant_header.as_str(),
            config.user_header.as_str(),
            config.email_header.as_str(),
            config.groups_header.as_str(),
        ]
        .iter()
        .any(|header| headers.contains_key(*header))
    }

    pub fn trusted_proxy_verified(headers: &HeaderMap, config: &TrustedIdentityConfig) -> bool {
        header_value(headers, &config.proxy_header).is_some_and(|value| {
            config.proxy_header_value == "*" || value == config.proxy_header_value
        })
    }

    pub fn from_auth_header(auth_header: &str) -> Option<Self> {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            Self::from_jwt(token)
        } else {
            None
        }
    }

    fn from_jwt(_token: &str) -> Option<Self> {
        None
    }
}

impl TrustedIdentityConfig {
    fn resolve_tenant(&self, tenant_hint: &str, user_email: &str) -> Option<String> {
        let candidate = self
            .tenant_mapping
            .pattern
            .replace("{tenant}", tenant_hint)
            .replace("{email}", user_email);

        let resolved = if candidate.trim().is_empty() {
            self.tenant_mapping.default_tenant.clone()?
        } else {
            candidate
        };

        let normalized = resolved.trim().to_string();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }

    fn resolve_roles(&self, groups: &[String]) -> Option<Vec<String>> {
        let roles = self
            .role_mappings
            .iter()
            .filter(|mapping| groups.iter().any(|group| group == &mapping.group))
            .flat_map(|mapping| mapping.roles.iter().cloned())
            .fold(Vec::<String>::new(), |mut acc, role| {
                if !acc.contains(&role) {
                    acc.push(role);
                }
                acc
            });

        if roles.is_empty() { None } else { Some(roles) }
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[derive(Clone)]
pub struct AuthState {
    pub api_key: Option<String>,
    pub jwt_secret: Option<String>,
    pub enabled: bool,
    pub trusted_identity: TrustedIdentityConfig,
}

impl AuthState {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.api_key.is_some() {
            return Ok(());
        }

        if self.trusted_identity.enabled {
            if self.trusted_identity.proxy_header.trim().is_empty()
                || self.trusted_identity.tenant_header.trim().is_empty()
                || self.trusted_identity.user_header.trim().is_empty()
                || self.trusted_identity.email_header.trim().is_empty()
                || self.trusted_identity.groups_header.trim().is_empty()
            {
                anyhow::bail!(
                    "Trusted identity auth is enabled but one or more required header names are empty."
                );
            }
            if self
                .trusted_identity
                .tenant_mapping
                .pattern
                .trim()
                .is_empty()
                && self
                    .trusted_identity
                    .tenant_mapping
                    .default_tenant
                    .is_none()
            {
                anyhow::bail!(
                    "Trusted identity auth is enabled but no tenant mapping pattern or default tenant is configured."
                );
            }
            if self.trusted_identity.role_mappings.is_empty() {
                anyhow::bail!(
                    "Trusted identity auth is enabled but no Okta group-to-role mappings are configured."
                );
            }
            for RoleMapping { group, roles } in &self.trusted_identity.role_mappings {
                if group.trim().is_empty()
                    || roles.is_empty()
                    || roles.iter().any(|role| role.trim().is_empty())
                {
                    anyhow::bail!(
                        "Trusted identity auth has an invalid role mapping with an empty group or role."
                    );
                }
            }
            return Ok(());
        }

        if self.jwt_secret.is_some() {
            anyhow::bail!(
                "JWT auth is configured but not yet implemented. Set AGENT_A2A_AUTH_API_KEY or disable auth."
            );
        }

        anyhow::bail!(
            "Authentication is enabled but no supported auth backend is configured. Set AGENT_A2A_AUTH_API_KEY or disable auth."
        )
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.validate().is_ok()
    }
}

pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.enabled {
        return Ok(next.run(request).await);
    }

    if let Some(api_key) = &state.api_key {
        if let Some(auth_header) = request.headers().get(AUTHORIZATION)
            && let Ok(auth_str) = auth_header.to_str()
            && auth_str == format!("Bearer {api_key}")
        {
            return Ok(next.run(request).await);
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    if state.trusted_identity.enabled {
        let headers = request.headers();
        if !TenantContext::has_trusted_identity_headers(headers, &state.trusted_identity) {
            return Err(StatusCode::UNAUTHORIZED);
        }

        if !TenantContext::trusted_proxy_verified(headers, &state.trusted_identity) {
            return Err(StatusCode::UNAUTHORIZED);
        }

        if TenantContext::from_trusted_identity_headers(headers, &state.trusted_identity).is_none()
        {
            return Err(StatusCode::UNAUTHORIZED);
        }

        return Ok(next.run(request).await);
    }

    // Auth is enabled but no api_key configured → JWT mode, which is not yet implemented.
    // Reject all requests rather than allowing them through silently.
    Err(StatusCode::UNAUTHORIZED)
}

pub async fn tenant_context_middleware(mut request: Request, next: Next) -> Response {
    if let Some(auth_state) = request.extensions().get::<Arc<AuthState>>()
        && auth_state.trusted_identity.enabled
        && let Some(tenant_context) = TenantContext::from_trusted_identity_headers(
            request.headers(),
            &auth_state.trusted_identity,
        )
    {
        request.extensions_mut().insert(tenant_context);
        return next.run(request).await;
    }

    if let Some(tenant_context) = TenantContext::from_headers(&request) {
        request.extensions_mut().insert(tenant_context);
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::AuthState;
    use crate::config::{RoleMapping, TrustedIdentityConfig};

    #[test]
    fn test_auth_state_validate_disabled_auth() {
        let state = AuthState {
            api_key: None,
            jwt_secret: None,
            enabled: false,
            trusted_identity: TrustedIdentityConfig::default(),
        };

        assert!(state.validate().is_ok());
        assert!(state.is_ready());
    }

    #[test]
    fn test_auth_state_validate_api_key_auth() {
        let state = AuthState {
            api_key: Some("secret".to_string()),
            jwt_secret: None,
            enabled: true,
            trusted_identity: TrustedIdentityConfig::default(),
        };

        assert!(state.validate().is_ok());
        assert!(state.is_ready());
    }

    #[test]
    fn test_auth_state_validate_rejects_unimplemented_jwt_mode() {
        let state = AuthState {
            api_key: None,
            jwt_secret: Some("jwt-secret".to_string()),
            enabled: true,
            trusted_identity: TrustedIdentityConfig::default(),
        };

        let err = state
            .validate()
            .expect_err("jwt-only auth must be rejected");
        assert!(
            err.to_string()
                .contains("JWT auth is configured but not yet implemented")
        );
        assert!(!state.is_ready());
    }

    #[test]
    fn test_auth_state_validate_rejects_missing_backend() {
        let state = AuthState {
            api_key: None,
            jwt_secret: None,
            enabled: true,
            trusted_identity: TrustedIdentityConfig::default(),
        };

        let err = state
            .validate()
            .expect_err("enabled auth without backend must be rejected");
        assert!(
            err.to_string()
                .contains("no supported auth backend is configured")
        );
        assert!(!state.is_ready());
    }

    #[test]
    fn test_auth_state_validate_trusted_identity_auth() {
        let state = AuthState {
            api_key: None,
            jwt_secret: None,
            enabled: true,
            trusted_identity: TrustedIdentityConfig {
                enabled: true,
                role_mappings: vec![RoleMapping {
                    group: "aeterna-users".to_string(),
                    roles: vec!["viewer".to_string()],
                }],
                ..TrustedIdentityConfig::default()
            },
        };

        assert!(state.validate().is_ok());
        assert!(state.is_ready());
    }

    #[test]
    fn test_auth_state_validate_rejects_missing_role_mappings() {
        let state = AuthState {
            api_key: None,
            jwt_secret: None,
            enabled: true,
            trusted_identity: TrustedIdentityConfig {
                enabled: true,
                ..TrustedIdentityConfig::default()
            },
        };

        let err = state
            .validate()
            .expect_err("missing role mappings must fail closed");
        assert!(err.to_string().contains("group-to-role mappings"));
    }
}
