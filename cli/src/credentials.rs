//! Credential persistence for the Aeterna CLI.
//!
//! Uses a file-based store at `~/.config/aeterna/credentials.toml` with
//! mode 0600. This is the documented fallback path per the design decision
//! (design.md §"Use secure local credential storage with explicit fallback").
//!
//! Future work (tracked separately): OS keychain integration via `keyring`
//! crate once the dependency is added to the workspace.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::profile::user_credentials_path;

// ---------------------------------------------------------------------------
// Stored credential model
// ---------------------------------------------------------------------------

/// Persisted credentials for one named profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredCredential {
    /// Profile these credentials belong to.
    pub profile_name: String,
    /// Short-lived access token (JWT).
    pub access_token: String,
    /// Long-lived refresh token used to obtain new access tokens.
    pub refresh_token: String,
    /// Unix timestamp (seconds) when the access token expires.
    pub expires_at: i64,
    /// GitHub login resolved at login time.
    pub github_login: Option<String>,
    /// Email resolved at login time.
    pub email: Option<String>,
}

impl StoredCredential {
    /// Returns true if the access token has expired (with a 60-second buffer).
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at <= now + 60
    }
}

// ---------------------------------------------------------------------------
// Credentials file schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CredentialsFile {
    #[serde(default)]
    credentials: HashMap<String, StoredCredential>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load credentials for a named profile. Returns `None` if not found.
pub fn load(profile_name: &str) -> Result<Option<StoredCredential>> {
    let path = match credentials_path() {
        Some(p) => p,
        None => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }
    let file = read_credentials_file(&path)?;
    Ok(file.credentials.get(profile_name).cloned())
}

/// Persist credentials for a named profile.
pub fn save(cred: &StoredCredential) -> Result<PathBuf> {
    let path = credentials_path()
        .context("Cannot determine credentials file location. Set XDG_CONFIG_HOME or HOME.")?;
    let mut file = if path.exists() {
        read_credentials_file(&path)?
    } else {
        CredentialsFile::default()
    };
    file.credentials
        .insert(cred.profile_name.clone(), cred.clone());
    write_credentials_file(&path, &file)?;
    Ok(path)
}

/// Remove stored credentials for a named profile.
pub fn delete(profile_name: &str) -> Result<bool> {
    let path = match credentials_path() {
        Some(p) => p,
        None => return Ok(false),
    };
    if !path.exists() {
        return Ok(false);
    }
    let mut file = read_credentials_file(&path)?;
    let removed = file.credentials.remove(profile_name).is_some();
    if removed {
        write_credentials_file(&path, &file)?;
    }
    Ok(removed)
}

/// List all profile names that have stored credentials.
pub fn list_profiles() -> Result<Vec<String>> {
    let path = match credentials_path() {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = read_credentials_file(&path)?;
    let mut names: Vec<String> = file.credentials.keys().cloned().collect();
    names.sort();
    Ok(names)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn credentials_path() -> Option<PathBuf> {
    // Allow tests to override the credentials path via AETERNA_TEST_CONFIG_DIR
    if let Ok(test_dir) = std::env::var("AETERNA_TEST_CONFIG_DIR") {
        return Some(PathBuf::from(test_dir).join("credentials.toml"));
    }
    user_credentials_path()
}

fn read_credentials_file(path: &PathBuf) -> Result<CredentialsFile> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read credentials file: {}", path.display()))?;
    toml::from_str(&raw)
        .with_context(|| format!("Invalid TOML in credentials file: {}", path.display()))
}

fn write_credentials_file(path: &PathBuf, file: &CredentialsFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Cannot create credentials directory: {}", parent.display())
        })?;
    }
    let raw = toml::to_string_pretty(file).context("Cannot serialize credentials")?;
    std::fs::write(path, &raw)
        .with_context(|| format!("Cannot write credentials file: {}", path.display()))?;

    // Restrict permissions to owner-read/write on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
            .with_context(|| format!("Cannot set permissions on: {}", path.display()))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir;

    /// RAII guard: sets AETERNA_TEST_CONFIG_DIR on construction, removes it on drop.
    /// Uses a test-specific env var that works on macOS (dirs::config_dir ignores XDG on macOS).
    struct EnvGuard;
    impl EnvGuard {
        fn set(dir: &TempDir) -> Self {
            unsafe {
                env::set_var("AETERNA_TEST_CONFIG_DIR", dir.path());
            }
            EnvGuard
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                env::remove_var("AETERNA_TEST_CONFIG_DIR");
            }
        }
    }

    fn setup_temp_home(dir: &TempDir) -> EnvGuard {
        EnvGuard::set(dir)
    }

    fn make_cred(profile: &str, expired: bool) -> StoredCredential {
        let expires_at = if expired {
            chrono::Utc::now().timestamp() - 3600 // 1 hour ago
        } else {
            chrono::Utc::now().timestamp() + 3600 // 1 hour from now
        };
        StoredCredential {
            profile_name: profile.to_string(),
            access_token: format!("token-{profile}"),
            refresh_token: format!("refresh-{profile}"),
            expires_at,
            github_login: Some("alice".to_string()),
            email: Some("alice@example.com".to_string()),
        }
    }

    #[test]
    fn test_is_expired_future() {
        let cred = make_cred("prod", false);
        assert!(!cred.is_expired());
    }

    #[test]
    fn test_is_expired_past() {
        let cred = make_cred("prod", true);
        assert!(cred.is_expired());
    }

    #[test]
    fn test_is_expired_near_boundary() {
        // expires in 30 seconds — should be treated as expired (buffer = 60s)
        let cred = StoredCredential {
            profile_name: "p".to_string(),
            access_token: "t".to_string(),
            refresh_token: "r".to_string(),
            expires_at: chrono::Utc::now().timestamp() + 30,
            github_login: None,
            email: None,
        };
        assert!(cred.is_expired());
    }

    #[serial]
    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        let cred = make_cred("staging", false);
        save(&cred).unwrap();

        let loaded = load("staging").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.access_token, "token-staging");
        assert_eq!(loaded.refresh_token, "refresh-staging");
        assert_eq!(loaded.github_login, Some("alice".to_string()));
    }

    #[serial]
    #[test]
    fn test_load_missing_profile_returns_none() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        let result = load("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[serial]
    #[test]
    fn test_delete_existing_credential() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        let cred = make_cred("prod", false);
        save(&cred).unwrap();

        let removed = delete("prod").unwrap();
        assert!(removed);

        let loaded = load("prod").unwrap();
        assert!(loaded.is_none());
    }

    #[serial]
    #[test]
    fn test_delete_nonexistent_returns_false() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        let removed = delete("does-not-exist").unwrap();
        assert!(!removed);
    }

    #[serial]
    #[test]
    fn test_list_profiles_empty() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        let profiles = list_profiles().unwrap();
        assert!(profiles.is_empty());
    }

    #[serial]
    #[test]
    fn test_list_profiles_multiple() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        save(&make_cred("beta", false)).unwrap();
        save(&make_cred("alpha", false)).unwrap();
        save(&make_cred("gamma", false)).unwrap();

        let profiles = list_profiles().unwrap();
        assert_eq!(profiles, vec!["alpha", "beta", "gamma"]); // sorted
    }

    #[serial]
    #[test]
    fn test_overwrite_existing_credential() {
        let dir = TempDir::new().unwrap();
        let _guard = setup_temp_home(&dir);

        let cred1 = make_cred("prod", false);
        save(&cred1).unwrap();

        let mut cred2 = make_cred("prod", false);
        cred2.access_token = "updated-token".to_string();
        save(&cred2).unwrap();

        let loaded = load("prod").unwrap().unwrap();
        assert_eq!(loaded.access_token, "updated-token");
    }

    #[test]
    fn test_stored_credential_fields() {
        let cred = StoredCredential {
            profile_name: "test".to_string(),
            access_token: "at".to_string(),
            refresh_token: "rt".to_string(),
            expires_at: 9999999999,
            github_login: Some("bob".to_string()),
            email: Some("bob@example.com".to_string()),
        };
        assert_eq!(cred.profile_name, "test");
        assert_eq!(cred.github_login, Some("bob".to_string()));
        assert!(!cred.is_expired());
    }
}
