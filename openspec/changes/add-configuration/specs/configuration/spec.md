## ADDED Requirements

### Requirement: Configuration Structure
The system SHALL define a comprehensive configuration structure for all system components.

#### Scenario: Main config struct
- **WHEN** loading configuration
- **THEN** system SHALL provide Config struct with all fields
- **AND** config SHALL include: provider, knowledge, sync, tool, observability
- **AND** config SHALL be serializable to JSON and TOML

#### Scenario: Provider configuration
- **WHEN** configuring storage providers
- **THEN** system SHALL provide ProviderConfig for PostgreSQL, Qdrant, Redis
- **AND** each provider SHALL have connection string, pool size, timeout settings

#### Scenario: Sync configuration
- **WHEN** configuring sync behavior
- **THEN** system SHALL provide SyncConfig struct
- **AND** config SHALL include: autoSync, scheduleInterval, sessionThreshold, stalenessThreshold

### Requirement: Environment Variable Loading
The system SHALL support loading configuration from environment variables using 12-factor app principles.

#### Scenario: Load from environment variables
- **WHEN** system starts with environment variables set
- **THEN** system SHALL load all config from env vars
- **AND** system SHALL use MK_* prefix for memory settings
- **AND** system SHALL use KK_* prefix for knowledge settings
- **AND** system SHALL use SY_* prefix for sync settings

#### Scenario: Handle missing environment variables
- **WHEN** required environment variable is not set
- **THEN** system SHALL use default value if available
- **AND** system SHALL fail with clear error if no default

#### Scenario: Parse complex types
- **WHEN** environment variable contains array or object
- **THEN** system SHALL parse value into correct type
- **AND** system SHALL handle JSON strings, comma-separated lists
- **AND** system SHALL log parsing errors

### Requirement: Config File Loading
The system SHALL support loading configuration from TOML and YAML files.

#### Scenario: Load TOML config file
- **WHEN** system starts with config.toml file
- **THEN** system SHALL parse TOML file
- **AND** system SHALL handle parse errors with clear messages
- **AND** system SHALL merge with environment variables (env vars take precedence)

#### Scenario: Load YAML config file
- **WHEN** system starts with config.yaml file
- **THEN** system SHALL parse YAML file
- **AND** system SHALL handle parse errors with clear messages
- **AND** system SHALL merge with environment variables (env vars take precedence)

#### Scenario: Config file not found
- **WHEN** specified config file does not exist
- **THEN** system SHALL fail with CONFIG_FILE_NOT_FOUND error
- **AND** error SHALL include file path

### Requirement: Configuration Precedence
The system SHALL apply configuration in precedence order: CLI args > env vars > config file > defaults.

#### Scenario: Precedence for simple values
- **WHEN** same value is set in multiple sources
- **THEN** system SHALL use CLI args if set
- **AND** system SHALL use env vars if CLI not set
- **AND** system SHALL use config file if env not set
- **AND** system SHALL use defaults if file not set

#### Scenario: Deep merge for nested configs
- **WHEN** nested struct values come from multiple sources
- **THEN** system SHALL deep merge the structures
- **AND** system SHALL not override entire nested object
- **AND** system SHALL merge at field level

#### Scenario: Log configuration sources
- **WHEN** applying configuration from multiple sources
- **THEN** system SHALL log which source provided each final value
- **AND** system SHALL help debugging configuration issues

### Requirement: Configuration Validation
The system SHALL validate all configuration at startup and return clear error messages.

#### Scenario: Validate required fields
- **WHEN** required configuration field is missing
- **THEN** system SHALL return CONFIG_MISSING_REQUIRED_FIELD error
- **AND** error SHALL specify which field is missing
- **AND** system SHALL fail to start

#### Scenario: Validate field types
- **WHEN** configuration field has wrong type
- **THEN** system SHALL return CONFIG_INVALID_TYPE error
- **AND** error SHALL specify field and expected type
- **AND** system SHALL fail to start

#### Scenario: Validate value ranges
- **WHEN** numeric value is outside valid range
- **THEN** system SHALL return CONFIG_INVALID_RANGE error
- **AND** error SHALL specify field, value, min, max
- **AND** system SHALL fail to start

#### Scenario: Validate provider-specific settings
- **WHEN** provider configuration is invalid
- **THEN** system SHALL return CONFIG_INVALID_PROVIDER error
- **AND** error SHALL include provider name and validation details
- **AND** system SHALL fail to start

### Requirement: Configuration Schema
The system SHALL generate JSON Schema from configuration structures for documentation.

#### Scenario: Generate schema from structs
- **WHEN** generating configuration schema
- **THEN** system SHALL use schemars crate to derive schema
- **AND** system SHALL include all fields with types and descriptions
- **AND** system SHALL include default values
- **AND** system SHALL include examples

#### Scenario: Validate schema
- **WHEN** schema is generated
- **THEN** system SHALL validate against JSON Schema specification
- **AND** system SHALL be compatible with JSON Schema draft 2020-12

### Requirement: Hot Reload
The system SHALL support reloading configuration without restart when file changes.

#### Scenario: Watch config file for changes
- **WHEN** config file is modified
- **THEN** system SHALL detect change using file watcher
- **AND** system SHALL reload configuration
- **AND** system SHALL emit config_reload event

#### Scenario: Apply new configuration
- **WHEN** configuration is reloaded
- **THEN** system SHALL validate new configuration
- **AND** system SHALL gracefully apply new config
- **AND** system SHALL notify affected components
- **AND** system SHALL log reload success or failure

#### Scenario: Handle reload errors
- **WHEN** reloaded configuration is invalid
- **THEN** system SHALL keep previous valid configuration
- **AND** system SHALL log reload failure with details
- **AND** system SHALL notify admin of error

### Requirement: Configuration Error Handling
The system SHALL provide specific error codes for all configuration failures.

#### Scenario: File not found error
- **WHEN** config file does not exist
- **THEN** system SHALL return CONFIG_FILE_NOT_FOUND error
- **AND** error SHALL include file path

#### Scenario: Parse error
- **WHEN** config file cannot be parsed
- **THEN** system SHALL return CONFIG_PARSE_ERROR error
- **AND** error SHALL include parse error details

#### Scenario: Validation error
- **WHEN** configuration validation fails
- **THEN** system SHALL return CONFIG_VALIDATION_ERROR error
- **AND** error SHALL list all validation issues
- **AND** error SHALL not be retryable

### Requirement: Configuration Documentation
The system SHALL provide comprehensive documentation for all configuration options.

#### Scenario: Document all fields
- **WHEN** reading configuration documentation
- **THEN** documentation SHALL describe all configuration fields
- **AND** documentation SHALL include type, default, description
- **AND** documentation SHALL provide examples

#### Scenario: Provide example config files
- **WHEN** developer needs configuration examples
- **THEN** system SHALL provide example-config.toml
- **AND** system SHALL provide example-config.yaml
- **AND** examples SHALL include comments explaining each option

#### Scenario: Document environment variables
- **WHEN** reading environment variable documentation
- **THEN** documentation SHALL list all environment variable names
- **AND** documentation SHALL show mapping to config fields
- **AND** documentation SHALL provide example commands
