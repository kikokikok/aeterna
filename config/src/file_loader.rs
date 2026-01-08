//! # Configuration File Loading
//!
//! Loads configuration from TOML or YAML files.
//!
//! Supports automatic format detection based on file extension.

use crate::config::Config;
use std::path::Path;

/// Configuration file loading error.
#[derive(Debug, thiserror::Error)]
pub enum ConfigFileError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse TOML: {0}")]
    TomlParse(String),

    #[error("Failed to parse YAML: {0}")]
    YamlParse(String),

    #[error("Config file has no extension")]
    NoExtension,

    #[error("Unsupported config file format: {0}")]
    UnsupportedFormat(String),
}

/// Load configuration from TOML file.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Loads complete configuration from a TOML format file.
///
/// ## Usage
/// ```rust,no_run
/// use config::load_from_toml;
/// use std::path::Path;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = load_from_toml(Path::new("config.toml"))?;
///     println!("PostgreSQL host: {}", config.providers.postgres.host);
///     Ok(())
/// }
/// ```
///
/// ## Error Handling
/// Returns `ConfigFileError` for:
/// - File not found
/// - Invalid TOML syntax
/// - Missing required fields
pub fn load_from_toml(path: &Path) -> Result<Config, ConfigFileError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|_e| ConfigFileError::FileNotFound(path.display().to_string()))?;

    let config: Config =
        toml::from_str(&contents).map_err(|e| ConfigFileError::TomlParse(e.to_string()))?;

    Ok(config)
}

/// Load configuration from YAML file.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Loads complete configuration from a YAML format file.
///
/// ## Usage
/// ```rust,no_run
/// use config::load_from_yaml;
/// use std::path::Path;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = load_from_yaml(Path::new("config.yaml"))?;
///     println!("PostgreSQL host: {}", config.providers.postgres.host);
///     Ok(())
/// }
/// ```
///
/// ## Error Handling
/// Returns `ConfigFileError` for:
/// - File not found
/// - Invalid YAML syntax
/// - Missing required fields
pub fn load_from_yaml(path: &Path) -> Result<Config, ConfigFileError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|_e| ConfigFileError::FileNotFound(path.display().to_string()))?;

    let config: Config =
        serde_yaml::from_str(&contents).map_err(|e| ConfigFileError::YamlParse(e.to_string()))?;

    Ok(config)
}

/// Load configuration from file with auto-detection.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Loads configuration from file, automatically detecting format from extension.
///
/// ## Supported Formats
/// - `.toml`: TOML format
/// - `.yaml`: YAML format
/// - `.yml`: YAML format
///
/// ## Usage
/// ```rust,no_run
/// use config::load_from_file;
/// use std::path::Path;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = load_from_file(Path::new("config.yaml"))?;
///     Ok(())
/// }
/// ```
///
/// ## Error Handling
/// Returns `ConfigFileError` for:
/// - File not found
/// - Invalid file extension
/// - Parse errors for detected format
pub fn load_from_file(path: &Path) -> Result<Config, ConfigFileError> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(ConfigFileError::NoExtension)?;

    match extension.to_lowercase().as_str() {
        "toml" => load_from_toml(path),
        "yaml" | "yml" => load_from_yaml(path),
        other => Err(ConfigFileError::UnsupportedFormat(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_from_toml() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("toml");

        let toml_content = r#"
[providers.postgres]
host = "testhost"
port = 5433
database = "testdb"
username = "testuser"
password = "testpass"

[providers.qdrant]
host = "qdranthost"
port = 7333
collection = "test_collection"

[providers.redis]
host = "redishost"
port = 6380

[sync]
enabled = false
sync_interval_seconds = 120

[tools]
enabled = false
port = 9090

[observability]
logging_level = "debug"
"#;
        fs::write(&path, toml_content).unwrap();

        let config = load_from_toml(&path).unwrap();
        assert_eq!(config.providers.postgres.host, "testhost");
        assert_eq!(config.providers.postgres.port, 5433);
        assert_eq!(config.providers.postgres.database, "testdb");
        assert_eq!(config.providers.qdrant.host, "qdranthost");
        assert_eq!(config.providers.qdrant.port, 7333);
        assert_eq!(config.providers.redis.host, "redishost");
        assert_eq!(config.sync.enabled, false);
        assert_eq!(config.sync.sync_interval_seconds, 120);
        assert_eq!(config.tools.port, 9090);
        assert_eq!(config.observability.logging_level, "debug");
    }

    #[test]
    fn test_load_from_yaml() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("yaml");

        let yaml_content = r#"
providers:
  postgres:
    host: testhost
    port: 5433
    database: testdb
    username: testuser
    password: testpass
  qdrant:
    host: qdranthost
    port: 7333
    collection: test_collection
  redis:
    host: redishost
    port: 6380

sync:
  enabled: false
  sync_interval_seconds: 120

tools:
  enabled: false
  port: 9090

observability:
  logging_level: debug
"#;
        fs::write(&path, yaml_content).unwrap();

        let config = load_from_yaml(&path).unwrap();
        assert_eq!(config.providers.postgres.host, "testhost");
        assert_eq!(config.providers.postgres.port, 5433);
        assert_eq!(config.providers.qdrant.host, "qdranthost");
        assert_eq!(config.sync.enabled, false);
        assert_eq!(config.tools.port, 9090);
        assert_eq!(config.observability.logging_level, "debug");
    }

    #[test]
    fn test_load_from_file_unsupported() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("json");
        fs::write(&path, "{}").unwrap();

        let result = load_from_file(&path);
        assert!(matches!(result, Err(ConfigFileError::UnsupportedFormat(_))));
    }

    #[test]
    fn test_load_from_file_no_extension() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("");
        fs::write(&path, "").unwrap();

        let result = load_from_file(&path);
        assert!(matches!(result, Err(ConfigFileError::NoExtension)));
    }

    #[test]
    fn test_load_from_file_auto_detect_toml() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("toml");
        let toml_content = r#"
[providers.postgres]
host = "autohost"
"#;
        fs::write(&path, toml_content).unwrap();

        let config = load_from_file(&path).unwrap();
        assert_eq!(config.providers.postgres.host, "autohost");
    }

    #[test]
    fn test_load_from_file_auto_detect_yaml() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("yaml");
        let yaml_content = r#"
providers:
  postgres:
    host: autohost
"#;
        fs::write(&path, yaml_content).unwrap();

        let config = load_from_file(&path).unwrap();
        assert_eq!(config.providers.postgres.host, "autohost");
    }

    #[test]
    fn test_load_from_toml_invalid() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("toml");
        let invalid_toml = r#"
[invalid
"#;
        fs::write(&path, invalid_toml).unwrap();

        let result = load_from_toml(&path);
        assert!(matches!(result, Err(ConfigFileError::TomlParse(_))));
    }

    #[test]
    fn test_load_from_yaml_invalid() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("yaml");
        let invalid_yaml = r#"
invalid: [unmatched
"#;
        fs::write(&path, invalid_yaml).unwrap();

        let result = load_from_yaml(&path);
        assert!(matches!(result, Err(ConfigFileError::YamlParse(_))));
    }

    #[test]
    fn test_load_from_toml_not_found() {
        let path = Path::new("/nonexistent/path/config.toml");
        let result = load_from_toml(path);
        assert!(matches!(result, Err(ConfigFileError::FileNotFound(_))));
    }
}
