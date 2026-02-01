mod generators;
mod types;
mod validators;
mod wizard;

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use colored::Colorize;

pub use types::*;
use wizard::SetupWizard;

use crate::output;

#[derive(Args)]
pub struct SetupArgs {
    /// Run in non-interactive mode (requires all options via flags)
    #[arg(long)]
    pub non_interactive: bool,

    /// Reconfigure existing setup
    #[arg(long)]
    pub reconfigure: bool,

    /// Validate existing configuration
    #[arg(long)]
    pub validate: bool,

    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Output directory for generated files
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    #[arg(long, value_enum, help = "Deployment target")]
    pub target: Option<DeploymentTarget>,

    #[arg(long, value_enum, help = "Deployment mode")]
    pub mode: Option<DeploymentMode>,

    #[arg(long, help = "Central server URL (required for hybrid/remote modes)")]
    pub central_url: Option<String>,

    #[arg(long, value_enum, help = "Authentication method for central server")]
    pub central_auth: Option<AuthMethod>,

    #[arg(
        long,
        env = "AETERNA_SETUP_API_KEY",
        help = "API key for central server"
    )]
    pub api_key: Option<String>,

    #[arg(long, value_enum, help = "Vector database backend")]
    pub vector_backend: Option<VectorBackend>,

    #[arg(long, value_enum, help = "Cache selection")]
    pub cache: Option<CacheType>,

    #[arg(long, help = "External Redis host")]
    pub redis_host: Option<String>,

    #[arg(long, default_value = "6379", help = "External Redis port")]
    pub redis_port: u16,

    #[arg(long, value_enum, help = "PostgreSQL selection")]
    pub postgresql: Option<PostgresqlType>,

    #[arg(long, help = "External PostgreSQL host")]
    pub pg_host: Option<String>,

    #[arg(long, default_value = "5432", help = "External PostgreSQL port")]
    pub pg_port: u16,

    #[arg(long, default_value = "aeterna", help = "External PostgreSQL database")]
    pub pg_database: String,

    #[arg(long, help = "Enable OPAL authorization stack")]
    pub opal: Option<bool>,

    #[arg(long, value_enum, help = "LLM provider")]
    pub llm: Option<LlmProvider>,

    #[arg(long, env = "OPENAI_API_KEY", help = "OpenAI API key")]
    pub openai_api_key: Option<String>,

    #[arg(long, env = "ANTHROPIC_API_KEY", help = "Anthropic API key")]
    pub anthropic_api_key: Option<String>,

    #[arg(
        long,
        default_value = "http://localhost:11434",
        help = "Ollama host URL"
    )]
    pub ollama_host: String,

    #[arg(long, help = "Enable OpenCode integration")]
    pub opencode: Option<bool>,

    #[arg(long, help = "Enable Ingress")]
    pub ingress: Option<bool>,

    #[arg(long, help = "Ingress hostname")]
    pub ingress_host: Option<String>,

    #[arg(long, help = "Enable ServiceMonitor for Prometheus")]
    pub service_monitor: Option<bool>,

    #[arg(long, help = "Enable NetworkPolicy")]
    pub network_policy: Option<bool>,

    #[arg(long, help = "Enable HorizontalPodAutoscaler")]
    pub hpa: Option<bool>,

    #[arg(long, help = "Enable PodDisruptionBudget")]
    pub pdb: Option<bool>
}

pub async fn run(args: SetupArgs) -> Result<()> {
    let output_dir = args.output.clone();
    let reconfigure = args.reconfigure;

    if args.validate {
        return run_validate(&output_dir);
    }

    if args.show {
        return run_show(&output_dir);
    }

    let config = if args.non_interactive {
        run_non_interactive(&args)?
    } else {
        let mut wizard = SetupWizard::new(reconfigure);
        wizard.run()?
    };

    let generated_files = generators::generate_all(&config, &output_dir)?;

    println!(
        "\n{}",
        "Configuration generated successfully!".green().bold()
    );
    println!("\n{}", "Generated files:".bold());
    for file in &generated_files {
        println!("  {} {}", "+".green(), file.display());
    }

    println!("\n{}", "Next steps:".bold());
    match config.target {
        DeploymentTarget::DockerCompose => {
            println!("  {} docker compose up -d", "$".dimmed());
            println!("  {} aeterna status", "$".dimmed());
        }
        DeploymentTarget::Kubernetes => {
            println!(
                "  {} helm install aeterna ./charts/aeterna -f values.yaml",
                "$".dimmed()
            );
            println!("  {} kubectl get pods -n aeterna", "$".dimmed());
            println!("  {} aeterna status", "$".dimmed());
        }
        DeploymentTarget::OpencodeOnly => {
            println!(
                "  {} Restart OpenCode to pick up the new MCP configuration",
                "1.".dimmed()
            );
            println!(
                "  {} Run 'aeterna status' to verify connectivity",
                "2.".dimmed()
            );
        }
    }

    Ok(())
}

fn run_validate(output_dir: &PathBuf) -> Result<()> {
    output::info("Validating configuration...");

    let config_path = output_dir.join(".aeterna").join("config.toml");
    if !config_path.exists() {
        output::error("No configuration found. Run 'aeterna setup' first.");
        return Ok(());
    }

    let validation_result = validators::validate_config(&config_path)?;

    if validation_result.is_valid {
        output::success("Configuration is valid");
    } else {
        output::error("Configuration has issues:");
        for issue in &validation_result.issues {
            println!("  {} {}", "-".red(), issue);
        }
    }

    Ok(())
}

fn run_show(output_dir: &PathBuf) -> Result<()> {
    let config_path = output_dir.join(".aeterna").join("config.toml");
    if !config_path.exists() {
        output::error("No configuration found. Run 'aeterna setup' first.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;

    println!("{}", "Current configuration:".bold());
    println!("{}", format!("({})", config_path.display()).dimmed());
    println!();

    let masked = mask_sensitive_values(&content);
    println!("{}", masked);

    Ok(())
}

fn run_non_interactive(args: &SetupArgs) -> Result<SetupConfig> {
    let target = args
        .target
        .ok_or_else(|| anyhow::anyhow!("--target is required in non-interactive mode"))?;

    let mode = args
        .mode
        .ok_or_else(|| anyhow::anyhow!("--mode is required in non-interactive mode"))?;

    if matches!(mode, DeploymentMode::Hybrid | DeploymentMode::Remote) {
        if args.central_url.is_none() {
            return Err(anyhow::anyhow!(
                "--central-url is required for hybrid/remote modes"
            ));
        }
    }

    let vector_backend = args.vector_backend.unwrap_or(VectorBackend::Qdrant);
    let cache = args.cache.unwrap_or(CacheType::Dragonfly);
    let postgresql = args.postgresql.unwrap_or(PostgresqlType::CloudNativePg);

    Ok(SetupConfig {
        target,
        mode,
        central_url: args.central_url.clone(),
        central_auth: args.central_auth.unwrap_or(AuthMethod::ApiKey),
        api_key: args.api_key.clone(),
        vector_backend,
        cache,
        redis_external: if matches!(cache, CacheType::External) {
            Some(ExternalRedisConfig {
                host: args
                    .redis_host
                    .clone()
                    .unwrap_or_else(|| "localhost".to_string()),
                port: args.redis_port,
                password: None
            })
        } else {
            None
        },
        postgresql,
        pg_external: if matches!(postgresql, PostgresqlType::External) {
            Some(ExternalPostgresConfig {
                host: args
                    .pg_host
                    .clone()
                    .unwrap_or_else(|| "localhost".to_string()),
                port: args.pg_port,
                database: args.pg_database.clone(),
                username: None,
                password: None
            })
        } else {
            None
        },
        opal_enabled: args.opal.unwrap_or(true),
        llm_provider: args.llm.unwrap_or(LlmProvider::None),
        openai_api_key: args.openai_api_key.clone(),
        anthropic_api_key: args.anthropic_api_key.clone(),
        ollama_host: Some(args.ollama_host.clone()),
        opencode_enabled: args.opencode.unwrap_or(false),
        ingress_enabled: args.ingress.unwrap_or(false),
        ingress_host: args.ingress_host.clone(),
        service_monitor_enabled: args.service_monitor.unwrap_or(false),
        network_policy_enabled: args.network_policy.unwrap_or(false),
        hpa_enabled: args.hpa.unwrap_or(false),
        pdb_enabled: args.pdb.unwrap_or(false)
    })
}

fn mask_sensitive_values(content: &str) -> String {
    let mut result = content.to_string();

    let patterns = [
        (
            r#"api[_-]?key\s*=\s*"[^"]+""#,
            r#"api_key = "***MASKED***""#
        ),
        (r#"password\s*=\s*"[^"]+""#, r#"password = "***MASKED***""#),
        (r#"token\s*=\s*"[^"]+""#, r#"token = "***MASKED***""#),
        (r#"secret\s*=\s*"[^"]+""#, r#"secret = "***MASKED***""#)
    ];

    for (pattern, replacement) in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, replacement).to_string();
        }
    }

    result
}
