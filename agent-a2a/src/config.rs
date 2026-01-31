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
    pub telemetry: TelemetryConfig
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
        if let Ok(port) = std::env::var("AGENT_A2A_PORT") {
            if let Ok(p) = port.parse() {
                config.port = p;
            }
        }
        if let Ok(enabled) = std::env::var("AGENT_A2A_AUTH_ENABLED") {
            config.auth.enabled = enabled == "true";
        }
        if let Ok(key) = std::env::var("AGENT_A2A_AUTH_API_KEY") {
            config.auth.api_key = Some(key);
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
            telemetry: TelemetryConfig::default()
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default)]
    pub jwt_secret: Option<String>
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: None,
            jwt_secret: None
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,

    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16
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
            metrics_port: default_metrics_port()
        }
    }
}
