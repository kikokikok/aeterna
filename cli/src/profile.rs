//! CLI profile and configuration management.
//!
//! # Canonical config file locations
//!
//! - **User-level**:    `~/.config/aeterna/config.toml`  (XDG_CONFIG_HOME / dirs::config_dir)
//! - **Project-level**: `.aeterna/config.toml`           (resolved from CWD upward)
//!
//! # Precedence (highest to lowest)
//!
//! 1. CLI flags / explicit overrides
//! 2. `AETERNA_*` environment variables
//! 3. Project-level config (`.aeterna/config.toml`)
//! 4. User-level config (`~/.config/aeterna/config.toml`)
//! 5. Built-in defaults

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Canonical path helpers
// ---------------------------------------------------------------------------

/// Returns `~/.config/aeterna/` (user-level config directory).
pub fn user_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("aeterna"))
}

/// Returns `~/.config/aeterna/config.toml`.
pub fn user_config_path() -> Option<PathBuf> {
    user_config_dir().map(|d| d.join("config.toml"))
}

/// Returns `~/.config/aeterna/credentials.toml`.
pub fn user_credentials_path() -> Option<PathBuf> {
    user_config_dir().map(|d| d.join("credentials.toml"))
}

/// Walk upward from `start` to find `.aeterna/config.toml`.
pub fn project_config_path(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(".aeterna").join("config.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Profile model
// ---------------------------------------------------------------------------

/// A named profile capturing all settings for one Aeterna target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    /// Human-readable label (e.g. "production", "staging", "local").
    pub name: String,
    /// Full URL of the Aeterna server (e.g. "https://aeterna.acme.com").
    pub server_url: String,
    /// Auth method used for this profile.
    #[serde(default)]
    pub auth_method: AuthMethod,
    /// Tenant ID override (overrides AETERNA_TENANT_ID).
    pub tenant_id: Option<String>,
    /// Optional free-form label (e.g. "prod", "dev").
    pub label: Option<String>,
    /// GitHub App OAuth client_id for device-flow authentication.
    /// Per-profile so different environments can use different GitHub Apps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_client_id: Option<String>,
}

impl Profile {
    pub fn new(name: impl Into<String>, server_url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            server_url: server_url.into(),
            auth_method: AuthMethod::GitHub,
            tenant_id: None,
            label: None,
            github_client_id: None,
        }
    }
}

/// Supported interactive auth methods for CLI use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    #[default]
    GitHub,
    ApiKey,
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::GitHub => write!(f, "github"),
            AuthMethod::ApiKey => write!(f, "api_key"),
        }
    }
}

// ---------------------------------------------------------------------------
// Config file schema
// ---------------------------------------------------------------------------

/// Top-level schema for `config.toml` (user-level or project-level).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AeternaConfig {
    /// Name of the default profile to use when none is specified.
    pub default_profile: Option<String>,
    /// Named profiles.
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

impl AeternaConfig {
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty() && self.default_profile.is_none()
    }
}

// ---------------------------------------------------------------------------
// Config loading with precedence
// ---------------------------------------------------------------------------

/// Resolved configuration after merging all sources.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Effective profile to use.
    pub profile: Profile,
    /// Name of the effective profile.
    pub profile_name: String,
    /// Source that provided the profile.
    pub profile_source: ConfigSource,
    /// Effective server URL (may differ from profile if env-overridden).
    pub server_url: String,
    /// Effective tenant ID.
    pub tenant_id: Option<String>,
}

/// Source that provided a config value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    CliFlag,
    EnvVar,
    ProjectConfig(PathBuf),
    UserConfig(PathBuf),
    Default,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::CliFlag => write!(f, "CLI flag"),
            ConfigSource::EnvVar => write!(f, "environment variable"),
            ConfigSource::ProjectConfig(p) => write!(f, "{}", p.display()),
            ConfigSource::UserConfig(p) => write!(f, "{}", p.display()),
            ConfigSource::Default => write!(f, "built-in default"),
        }
    }
}

/// Load and merge configs, returning the effective resolved config.
///
/// Precedence: CLI flags (passed as `overrides`) > env vars > project config > user config > defaults.
pub fn load_resolved(
    profile_name_override: Option<&str>,
    server_url_override: Option<&str>,
) -> Result<ResolvedConfig> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Load both config files (neither is required to exist)
    let project_cfg = project_config_path(&cwd)
        .map(|p| load_config_file(&p).map(|c| (c, p)))
        .transpose()?;

    let user_cfg = user_config_path()
        .map(|p| {
            if p.exists() {
                load_config_file(&p).map(|c| Some((c, p)))
            } else {
                Ok(None)
            }
        })
        .transpose()?
        .flatten();

    // Determine active profile name (precedence: CLI flag > AETERNA_PROFILE env > project default > user default)
    let env_profile = std::env::var("AETERNA_PROFILE").ok();
    let profile_name: String = profile_name_override
        .map(|s| s.to_string())
        .or(env_profile)
        .or_else(|| {
            project_cfg
                .as_ref()
                .and_then(|(c, _)| c.default_profile.clone())
        })
        .or_else(|| {
            user_cfg
                .as_ref()
                .and_then(|(c, _)| c.default_profile.clone())
        })
        .unwrap_or_else(|| "default".to_string());

    // Locate the profile definition (project first, then user)
    let (profile, profile_source) = find_profile(
        &profile_name,
        project_cfg.as_ref().map(|(c, p)| (c, p.as_path())),
        user_cfg.as_ref().map(|(c, p)| (c, p.as_path())),
    )?;

    // Apply env-var overrides on top of the profile
    let server_url = server_url_override
        .map(|s| s.to_string())
        .or_else(|| std::env::var("AETERNA_SERVER_URL").ok())
        .unwrap_or_else(|| profile.server_url.clone());

    let tenant_id = std::env::var("AETERNA_TENANT_ID")
        .ok()
        .or_else(|| profile.tenant_id.clone());

    Ok(ResolvedConfig {
        profile: Profile {
            server_url: server_url.clone(),
            tenant_id: tenant_id.clone(),
            ..profile
        },
        profile_name,
        profile_source,
        server_url,
        tenant_id,
    })
}

fn find_profile(
    name: &str,
    project: Option<(&AeternaConfig, &Path)>,
    user: Option<(&AeternaConfig, &Path)>,
) -> Result<(Profile, ConfigSource)> {
    // Project config takes precedence over user config
    if let Some((cfg, path)) = project {
        if let Some(p) = cfg.profiles.get(name) {
            return Ok((p.clone(), ConfigSource::ProjectConfig(path.to_path_buf())));
        }
    }
    if let Some((cfg, path)) = user {
        if let Some(p) = cfg.profiles.get(name) {
            return Ok((p.clone(), ConfigSource::UserConfig(path.to_path_buf())));
        }
    }
    // Return a skeleton if the profile is missing (lets callers give useful errors)
    Ok((
        Profile {
            name: name.to_string(),
            server_url: String::new(),
            auth_method: AuthMethod::GitHub,
            tenant_id: None,
            label: None,
            github_client_id: None,
        },
        ConfigSource::Default,
    ))
}

// ---------------------------------------------------------------------------
// Config file I/O
// ---------------------------------------------------------------------------

pub fn load_config_file(path: &Path) -> Result<AeternaConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read config file: {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("Invalid TOML in config file: {}", path.display()))
}

pub fn save_config_file(path: &Path, config: &AeternaConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create config directory: {}", parent.display()))?;
    }
    let raw = toml::to_string_pretty(config).context("Cannot serialize config")?;
    std::fs::write(path, raw)
        .with_context(|| format!("Cannot write config file: {}", path.display()))?;
    Ok(())
}

/// Upsert a profile in the user-level config file.
pub fn save_profile(profile: &Profile) -> Result<PathBuf> {
    let path = user_config_path()
        .context("Cannot determine user config directory. Set XDG_CONFIG_HOME or HOME.")?;
    let mut config = if path.exists() {
        load_config_file(&path)?
    } else {
        AeternaConfig::default()
    };
    config
        .profiles
        .insert(profile.name.clone(), profile.clone());
    if config.default_profile.is_none() {
        config.default_profile = Some(profile.name.clone());
    }
    save_config_file(&path, &config)?;
    Ok(path)
}

/// Set the default profile name in the user-level config file.
pub fn set_default_profile(name: &str) -> Result<PathBuf> {
    let path = user_config_path()
        .context("Cannot determine user config directory. Set XDG_CONFIG_HOME or HOME.")?;
    let mut config = if path.exists() {
        load_config_file(&path)?
    } else {
        AeternaConfig::default()
    };
    config.default_profile = Some(name.to_string());
    save_config_file(&path, &config)?;
    Ok(path)
}

/// Remove a profile from the user-level config file.
///
/// Returns `Ok(true)` if the profile was found and removed, `Ok(false)` if it
/// did not exist. Clears `default_profile` when it matches the removed name.
pub fn delete_profile(name: &str) -> Result<(bool, PathBuf)> {
    let path = user_config_path()
        .context("Cannot determine user config directory. Set XDG_CONFIG_HOME or HOME.")?;
    if !path.exists() {
        return Ok((false, path));
    }
    let mut config = load_config_file(&path)?;
    let removed = config.profiles.remove(name).is_some();
    if removed {
        if config.default_profile.as_deref() == Some(name) {
            config.default_profile = None;
        }
        save_config_file(&path, &config)?;
    }
    Ok((removed, path))
}

/// List all profile names in the user-level config file.
pub fn list_profiles() -> Result<Vec<(String, Profile)>> {
    let path = match user_config_path() {
        Some(p) if p.exists() => p,
        _ => return Ok(Vec::new()),
    };
    let config = load_config_file(&path)?;
    let mut profiles: Vec<(String, Profile)> = config.profiles.into_iter().collect();
    profiles.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(profiles)
}

/// Return the default profile name from the user-level config, if set.
pub fn default_profile_name() -> Result<Option<String>> {
    let path = match user_config_path() {
        Some(p) if p.exists() => p,
        _ => return Ok(None),
    };
    let config = load_config_file(&path)?;
    Ok(config.default_profile)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_config(dir: &Path, content: &str) -> PathBuf {
        let path = dir.join("config.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_profile_new() {
        let p = Profile::new("staging", "https://staging.example.com");
        assert_eq!(p.name, "staging");
        assert_eq!(p.server_url, "https://staging.example.com");
        assert_eq!(p.auth_method, AuthMethod::GitHub);
        assert!(p.tenant_id.is_none());
    }

    #[test]
    fn test_auth_method_display() {
        assert_eq!(AuthMethod::GitHub.to_string(), "github");
        assert_eq!(AuthMethod::ApiKey.to_string(), "api_key");
    }

    #[test]
    fn test_load_config_file_valid() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            dir.path(),
            r#"
default_profile = "prod"
[profiles.prod]
name = "prod"
server_url = "https://aeterna.example.com"
"#,
        );
        let config = load_config_file(&path).unwrap();
        assert_eq!(config.default_profile, Some("prod".to_string()));
        assert!(config.profiles.contains_key("prod"));
        assert_eq!(
            config.profiles["prod"].server_url,
            "https://aeterna.example.com"
        );
    }

    #[test]
    fn test_load_config_file_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let path = write_config(dir.path(), "not = [valid toml");
        assert!(load_config_file(&path).is_err());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = AeternaConfig::default();
        config.default_profile = Some("local".to_string());
        config.profiles.insert(
            "local".to_string(),
            Profile::new("local", "http://localhost:3000"),
        );
        save_config_file(&path, &config).unwrap();

        let loaded = load_config_file(&path).unwrap();
        assert_eq!(loaded.default_profile, Some("local".to_string()));
        assert_eq!(loaded.profiles["local"].server_url, "http://localhost:3000");
    }

    #[test]
    fn test_find_profile_prefers_project() {
        let mut project_cfg = AeternaConfig::default();
        project_cfg.profiles.insert(
            "default".to_string(),
            Profile::new("default", "https://project.example.com"),
        );
        let mut user_cfg = AeternaConfig::default();
        user_cfg.profiles.insert(
            "default".to_string(),
            Profile::new("default", "https://user.example.com"),
        );
        let project_path = PathBuf::from(".aeterna/config.toml");
        let user_path = PathBuf::from("~/.config/aeterna/config.toml");

        let (profile, source) = find_profile(
            "default",
            Some((&project_cfg, &project_path)),
            Some((&user_cfg, &user_path)),
        )
        .unwrap();

        assert_eq!(profile.server_url, "https://project.example.com");
        assert!(matches!(source, ConfigSource::ProjectConfig(_)));
    }

    #[test]
    fn test_find_profile_falls_back_to_user() {
        let project_cfg = AeternaConfig::default();
        let mut user_cfg = AeternaConfig::default();
        user_cfg.profiles.insert(
            "default".to_string(),
            Profile::new("default", "https://user.example.com"),
        );
        let project_path = PathBuf::from(".aeterna/config.toml");
        let user_path = PathBuf::from("~/.config/aeterna/config.toml");

        let (profile, source) = find_profile(
            "default",
            Some((&project_cfg, &project_path)),
            Some((&user_cfg, &user_path)),
        )
        .unwrap();

        assert_eq!(profile.server_url, "https://user.example.com");
        assert!(matches!(source, ConfigSource::UserConfig(_)));
    }

    #[test]
    fn test_find_profile_missing_returns_skeleton() {
        let (profile, source) = find_profile("missing", None, None).unwrap();
        assert_eq!(profile.name, "missing");
        assert_eq!(profile.server_url, "");
        assert_eq!(source, ConfigSource::Default);
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(ConfigSource::CliFlag.to_string(), "CLI flag");
        assert_eq!(ConfigSource::EnvVar.to_string(), "environment variable");
        assert_eq!(ConfigSource::Default.to_string(), "built-in default");
    }

    #[test]
    fn test_project_config_path_not_found() {
        let dir = TempDir::new().unwrap();
        assert!(project_config_path(dir.path()).is_none());
    }

    #[test]
    fn test_project_config_path_found() {
        let dir = TempDir::new().unwrap();
        let aeterna_dir = dir.path().join(".aeterna");
        std::fs::create_dir_all(&aeterna_dir).unwrap();
        std::fs::write(aeterna_dir.join("config.toml"), "").unwrap();
        let found = project_config_path(dir.path());
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("config.toml"));
    }

    #[test]
    fn test_aeterna_config_is_empty() {
        let config = AeternaConfig::default();
        assert!(config.is_empty());
    }

    #[test]
    fn test_aeterna_config_not_empty_with_profile() {
        let mut config = AeternaConfig::default();
        config
            .profiles
            .insert("x".to_string(), Profile::new("x", "http://x"));
        assert!(!config.is_empty());
    }

    #[test]
    fn test_profile_serialization_roundtrip() {
        let p = Profile {
            name: "prod".to_string(),
            server_url: "https://prod.example.com".to_string(),
            auth_method: AuthMethod::ApiKey,
            tenant_id: Some("tenant-123".to_string()),
            label: Some("production".to_string()),
            github_client_id: None,
        };
        let s = toml::to_string_pretty(&p).unwrap();
        let p2: Profile = toml::from_str(&s).unwrap();
        assert_eq!(p, p2);
    }
}
