## ADDED Requirements

### Requirement: CLI Setup Command

The system SHALL provide an `aeterna setup` command that interactively configures Aeterna deployments through a wizard interface.

The command MUST support three execution modes:
- **Interactive mode** (default): Prompts user for all configuration options
- **Non-interactive mode** (`--non-interactive`): Uses CLI flags and environment variables
- **Reconfigure mode** (`--reconfigure`): Modifies existing configuration

#### Scenario: Interactive wizard launch
- **WHEN** user runs `aeterna setup` without flags
- **THEN** the system SHALL display an interactive wizard
- **AND** guide the user through deployment configuration step by step

#### Scenario: Non-interactive execution
- **WHEN** user runs `aeterna setup --non-interactive --target kubernetes --vector-backend qdrant`
- **THEN** the system SHALL generate configuration without prompts
- **AND** use provided flags for all options
- **AND** fail with clear error if required options are missing

#### Scenario: Reconfigure existing setup
- **WHEN** user runs `aeterna setup --reconfigure`
- **THEN** the system SHALL load existing configuration
- **AND** allow modification of specific options
- **AND** preserve unchanged options

### Requirement: Deployment Mode Selection

The system SHALL support three deployment modes with distinct infrastructure configurations.

| Mode | Description | Local Components | Central Components |
|------|-------------|------------------|-------------------|
| **Local** | Self-contained, all components local | All | None |
| **Hybrid** | Local cache with central server | Working/Session memory, Cedar Agent | Episodic+, Knowledge, Governance |
| **Remote** | Thin client to central server | None | All |

#### Scenario: Local mode selection
- **WHEN** user selects "Local" deployment mode
- **THEN** the wizard SHALL prompt for all infrastructure components
- **AND** generate configuration with all services enabled locally

#### Scenario: Hybrid mode selection
- **WHEN** user selects "Hybrid" deployment mode
- **THEN** the wizard SHALL prompt for central server URL
- **AND** prompt for authentication method
- **AND** prompt for local cache size
- **AND** generate configuration with local cache and central sync

#### Scenario: Remote mode selection
- **WHEN** user selects "Remote" deployment mode
- **THEN** the wizard SHALL prompt for central server URL
- **AND** prompt for authentication method
- **AND** generate thin client configuration

### Requirement: Vector Backend Selection

The system SHALL support selection of vector database backend from available options.

Supported backends:
- **Qdrant** (default, self-hosted) - Feature flag: default
- **pgvector** (PostgreSQL extension) - Feature flag: `pgvector`
- **Pinecone** (managed cloud) - Feature flag: `pinecone`
- **Weaviate** (hybrid search) - Feature flag: `weaviate`
- **MongoDB Atlas** (managed) - Feature flag: `mongodb`
- **Vertex AI** (Google Cloud) - Feature flag: `vertex-ai`
- **Databricks** (Unity Catalog) - Feature flag: `databricks`

#### Scenario: Self-hosted backend selection
- **WHEN** user selects Qdrant, pgvector, or Weaviate
- **THEN** the wizard SHALL offer bundled deployment option
- **OR** external connection configuration

#### Scenario: Managed backend selection
- **WHEN** user selects Pinecone, MongoDB Atlas, Vertex AI, or Databricks
- **THEN** the wizard SHALL prompt for required credentials
- **AND** validate connection before proceeding

#### Scenario: Pinecone configuration
- **WHEN** user selects Pinecone as vector backend
- **THEN** the wizard SHALL prompt for API key
- **AND** prompt for environment
- **AND** prompt for index name
- **AND** validate API key is valid

#### Scenario: Vertex AI configuration
- **WHEN** user selects Vertex AI as vector backend
- **THEN** the wizard SHALL prompt for GCP project ID
- **AND** prompt for region
- **AND** prompt for index endpoint
- **AND** prompt for service account JSON or path

### Requirement: Cache Selection

The system SHALL support selection of Redis-compatible cache from available options.

Supported caches:
- **Dragonfly** (recommended, Apache-2.0) - 5x faster than Redis
- **Valkey** (BSD-3) - Official Redis fork
- **External Redis** - Bring your own

#### Scenario: Dragonfly selection
- **WHEN** user selects Dragonfly
- **THEN** the wizard SHALL configure Dragonfly operator deployment
- **AND** set recommended resource limits

#### Scenario: Valkey selection
- **WHEN** user selects Valkey
- **THEN** the wizard SHALL configure Valkey deployment
- **AND** disable Dragonfly (mutual exclusivity)

#### Scenario: External Redis selection
- **WHEN** user selects "External Redis"
- **THEN** the wizard SHALL prompt for Redis host
- **AND** prompt for Redis port
- **AND** prompt for Redis password (optional)
- **AND** validate connection before proceeding

### Requirement: PostgreSQL Selection

The system SHALL support PostgreSQL deployment or external connection.

Options:
- **CloudNativePG** (Apache-2.0, CNCF) - Production operator with HA
- **External PostgreSQL** - Bring your own

#### Scenario: CloudNativePG selection
- **WHEN** user selects CloudNativePG
- **THEN** the wizard SHALL configure operator deployment
- **AND** prompt for cluster size (1, 3, or 5 instances)
- **AND** prompt for storage size
- **AND** optionally configure backup destination

#### Scenario: External PostgreSQL selection
- **WHEN** user selects "External PostgreSQL"
- **THEN** the wizard SHALL prompt for host, port, database, username, password
- **AND** validate connection before proceeding
- **AND** check pgvector extension availability if pgvector backend selected

### Requirement: OPAL Authorization Configuration

The system SHALL support OPAL authorization stack configuration for multi-tenant deployments.

#### Scenario: OPAL enabled
- **WHEN** user enables OPAL authorization
- **THEN** the wizard SHALL configure OPAL Server deployment
- **AND** configure Cedar Agent deployment
- **AND** configure OPAL Fetcher deployment
- **AND** generate required secrets (master token, client token)

#### Scenario: OPAL disabled (single-tenant)
- **WHEN** user disables OPAL authorization
- **THEN** the wizard SHALL skip OPAL configuration
- **AND** configure single-tenant mode
- **AND** warn about missing multi-tenant governance features

#### Scenario: Hybrid mode with local Cedar Agent
- **WHEN** user selects Hybrid mode with OPAL
- **THEN** the wizard SHALL offer local Cedar Agent option
- **AND** configure OPAL Client to sync from central server
- **AND** enable offline policy evaluation

### Requirement: LLM Provider Configuration

The system SHALL support LLM provider configuration for embeddings and summarization.

Supported providers:
- **OpenAI** - text-embedding-3-small, gpt-4o
- **Anthropic** - claude-3-haiku
- **Ollama** - Local, no API key required
- **Skip** - Configure later

#### Scenario: OpenAI configuration
- **WHEN** user selects OpenAI
- **THEN** the wizard SHALL prompt for API key
- **AND** validate API key format
- **AND** configure embedding model (text-embedding-3-small)
- **AND** configure chat model (gpt-4o)

#### Scenario: Anthropic configuration
- **WHEN** user selects Anthropic
- **THEN** the wizard SHALL prompt for API key
- **AND** configure model (claude-3-haiku-20240307)

#### Scenario: Ollama configuration
- **WHEN** user selects Ollama
- **THEN** the wizard SHALL prompt for Ollama host URL
- **AND** prompt for model name
- **AND** validate Ollama server is reachable

### Requirement: OpenCode Integration Configuration

The system SHALL support OpenCode MCP integration configuration.

#### Scenario: OpenCode enabled
- **WHEN** user enables OpenCode integration
- **THEN** the wizard SHALL generate MCP configuration
- **AND** write to `~/.config/opencode/mcp.json`
- **AND** configure appropriate transport (stdio for local, HTTP for remote)

#### Scenario: Local OpenCode integration
- **WHEN** OpenCode is enabled with Local deployment mode
- **THEN** the wizard SHALL configure stdio transport
- **AND** set command to spawn `aeterna-mcp` process

#### Scenario: Remote OpenCode integration
- **WHEN** OpenCode is enabled with Hybrid or Remote deployment mode
- **THEN** the wizard SHALL configure HTTP transport
- **AND** set URL to central server MCP endpoint
- **AND** configure Bearer token authentication

### Requirement: Advanced Options

The system SHALL support advanced configuration options, collapsed by default in interactive mode.

Options:
- **Ingress** - Enable ingress with TLS
- **ServiceMonitor** - Enable Prometheus metrics scraping
- **NetworkPolicy** - Enable network isolation
- **HPA** - Enable horizontal pod autoscaling
- **PDB** - Enable pod disruption budget

#### Scenario: Advanced options in interactive mode
- **WHEN** user reaches advanced options step
- **THEN** the wizard SHALL display options as multi-select
- **AND** all options SHALL be disabled by default
- **AND** each option SHALL have brief description

#### Scenario: Ingress configuration
- **WHEN** user enables Ingress
- **THEN** the wizard SHALL prompt for hostname
- **AND** prompt for Ingress class (nginx, traefik, contour)
- **AND** prompt for TLS configuration (cert-manager, manual, none)

### Requirement: Configuration File Generation

The system SHALL generate configuration files for all deployment targets.

Output files:
- `values.yaml` - Helm chart values
- `docker-compose.yaml` - Local Docker development
- `.aeterna/config.toml` - Runtime configuration
- `~/.config/opencode/mcp.json` - OpenCode MCP configuration

#### Scenario: File generation success
- **WHEN** wizard completes successfully
- **THEN** the system SHALL generate all applicable configuration files
- **AND** display list of generated files
- **AND** display next steps instructions

#### Scenario: Existing file backup
- **WHEN** generating files that already exist
- **THEN** the system SHALL backup existing files with `.bak` extension
- **AND** inform user of backup location

#### Scenario: Docker Compose generation for Local mode
- **WHEN** deployment target is Local with Docker Compose
- **THEN** the system SHALL generate `docker-compose.yaml`
- **AND** include all selected services
- **AND** configure health checks and dependencies

#### Scenario: Helm values generation for Kubernetes
- **WHEN** deployment target is Kubernetes
- **THEN** the system SHALL generate `values.yaml`
- **AND** include all configuration options
- **AND** follow Helm values schema

### Requirement: Configuration Validation

The system SHALL validate configurations before and after generation.

#### Scenario: Pre-generation validation
- **WHEN** all wizard steps are completed
- **THEN** the system SHALL validate configuration consistency
- **AND** check for conflicting options
- **AND** warn about potential issues

#### Scenario: External service validation
- **WHEN** external services are configured (PostgreSQL, Redis, etc.)
- **THEN** the system SHALL attempt connection validation
- **AND** report success or failure with actionable error messages

#### Scenario: Post-generation validation
- **WHEN** configuration files are generated
- **THEN** the system SHALL validate file syntax (YAML, TOML, JSON)
- **AND** validate against schemas where available

### Requirement: CLI Subcommands

The system SHALL provide additional CLI subcommands for configuration management.

Subcommands:
- `aeterna setup` - Interactive configuration wizard
- `aeterna setup --validate` - Validate existing configuration
- `aeterna setup --show` - Display current configuration
- `aeterna status` - Show deployment status and health
- `aeterna version` - Show version information

#### Scenario: Configuration validation
- **WHEN** user runs `aeterna setup --validate`
- **THEN** the system SHALL load existing configuration
- **AND** validate all settings
- **AND** report any issues found

#### Scenario: Configuration display
- **WHEN** user runs `aeterna setup --show`
- **THEN** the system SHALL display current configuration
- **AND** mask sensitive values (API keys, passwords)
- **AND** show configuration source (file path)

#### Scenario: Deployment status
- **WHEN** user runs `aeterna status`
- **THEN** the system SHALL check connectivity to all configured services
- **AND** display health status for each component
- **AND** show summary of deployment mode and features

### Requirement: Error Handling and User Feedback

The system SHALL provide clear error messages and user feedback throughout the wizard.

#### Scenario: Connection failure
- **WHEN** validation of external service fails
- **THEN** the system SHALL display specific error message
- **AND** offer option to retry, skip validation, or reconfigure

#### Scenario: Invalid input
- **WHEN** user provides invalid input (e.g., malformed URL)
- **THEN** the system SHALL display validation error
- **AND** allow user to correct input
- **AND** NOT proceed until valid input provided

#### Scenario: Wizard cancellation
- **WHEN** user cancels wizard (Ctrl+C)
- **THEN** the system SHALL cleanup any partial state
- **AND** NOT write any configuration files
- **AND** display message about how to resume

### Requirement: Non-Interactive Mode Flags

The system SHALL support all configuration options via CLI flags for non-interactive use.

Required flags for non-interactive mode:
- `--target` - Deployment target (docker-compose, kubernetes, opencode-only)
- `--mode` - Deployment mode (local, hybrid, remote)
- `--vector-backend` - Vector database backend
- `--cache` - Cache selection (dragonfly, valkey, external)
- `--postgresql` - PostgreSQL selection (cloudnative-pg, external)

Optional flags:
- `--central-url` - Central server URL (for hybrid/remote)
- `--central-auth` - Authentication method (api-key, oauth2, service-account)
- `--opal` - Enable/disable OPAL (true/false)
- `--llm` - LLM provider (openai, anthropic, ollama, none)
- `--opencode` - Enable OpenCode integration (true/false)
- `--ingress` - Enable ingress (true/false)
- `--ingress-host` - Ingress hostname
- `--service-monitor` - Enable ServiceMonitor (true/false)
- `--network-policy` - Enable NetworkPolicy (true/false)
- `--hpa` - Enable HPA (true/false)
- `--pdb` - Enable PDB (true/false)
- `--output` - Output directory for generated files

#### Scenario: Complete non-interactive execution
- **WHEN** user provides all required flags
- **THEN** the system SHALL generate configuration without prompts
- **AND** exit with code 0 on success

#### Scenario: Missing required flags
- **WHEN** user runs non-interactive mode with missing required flags
- **THEN** the system SHALL exit with code 1
- **AND** display list of missing required flags

### Requirement: Environment Variable Support

The system SHALL support configuration via environment variables for CI/CD integration.

Environment variable prefix: `AETERNA_SETUP_`

Examples:
- `AETERNA_SETUP_TARGET=kubernetes`
- `AETERNA_SETUP_MODE=hybrid`
- `AETERNA_SETUP_CENTRAL_URL=https://aeterna.company.com`
- `AETERNA_SETUP_OPENAI_API_KEY=sk-...`

#### Scenario: Environment variable precedence
- **WHEN** both CLI flag and environment variable are set
- **THEN** CLI flag SHALL take precedence

#### Scenario: Sensitive values from environment
- **WHEN** sensitive values (API keys) are needed
- **THEN** the system SHALL check environment variables first
- **AND** prompt only if not found in environment
