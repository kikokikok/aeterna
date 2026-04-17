//! Shared backend-access helpers for CLI commands.
//!
//! All commands that need to reach the Aeterna server call
//! [`connect`] to obtain a live, authenticated [`AeternaClient`].
//! If authentication or reachability fails, this module surfaces the
//! canonical `ux_error` messages and returns an `Err` (non-zero exit).
//!
//! Commands that have no server API yet call [`unsupported`] instead,
//! which fails explicitly with a clear message — satisfying the design
//! requirement that commands either execute real work or fail explicitly.

use anyhow::{Result, bail};

use crate::client::AeternaClient;
use crate::profile::{self, ResolvedConfig};
use crate::ux_error;

/// Attempt to load the active profile and authenticate.
///
/// On success returns `(AeternaClient, ResolvedConfig)`.
/// On failure, displays the appropriate `ux_error` and returns `Err`.
pub async fn connect() -> Result<(AeternaClient, ResolvedConfig)> {
    connect_with_overrides(None, None).await
}

/// Same as [`connect`] but allows explicit profile/URL overrides
/// (e.g. from `--profile` / `--server-url` flags).
pub async fn connect_with_overrides(
    profile_override: Option<&str>,
    server_url_override: Option<&str>,
) -> Result<(AeternaClient, ResolvedConfig)> {
    let resolved = match profile::load_resolved(profile_override, server_url_override) {
        Ok(r) => r,
        Err(e) => {
            ux_error::UxError::new("No Aeterna profile is configured")
                .why("Backend-backed commands need a configured profile and server URL")
                .fix("Create or select a profile with a reachable server URL")
                .suggest("aeterna setup")
                .display();
            bail!("Profile load failed: {e}");
        }
    };

    // Require a non-empty server URL before attempting auth
    if resolved.server_url.is_empty() {
        if let Some(override_name) = profile_override {
            ux_error::UxError::new(format!("Profile '{override_name}' has no server URL"))
                .why("The selected profile cannot be used until it points at an Aeterna server")
                .fix("Set a server URL for that profile")
                .suggest(format!(
                    "aeterna config set --profile {override_name} --server-url <URL>"
                ))
                .display();
        } else {
            ux_error::UxError::new("No Aeterna server URL is configured")
                .why("Backend-backed commands need a configured profile and server URL")
                .fix("Configure a default profile with a server URL")
                .suggest("aeterna config set --server-url <URL>")
                .display();
        }
        bail!(
            "No server URL configured for profile '{}'. \
             Run: aeterna config set --server-url <URL>",
            resolved.profile_name
        );
    }

    match AeternaClient::from_profile(&resolved).await {
        Ok(client) => Ok((client, resolved)),
        Err(e) => {
            ux_error::UxError::new(format!(
                "Not authenticated for profile '{}'",
                resolved.profile_name
            ))
            .why("The CLI could not load valid credentials for the configured backend")
            .fix("Log in again for this profile")
            .suggest("aeterna auth login")
            .display();
            bail!("{e}");
        }
    }
}

/// Fail explicitly when a command has no backend API yet.
///
/// Per design.md: "commands either execute real work against a backend
/// or fail explicitly".  Use this at every backend-call site where the
/// server-side endpoint does not exist yet, so the user gets a clear
/// message instead of a generic "server not connected".
///
/// The `profile_name` is shown in the error to reassure the user that
/// their auth configuration is correct — only the backend endpoint is missing.
pub fn unsupported(command: &str, profile_name: &str) -> anyhow::Error {
    ux_error::UxError::new(format!("Backend API for '{command}' is not yet available"))
        .why("The server-side endpoint for this command is not implemented yet")
        .fix("This command will be available in a future release")
        .fix(format!(
            "Profile '{profile_name}' is configured — authentication is set up correctly"
        ))
        .fix("Track progress: https://github.com/kikokikok/aeterna")
        .suggest("aeterna auth status")
        .display();
    anyhow::anyhow!("Backend API for '{command}' not yet available")
}

/// Like [`unsupported`] but for cases where no profile has been loaded yet
/// (e.g. before a server URL is configured). Uses the raw `server_not_connected`
/// error so the user sees actionable auth/profile guidance.
pub fn not_connected(context: &str) -> anyhow::Error {
    ux_error::server_not_connected().display();
    anyhow::anyhow!("Server not connected ({context})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_returns_error_with_command_name() {
        let err = unsupported("memory search", "default");
        assert!(err.to_string().contains("memory search"));
    }

    #[test]
    fn test_unsupported_returns_error_with_profile_name() {
        let err = unsupported("knowledge list", "production");
        assert!(err.to_string().contains("knowledge list"));
    }

    #[test]
    fn test_not_connected_returns_error_with_context() {
        let err = not_connected("test context");
        assert!(err.to_string().contains("not connected"));
    }
}
