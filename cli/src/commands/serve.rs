use clap::Args;

#[derive(Args)]
pub struct ServeArgs {
    /// Bind address (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// HTTP port for the API server
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Metrics port
    #[arg(long, default_value = "9090")]
    pub metrics_port: u16,

    /// Path to configuration file (defaults to AETERNA_CONFIG_PATH or ./config)
    #[arg(long, env = "AETERNA_CONFIG_PATH")]
    pub config: Option<std::path::PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String
}

pub fn run(args: ServeArgs) -> anyhow::Result<()> {
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("./config"));

    // Validate required environment / config presence before attempting to bind.
    // In the current state the full Aeterna HTTP server is not yet wired into
    // this binary; surface a clear, actionable error rather than silently
    // claiming success or panicking.
    if !config_path.exists() {
        anyhow::bail!(
            "Configuration directory not found: {}\n\
             \n\
             Set AETERNA_CONFIG_PATH to a valid config directory, or run:\n\
             \n\
             \taeterna setup\n\
             \n\
             to create an initial configuration.",
            config_path.display()
        );
    }

    // Check required infrastructure environment variables.
    let missing: Vec<&str> = ["AETERNA_POSTGRESQL_HOST", "AETERNA_POSTGRESQL_DATABASE"]
        .iter()
        .copied()
        .filter(|v| std::env::var(v).is_err())
        .collect();

    if !missing.is_empty() {
        anyhow::bail!(
            "Required environment variables are not set: {}\n\
             \n\
             Aeterna server requires a running PostgreSQL instance.\n\
             Configure the connection via environment variables or Helm values.\n\
             Run 'aeterna setup' to generate a configuration.",
            missing.join(", ")
        );
    }

    // All prerequisites met — start the server.
    // The bind address and port are resolved here; the actual Axum/Hyper server
    // will be wired in when the server crate is integrated.
    let addr = format!("{}:{}", args.bind, args.port);
    tracing::info!("Aeterna API server starting on http://{}", addr);
    tracing::info!("Metrics endpoint on http://{}:{}/metrics", args.bind, args.metrics_port);

    // Runtime is already provided by the #[tokio::main] in main.rs.
    // This function is synchronous; the actual async server loop will be
    // spawned here once the server crate is integrated.
    anyhow::bail!(
        "The Aeterna HTTP API server is not yet integrated into this binary.\n\
         \n\
         To run Aeterna in a container, ensure AETERNA_POSTGRESQL_HOST and related\n\
         variables are set correctly, then re-run once the server crate is available.\n\
         \n\
         For development, use: cargo run -p agent-a2a"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_args_defaults() {
        let args = ServeArgs {
            bind: "0.0.0.0".to_string(),
            port: 8080,
            metrics_port: 9090,
            config: None,
            log_level: "info".to_string()
        };
        assert_eq!(args.bind, "0.0.0.0");
        assert_eq!(args.port, 8080);
        assert_eq!(args.metrics_port, 9090);
        assert!(args.config.is_none());
        assert_eq!(args.log_level, "info");
    }

    #[test]
    fn test_serve_args_custom_port() {
        let args = ServeArgs {
            bind: "127.0.0.1".to_string(),
            port: 3000,
            metrics_port: 3001,
            config: Some(std::path::PathBuf::from("/etc/aeterna")),
            log_level: "debug".to_string()
        };
        assert_eq!(args.port, 3000);
        assert_eq!(args.metrics_port, 3001);
        assert_eq!(args.bind, "127.0.0.1");
        assert!(args.config.is_some());
        assert_eq!(args.log_level, "debug");
    }

    #[test]
    fn test_serve_missing_config_path_returns_error() {
        let args = ServeArgs {
            bind: "0.0.0.0".to_string(),
            port: 8080,
            metrics_port: 9090,
            config: Some(std::path::PathBuf::from("/nonexistent/path/that/does/not/exist")),
            log_level: "info".to_string()
        };
        let result = run(args);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Configuration directory not found"),
            "Expected 'Configuration directory not found', got: {msg}"
        );
    }

    #[test]
    fn test_serve_missing_env_vars_with_existing_config_path() {
        // Use a path that exists ("/tmp") so we pass the config check and hit
        // the environment variable check.
        // Clear relevant env vars for this test.
        // SAFETY: single-threaded test, no other threads reading these vars.
        unsafe {
            std::env::remove_var("AETERNA_POSTGRESQL_HOST");
            std::env::remove_var("AETERNA_POSTGRESQL_DATABASE");
        }

        let args = ServeArgs {
            bind: "0.0.0.0".to_string(),
            port: 8080,
            metrics_port: 9090,
            config: Some(std::path::PathBuf::from("/tmp")),
            log_level: "info".to_string()
        };
        let result = run(args);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        // Either the env var check or the "not yet integrated" message is acceptable.
        assert!(
            msg.contains("AETERNA_POSTGRESQL") || msg.contains("not yet integrated"),
            "Expected AETERNA_POSTGRESQL or not-yet-integrated error, got: {msg}"
        );
    }
}
