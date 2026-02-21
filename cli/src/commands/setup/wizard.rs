use anyhow::Result;
use colored::Colorize;
use console::Term;
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};

use super::types::*;

pub struct SetupWizard {
    term: Term,
    theme: ColorfulTheme,
    reconfigure: bool,
    config: SetupConfig,
}

impl SetupWizard {
    pub fn new(reconfigure: bool) -> Self {
        Self {
            term: Term::stderr(),
            theme: ColorfulTheme::default(),
            reconfigure,
            config: SetupConfig::default(),
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

        if matches!(self.config.mode, DeploymentMode::Hybrid) {
            self.configure_hybrid_mode()?;
        }

        if matches!(
            self.config.mode,
            DeploymentMode::Local | DeploymentMode::Hybrid
        ) {
            self.select_vector_backend()?;
            self.configure_vector_backend_details()?;
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
            _ => unreachable!(),
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
            _ => unreachable!(),
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

        {
            let url_clone = url.clone();
            match tokio::runtime::Runtime::new() {
                Ok(rt) => match rt.block_on(validate_server_connectivity(&url_clone, 5)) {
                    Ok(()) => {
                        println!(
                            "{}",
                            format!("Successfully connected to {url_clone}").dimmed()
                        );
                    }
                    Err(err) => {
                        eprintln!(
                                "{}",
                                format!(
                                    "Warning: Could not connect to {url_clone}: {err}. Proceeding anyway."
                                )
                                .yellow()
                            );
                    }
                },
                Err(err) => {
                    eprintln!(
                        "{}",
                        format!(
                            "Warning: Could not validate connectivity: {err}. Proceeding anyway."
                        )
                        .yellow()
                    );
                }
            }
        }

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
            _ => unreachable!(),
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

    fn configure_hybrid_mode(&mut self) -> Result<()> {
        let mut hybrid = HybridConfig::default();

        let cache_size: u32 = Input::with_theme(&self.theme)
            .with_prompt("Local cache size (MB)")
            .default(512)
            .validate_with(|input: &u32| -> Result<(), &str> {
                if *input >= 64 {
                    Ok(())
                } else {
                    Err("Minimum cache size is 64 MB")
                }
            })
            .interact_text()?;
        hybrid.local_cache_size_mb = cache_size;

        hybrid.offline_cedar = Confirm::with_theme(&self.theme)
            .with_prompt("Enable offline Cedar Agent? (local policy evaluation when disconnected)")
            .default(true)
            .interact()?;

        let sync_interval: u64 = Input::with_theme(&self.theme)
            .with_prompt("Sync interval (seconds)")
            .default(300)
            .validate_with(|input: &u64| -> Result<(), &str> {
                if *input >= 10 {
                    Ok(())
                } else {
                    Err("Minimum sync interval is 10 seconds")
                }
            })
            .interact_text()?;
        hybrid.sync_interval_secs = sync_interval;

        self.config.hybrid = Some(hybrid);

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
            _ => unreachable!(),
        };

        Ok(())
    }

    fn configure_vector_backend_details(&mut self) -> Result<()> {
        match self.config.vector_backend {
            VectorBackend::Pinecone => self.configure_pinecone(),
            VectorBackend::Weaviate => self.configure_weaviate(),
            VectorBackend::Mongodb => self.configure_mongodb(),
            VectorBackend::VertexAi => self.configure_vertex_ai(),
            VectorBackend::Databricks => self.configure_databricks(),
            _ => Ok(()),
        }
    }

    fn configure_pinecone(&mut self) -> Result<()> {
        let api_key: String = if let Ok(key) = std::env::var("PINECONE_API_KEY") {
            println!("{}", "Using API key from PINECONE_API_KEY".dimmed());
            key
        } else {
            Input::with_theme(&self.theme)
                .with_prompt("Pinecone API key")
                .interact_text()?
        };

        let environment: String = Input::with_theme(&self.theme)
            .with_prompt("Pinecone environment (e.g. us-east1-gcp)")
            .interact_text()?;

        let index_name: String = Input::with_theme(&self.theme)
            .with_prompt("Pinecone index name")
            .default("aeterna".to_string())
            .interact_text()?;

        self.config.pinecone = Some(PineconeConfig {
            api_key,
            environment,
            index_name,
        });

        Ok(())
    }

    fn configure_weaviate(&mut self) -> Result<()> {
        let host: String = Input::with_theme(&self.theme)
            .with_prompt("Weaviate host URL")
            .default("http://localhost:8080".to_string())
            .interact_text()?;

        let api_key: String = Input::with_theme(&self.theme)
            .with_prompt("Weaviate API key (leave empty for none)")
            .allow_empty(true)
            .interact_text()?;

        self.config.weaviate = Some(WeaviateConfig {
            host,
            api_key: if api_key.is_empty() {
                None
            } else {
                Some(api_key)
            },
        });

        Ok(())
    }

    fn configure_mongodb(&mut self) -> Result<()> {
        let connection_uri: String = Input::with_theme(&self.theme)
            .with_prompt("MongoDB Atlas connection URI")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.starts_with("mongodb://") || input.starts_with("mongodb+srv://") {
                    Ok(())
                } else {
                    Err("URI must start with mongodb:// or mongodb+srv://")
                }
            })
            .interact_text()?;

        self.config.mongodb = Some(MongodbConfig { connection_uri });

        Ok(())
    }

    fn configure_vertex_ai(&mut self) -> Result<()> {
        let project_id: String = Input::with_theme(&self.theme)
            .with_prompt("Google Cloud project ID")
            .interact_text()?;

        let region: String = Input::with_theme(&self.theme)
            .with_prompt("Region (e.g. us-central1)")
            .default("us-central1".to_string())
            .interact_text()?;

        let endpoint_url: String = Input::with_theme(&self.theme)
            .with_prompt("Vertex AI endpoint URL")
            .interact_text()?;

        let sa_json: String = Input::with_theme(&self.theme)
            .with_prompt("Service account JSON path (leave empty for ADC)")
            .allow_empty(true)
            .interact_text()?;

        self.config.vertex_ai = Some(VertexAiConfig {
            project_id,
            region,
            endpoint_url,
            service_account_json: if sa_json.is_empty() {
                None
            } else {
                Some(sa_json)
            },
        });

        Ok(())
    }

    fn configure_databricks(&mut self) -> Result<()> {
        let workspace_url: String = Input::with_theme(&self.theme)
            .with_prompt("Databricks workspace URL")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.starts_with("https://") {
                    Ok(())
                } else {
                    Err("Workspace URL must start with https://")
                }
            })
            .interact_text()?;

        let token: String = if let Ok(t) = std::env::var("DATABRICKS_TOKEN") {
            println!("{}", "Using token from DATABRICKS_TOKEN".dimmed());
            t
        } else {
            Input::with_theme(&self.theme)
                .with_prompt("Databricks access token")
                .interact_text()?
        };

        let catalog: String = Input::with_theme(&self.theme)
            .with_prompt("Unity Catalog name")
            .default("main".to_string())
            .interact_text()?;

        self.config.databricks = Some(DatabricksConfig {
            workspace_url,
            token,
            catalog,
        });

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
            _ => unreachable!(),
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
                },
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
            _ => unreachable!(),
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
                password: Some(password),
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
            _ => unreachable!(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_new_creates_default_config() {
        let wizard = SetupWizard::new(false);
        assert!(!wizard.reconfigure);
        assert_eq!(wizard.config.target, DeploymentTarget::DockerCompose);
        assert_eq!(wizard.config.mode, DeploymentMode::Local);
        assert_eq!(wizard.config.vector_backend, VectorBackend::Qdrant);
        assert_eq!(wizard.config.cache, CacheType::Dragonfly);
        assert!(wizard.config.opal_enabled);
        assert!(!wizard.config.opencode_enabled);
        assert!(wizard.config.central_url.is_none());
        assert!(wizard.config.hybrid.is_none());
        assert!(wizard.config.pinecone.is_none());
        assert!(wizard.config.weaviate.is_none());
        assert!(wizard.config.mongodb.is_none());
        assert!(wizard.config.vertex_ai.is_none());
        assert!(wizard.config.databricks.is_none());
    }

    #[test]
    fn test_wizard_new_reconfigure_flag() {
        let wizard = SetupWizard::new(true);
        assert!(wizard.reconfigure);
    }

    #[test]
    fn test_wizard_config_is_clonable() {
        let wizard = SetupWizard::new(false);
        let config = wizard.config.clone();
        assert_eq!(config.target, DeploymentTarget::DockerCompose);
    }

    #[test]
    fn test_wizard_default_config_matches_setup_config_default() {
        let wizard = SetupWizard::new(false);
        let default_config = SetupConfig::default();
        assert_eq!(wizard.config.target, default_config.target);
        assert_eq!(wizard.config.mode, default_config.mode);
        assert_eq!(wizard.config.vector_backend, default_config.vector_backend);
        assert_eq!(wizard.config.cache, default_config.cache);
        assert_eq!(wizard.config.postgresql, default_config.postgresql);
        assert_eq!(wizard.config.opal_enabled, default_config.opal_enabled);
        assert_eq!(wizard.config.llm_provider, default_config.llm_provider);
        assert_eq!(
            wizard.config.opencode_enabled,
            default_config.opencode_enabled
        );
        assert_eq!(
            wizard.config.ingress_enabled,
            default_config.ingress_enabled
        );
        assert_eq!(
            wizard.config.service_monitor_enabled,
            default_config.service_monitor_enabled
        );
        assert_eq!(
            wizard.config.network_policy_enabled,
            default_config.network_policy_enabled
        );
        assert_eq!(wizard.config.hpa_enabled, default_config.hpa_enabled);
        assert_eq!(wizard.config.pdb_enabled, default_config.pdb_enabled);
    }

    #[test]
    fn test_wizard_config_can_be_mutated_for_hybrid() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.mode = DeploymentMode::Hybrid;
        wizard.config.central_url = Some("https://central.example.com".to_string());
        wizard.config.hybrid = Some(HybridConfig {
            local_cache_size_mb: 1024,
            offline_cedar: true,
            sync_interval_secs: 120,
        });
        assert_eq!(wizard.config.mode, DeploymentMode::Hybrid);
        assert!(wizard.config.central_url.is_some());
        let hybrid = wizard.config.hybrid.as_ref().expect("hybrid should be set");
        assert_eq!(hybrid.local_cache_size_mb, 1024);
        assert!(hybrid.offline_cedar);
        assert_eq!(hybrid.sync_interval_secs, 120);
    }

    #[test]
    fn test_wizard_config_can_be_mutated_for_pinecone() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.vector_backend = VectorBackend::Pinecone;
        wizard.config.pinecone = Some(PineconeConfig {
            api_key: "pk-test".to_string(),
            environment: "us-east1-gcp".to_string(),
            index_name: "test-idx".to_string(),
        });
        assert_eq!(wizard.config.vector_backend, VectorBackend::Pinecone);
        let pc = wizard.config.pinecone.as_ref().expect("pinecone config");
        assert_eq!(pc.api_key, "pk-test");
    }

    #[test]
    fn test_wizard_config_can_be_mutated_for_weaviate() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.vector_backend = VectorBackend::Weaviate;
        wizard.config.weaviate = Some(WeaviateConfig {
            host: "http://weaviate:8080".to_string(),
            api_key: Some("wk-test".to_string()),
        });
        assert_eq!(wizard.config.vector_backend, VectorBackend::Weaviate);
        let wc = wizard.config.weaviate.as_ref().expect("weaviate config");
        assert_eq!(wc.host, "http://weaviate:8080");
    }

    #[test]
    fn test_wizard_config_can_be_mutated_for_mongodb() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.vector_backend = VectorBackend::Mongodb;
        wizard.config.mongodb = Some(MongodbConfig {
            connection_uri: "mongodb+srv://user:pass@cluster.mongodb.net/db".to_string(),
        });
        assert_eq!(wizard.config.vector_backend, VectorBackend::Mongodb);
    }

    #[test]
    fn test_wizard_config_can_be_mutated_for_vertex_ai() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.vector_backend = VectorBackend::VertexAi;
        wizard.config.vertex_ai = Some(VertexAiConfig {
            project_id: "proj".to_string(),
            region: "us-central1".to_string(),
            endpoint_url: "https://endpoint".to_string(),
            service_account_json: None,
        });
        assert_eq!(wizard.config.vector_backend, VectorBackend::VertexAi);
    }

    #[test]
    fn test_wizard_config_can_be_mutated_for_databricks() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.vector_backend = VectorBackend::Databricks;
        wizard.config.databricks = Some(DatabricksConfig {
            workspace_url: "https://adb.databricks.net".to_string(),
            token: "dapi-tok".to_string(),
            catalog: "main".to_string(),
        });
        assert_eq!(wizard.config.vector_backend, VectorBackend::Databricks);
    }

    #[test]
    fn test_wizard_config_advanced_k8s_options() {
        let mut wizard = SetupWizard::new(false);
        wizard.config.target = DeploymentTarget::Kubernetes;
        wizard.config.ingress_enabled = true;
        wizard.config.ingress_host = Some("aeterna.example.com".to_string());
        wizard.config.service_monitor_enabled = true;
        wizard.config.network_policy_enabled = true;
        wizard.config.hpa_enabled = true;
        wizard.config.pdb_enabled = true;
        assert!(wizard.config.ingress_enabled);
        assert!(wizard.config.service_monitor_enabled);
        assert!(wizard.config.network_policy_enabled);
        assert!(wizard.config.hpa_enabled);
        assert!(wizard.config.pdb_enabled);
    }
}
