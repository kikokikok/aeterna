use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default)]
    pub auth: AuthConfig,

    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();

        if let Ok(addr) = std::env::var("AGENT_A2A_BIND_ADDRESS") {
            config.bind_address = addr;
        }
        if let Ok(port) = std::env::var("AGENT_A2A_PORT")
            && let Ok(p) = port.parse()
        {
            config.port = p;
        }
        if let Ok(enabled) = std::env::var("AGENT_A2A_AUTH_ENABLED") {
            config.auth.enabled = enabled == "true";
        }
        if let Ok(key) = std::env::var("AGENT_A2A_AUTH_API_KEY") {
            config.auth.api_key = Some(key);
        }
        if let Ok(secret) = std::env::var("AGENT_A2A_AUTH_JWT_SECRET") {
            config.auth.jwt_secret = Some(secret);
        }
        if let Ok(enabled) = std::env::var("AGENT_A2A_AUTH_TRUSTED_IDENTITY_ENABLED") {
            config.auth.trusted_identity.enabled = enabled == "true";
        }
        if let Ok(header) = std::env::var("AGENT_A2A_AUTH_TRUSTED_PROXY_HEADER") {
            config.auth.trusted_identity.proxy_header = header;
        }
        if let Ok(value) = std::env::var("AGENT_A2A_AUTH_TRUSTED_PROXY_HEADER_VALUE") {
            config.auth.trusted_identity.proxy_header_value = value;
        }
        if let Ok(header) = std::env::var("AGENT_A2A_AUTH_TRUSTED_TENANT_HEADER") {
            config.auth.trusted_identity.tenant_header = header;
        }
        if let Ok(header) = std::env::var("AGENT_A2A_AUTH_TRUSTED_USER_HEADER") {
            config.auth.trusted_identity.user_header = header;
        }
        if let Ok(header) = std::env::var("AGENT_A2A_AUTH_TRUSTED_EMAIL_HEADER") {
            config.auth.trusted_identity.email_header = header;
        }
        if let Ok(header) = std::env::var("AGENT_A2A_AUTH_TRUSTED_GROUPS_HEADER") {
            config.auth.trusted_identity.groups_header = header;
        }
        if let Ok(pattern) = std::env::var("AGENT_A2A_AUTH_TENANT_MAPPING_PATTERN") {
            config.auth.trusted_identity.tenant_mapping.pattern = pattern;
        }
        if let Ok(default_tenant) = std::env::var("AGENT_A2A_AUTH_TENANT_MAPPING_DEFAULT") {
            config.auth.trusted_identity.tenant_mapping.default_tenant = Some(default_tenant);
        }
        if let Ok(role_mappings) = std::env::var("AGENT_A2A_AUTH_ROLE_MAPPINGS") {
            config.auth.trusted_identity.role_mappings = role_mappings
                .split(';')
                .filter_map(|entry| entry.split_once('='))
                .map(|(group, roles)| RoleMapping {
                    group: group.trim().to_string(),
                    roles: roles
                        .split(',')
                        .map(str::trim)
                        .filter(|role| !role.is_empty())
                        .map(ToString::to_string)
                        .collect(),
                })
                .filter(|mapping| !mapping.group.is_empty() && !mapping.roles.is_empty())
                .collect();
        }

        Ok(config)
    }

    pub fn socket_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(format!("{}:{}", self.bind_address, self.port).parse()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: default_port(),
            auth: AuthConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default)]
    pub jwt_secret: Option<String>,

    #[serde(default)]
    pub trusted_identity: TrustedIdentityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrustedIdentityConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_proxy_header")]
    pub proxy_header: String,

    #[serde(default = "default_proxy_header_value")]
    pub proxy_header_value: String,

    #[serde(default = "default_tenant_header")]
    pub tenant_header: String,

    #[serde(default = "default_user_header")]
    pub user_header: String,

    #[serde(default = "default_email_header")]
    pub email_header: String,

    #[serde(default = "default_groups_header")]
    pub groups_header: String,

    #[serde(default)]
    pub tenant_mapping: TenantMappingConfig,

    #[serde(default)]
    pub role_mappings: Vec<RoleMapping>,
}

fn default_proxy_header() -> String {
    "x-auth-request-email".to_string()
}

fn default_proxy_header_value() -> String {
    "*".to_string()
}

fn default_tenant_header() -> String {
    "x-tenant-id".to_string()
}

fn default_user_header() -> String {
    "x-auth-request-user".to_string()
}

fn default_email_header() -> String {
    "x-auth-request-email".to_string()
}

fn default_groups_header() -> String {
    "x-auth-request-groups".to_string()
}

impl Default for TrustedIdentityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_header: default_proxy_header(),
            proxy_header_value: default_proxy_header_value(),
            tenant_header: default_tenant_header(),
            user_header: default_user_header(),
            email_header: default_email_header(),
            groups_header: default_groups_header(),
            tenant_mapping: TenantMappingConfig::default(),
            role_mappings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TenantMappingConfig {
    #[serde(default = "default_tenant_mapping_pattern")]
    pub pattern: String,

    #[serde(default)]
    pub default_tenant: Option<String>,
}

fn default_tenant_mapping_pattern() -> String {
    "{tenant}".to_string()
}

impl Default for TenantMappingConfig {
    fn default() -> Self {
        Self {
            pattern: default_tenant_mapping_pattern(),
            default_tenant: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoleMapping {
    pub group: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,

    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: default_metrics_enabled(),
            metrics_port: default_metrics_port(),
        }
    }
}
