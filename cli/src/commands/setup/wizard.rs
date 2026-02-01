use anyhow::Result;
use colored::Colorize;
use console::Term;
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};

use super::types::*;

pub struct SetupWizard {
    term: Term,
    theme: ColorfulTheme,
    reconfigure: bool,
    config: SetupConfig
}

impl SetupWizard {
    pub fn new(reconfigure: bool) -> Self {
        Self {
            term: Term::stderr(),
            theme: ColorfulTheme::default(),
            reconfigure,
            config: SetupConfig::default()
        }
    }

    pub fn run(&mut self) -> Result<SetupConfig> {
        self.print_welcome();

        self.select_deployment_target()?;
        self.select_deployment_mode()?;

        if matches!(
            self.config.mode,
            DeploymentMode::Hybrid | DeploymentMode::Remote
        ) {
            self.configure_central_server()?;
        }

        if matches!(
            self.config.mode,
            DeploymentMode::Local | DeploymentMode::Hybrid
        ) {
            self.select_vector_backend()?;
            self.select_cache()?;
            self.select_postgresql()?;
        }

        self.configure_opal()?;
        self.configure_llm()?;
        self.configure_opencode()?;
        self.configure_advanced_options()?;

        Ok(self.config.clone())
    }

    fn print_welcome(&self) {
        println!();
        println!("{}", "Welcome to Aeterna Setup Wizard!".bold().cyan());
        println!(
            "{}",
            "This wizard will help you configure your deployment.".dimmed()
        );
        println!();
    }

    fn select_deployment_target(&mut self) -> Result<()> {
        let options = vec![
            "Local development (Docker Compose)",
            "Kubernetes (Helm chart)",
            "OpenCode configuration only",
        ];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Deployment target")
            .items(&options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.target = match selection {
            0 => DeploymentTarget::DockerCompose,
            1 => DeploymentTarget::Kubernetes,
            2 => DeploymentTarget::OpencodeOnly,
            _ => unreachable!()
        };

        Ok(())
    }

    fn select_deployment_mode(&mut self) -> Result<()> {
        let options = vec![
            "Local (all components, self-contained)",
            "Hybrid (local cache + central server)",
            "Remote (thin client only)",
        ];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Deployment mode")
            .items(&options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.mode = match selection {
            0 => DeploymentMode::Local,
            1 => DeploymentMode::Hybrid,
            2 => DeploymentMode::Remote,
            _ => unreachable!()
        };

        Ok(())
    }

    fn configure_central_server(&mut self) -> Result<()> {
        let url: String = Input::with_theme(&self.theme)
            .with_prompt("Central server URL")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.starts_with("http://") || input.starts_with("https://") {
                    Ok(())
                } else {
                    Err("URL must start with http:// or https://")
                }
            })
            .interact_text()?;

        self.config.central_url = Some(url);

        let auth_options = vec!["API Key", "OAuth2 / SSO", "Service Account"];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Authentication method")
            .items(&auth_options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.central_auth = match selection {
            0 => AuthMethod::ApiKey,
            1 => AuthMethod::Oauth2,
            2 => AuthMethod::ServiceAccount,
            _ => unreachable!()
        };

        if matches!(self.config.central_auth, AuthMethod::ApiKey) {
            if let Ok(key) = std::env::var("AETERNA_SETUP_API_KEY") {
                println!("{}", "Using API key from AETERNA_SETUP_API_KEY".dimmed());
                self.config.api_key = Some(key);
            } else {
                let key: String = Input::with_theme(&self.theme)
                    .with_prompt("API Key")
                    .interact_text()?;
                self.config.api_key = Some(key);
            }
        }

        Ok(())
    }

    fn select_vector_backend(&mut self) -> Result<()> {
        let options = vec![
            "Qdrant (default, self-hosted)",
            "pgvector (PostgreSQL extension)",
            "Pinecone (managed cloud)",
            "Weaviate (hybrid search)",
            "MongoDB Atlas (managed)",
            "Vertex AI (Google Cloud)",
            "Databricks (Unity Catalog)",
        ];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Vector database backend")
            .items(&options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.vector_backend = match selection {
            0 => VectorBackend::Qdrant,
            1 => VectorBackend::Pgvector,
            2 => VectorBackend::Pinecone,
            3 => VectorBackend::Weaviate,
            4 => VectorBackend::Mongodb,
            5 => VectorBackend::VertexAi,
            6 => VectorBackend::Databricks,
            _ => unreachable!()
        };

        Ok(())
    }

    fn select_cache(&mut self) -> Result<()> {
        let options = vec![
            "Dragonfly (recommended, 5x faster, Apache-2.0)",
            "Valkey (official Redis fork, BSD-3)",
            "External Redis (bring your own)",
        ];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("Redis-compatible cache")
            .items(&options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.cache = match selection {
            0 => CacheType::Dragonfly,
            1 => CacheType::Valkey,
            2 => CacheType::External,
            _ => unreachable!()
        };

        if matches!(self.config.cache, CacheType::External) {
            let host: String = Input::with_theme(&self.theme)
                .with_prompt("Redis host")
                .default("localhost".to_string())
                .interact_text()?;

            let port: u16 = Input::with_theme(&self.theme)
                .with_prompt("Redis port")
                .default(6379)
                .interact_text()?;

            let password: String = Input::with_theme(&self.theme)
                .with_prompt("Redis password (leave empty for none)")
                .allow_empty(true)
                .interact_text()?;

            self.config.redis_external = Some(ExternalRedisConfig {
                host,
                port,
                password: if password.is_empty() {
                    None
                } else {
                    Some(password)
                }
            });
        }

        Ok(())
    }

    fn select_postgresql(&mut self) -> Result<()> {
        let options = vec![
            "CloudNativePG (production operator, Apache-2.0)",
            "External PostgreSQL (bring your own)",
        ];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("PostgreSQL deployment")
            .items(&options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.postgresql = match selection {
            0 => PostgresqlType::CloudNativePg,
            1 => PostgresqlType::External,
            _ => unreachable!()
        };

        if matches!(self.config.postgresql, PostgresqlType::External) {
            let host: String = Input::with_theme(&self.theme)
                .with_prompt("PostgreSQL host")
                .default("localhost".to_string())
                .interact_text()?;

            let port: u16 = Input::with_theme(&self.theme)
                .with_prompt("PostgreSQL port")
                .default(5432)
                .interact_text()?;

            let database: String = Input::with_theme(&self.theme)
                .with_prompt("Database name")
                .default("aeterna".to_string())
                .interact_text()?;

            let username: String = Input::with_theme(&self.theme)
                .with_prompt("Username")
                .default("postgres".to_string())
                .interact_text()?;

            let password: String = Input::with_theme(&self.theme)
                .with_prompt("Password")
                .interact_text()?;

            self.config.pg_external = Some(ExternalPostgresConfig {
                host,
                port,
                database,
                username: Some(username),
                password: Some(password)
            });
        }

        Ok(())
    }

    fn configure_opal(&mut self) -> Result<()> {
        self.config.opal_enabled = Confirm::with_theme(&self.theme)
            .with_prompt("Enable OPAL authorization stack? (recommended for multi-tenant)")
            .default(true)
            .interact()?;

        Ok(())
    }

    fn configure_llm(&mut self) -> Result<()> {
        let options = vec![
            "OpenAI (text-embedding-3-small, gpt-4o)",
            "Anthropic (claude-3-haiku)",
            "Ollama (local, no API key)",
            "Skip (configure later)",
        ];

        let selection = Select::with_theme(&self.theme)
            .with_prompt("LLM provider for embeddings")
            .items(&options)
            .default(0)
            .interact_on(&self.term)?;

        self.config.llm_provider = match selection {
            0 => LlmProvider::Openai,
            1 => LlmProvider::Anthropic,
            2 => LlmProvider::Ollama,
            3 => LlmProvider::None,
            _ => unreachable!()
        };

        match self.config.llm_provider {
            LlmProvider::Openai => {
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    println!("{}", "Using API key from OPENAI_API_KEY".dimmed());
                    self.config.openai_api_key = Some(key);
                } else {
                    let key: String = Input::with_theme(&self.theme)
                        .with_prompt("OpenAI API Key")
                        .interact_text()?;
                    self.config.openai_api_key = Some(key);
                }
            }
            LlmProvider::Anthropic => {
                if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                    println!("{}", "Using API key from ANTHROPIC_API_KEY".dimmed());
                    self.config.anthropic_api_key = Some(key);
                } else {
                    let key: String = Input::with_theme(&self.theme)
                        .with_prompt("Anthropic API Key")
                        .interact_text()?;
                    self.config.anthropic_api_key = Some(key);
                }
            }
            LlmProvider::Ollama => {
                let host: String = Input::with_theme(&self.theme)
                    .with_prompt("Ollama host URL")
                    .default("http://localhost:11434".to_string())
                    .interact_text()?;
                self.config.ollama_host = Some(host);
            }
            LlmProvider::None => {}
        }

        Ok(())
    }

    fn configure_opencode(&mut self) -> Result<()> {
        self.config.opencode_enabled = Confirm::with_theme(&self.theme)
            .with_prompt("Enable OpenCode integration?")
            .default(true)
            .interact()?;

        Ok(())
    }

    fn configure_advanced_options(&mut self) -> Result<()> {
        if !matches!(self.config.target, DeploymentTarget::Kubernetes) {
            return Ok(());
        }

        let show_advanced = Confirm::with_theme(&self.theme)
            .with_prompt("Configure advanced options?")
            .default(false)
            .interact()?;

        if !show_advanced {
            return Ok(());
        }

        let options = vec![
            "Ingress (TLS termination)",
            "ServiceMonitor (Prometheus)",
            "NetworkPolicy (isolation)",
            "HPA (autoscaling)",
            "PDB (disruption budget)",
        ];

        let selections = MultiSelect::with_theme(&self.theme)
            .with_prompt("Enable advanced features")
            .items(&options)
            .interact()?;

        for selection in selections {
            match selection {
                0 => {
                    self.config.ingress_enabled = true;
                    let host: String = Input::with_theme(&self.theme)
                        .with_prompt("Ingress hostname")
                        .default("aeterna.local".to_string())
                        .interact_text()?;
                    self.config.ingress_host = Some(host);
                }
                1 => self.config.service_monitor_enabled = true,
                2 => self.config.network_policy_enabled = true,
                3 => self.config.hpa_enabled = true,
                4 => self.config.pdb_enabled = true,
                _ => {}
            }
        }

        Ok(())
    }
}
