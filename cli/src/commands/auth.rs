//! `aeterna auth` — interactive authentication subcommands.
//!
//! Subcommands: login, logout, status

use clap::{Args, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;
use std::process::Command;

use crate::{client, credentials, output, profile, ux_error};

// ---------------------------------------------------------------------------
// Clap surface
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum AuthCommand {
    #[command(about = "Log in to an Aeterna server and save credentials")]
    Login(LoginArgs),

    #[command(about = "Log out from an Aeterna server and clear stored credentials")]
    Logout(LogoutArgs),

    #[command(about = "Show authentication status for the selected profile")]
    Status(StatusArgs),
}

#[derive(Args)]
pub struct LoginArgs {
    /// GitHub personal access token (PAT) to exchange for Aeterna credentials.
    /// If omitted, the CLI starts GitHub device-flow login.
    #[arg(long, env = "GITHUB_TOKEN")]
    pub github_token: Option<String>,

    /// Profile name to log in to (defaults to the configured default profile).
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Aeterna server URL (overrides profile config and AETERNA_SERVER_URL).
    #[arg(long)]
    pub server_url: Option<String>,

    /// Output result as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct LogoutArgs {
    /// Profile to log out from.
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Skip the confirmation prompt.
    #[arg(long, short)]
    pub yes: bool,

    /// Output result as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Profile to check (defaults to the configured default profile).
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Output result as JSON.
    #[arg(long)]
    pub json: bool,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub async fn run(cmd: AuthCommand) -> anyhow::Result<()> {
    match cmd {
        AuthCommand::Login(args) => run_login(args).await,
        AuthCommand::Logout(args) => run_logout(args).await,
        AuthCommand::Status(args) => run_status(args).await,
    }
}

// ---------------------------------------------------------------------------
// login
// ---------------------------------------------------------------------------

async fn run_login(args: LoginArgs) -> anyhow::Result<()> {
    // Resolve profile / server URL
    let resolved = profile::load_resolved(args.profile.as_deref(), args.server_url.as_deref())?;

    let server_url = if resolved.server_url.is_empty() {
        // Prompt for server URL if not configured
        let prompted: String = dialoguer::Input::new()
            .with_prompt("Aeterna server URL")
            .interact_text()?;
        prompted
    } else {
        resolved.server_url.clone()
    };

    let github_token = if let Some(t) = args.github_token {
        t
    } else {
        let github_client_id = std::env::var("AETERNA_GITHUB_CLIENT_ID")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                resolved
                    .profile
                    .github_client_id
                    .clone()
                    .filter(|v| !v.trim().is_empty())
            })
            .ok_or_else(|| {
                let err = ux_error::UxError::new("Missing GitHub OAuth client ID")
                    .why("Device-flow login requires a GitHub OAuth App client ID")
                    .fix("Set AETERNA_GITHUB_CLIENT_ID in your environment")
                    .fix("Or configure profile.github_client_id in your Aeterna profile")
                    .suggest("aeterna auth login --github-token <PAT>");
                err.display();
                anyhow::anyhow!(
                    "Missing GitHub OAuth client ID. Set AETERNA_GITHUB_CLIENT_ID or configure profile.github_client_id."
                )
            })?;

        let github_oauth_base_url = std::env::var("AETERNA_GITHUB_OAUTH_BASE_URL").ok();

        let device = client::request_device_code(
            &github_client_id,
            "read:user,user:email",
            github_oauth_base_url.as_deref(),
        )
        .await
        .map_err(|e| {
            let err = ux_error::UxError::new("GitHub device flow setup failed")
                .why(e.to_string())
                .fix("Verify the GitHub OAuth client ID is correct")
                .fix("Check network access to github.com")
                .suggest("aeterna auth login");
            err.display();
            anyhow::anyhow!("Device flow setup failed: {e}")
        })?;

        if !args.json {
            output::info(&format!(
                "Open {} and enter code {}",
                device.verification_uri.bold().underline(),
                device.user_code.bold().yellow()
            ));
        }

        let _ = try_open_browser(&device.verification_uri);

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template("{spinner} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        spinner.set_message("Waiting for authorization...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(120));

        let token_result = client::poll_device_authorization(
            &github_client_id,
            &device.device_code,
            device.interval,
            device.expires_in,
            github_oauth_base_url.as_deref(),
        )
        .await;

        match token_result {
            Ok(token) => {
                spinner.finish_and_clear();
                token
            }
            Err(e) => {
                spinner.finish_and_clear();
                let err = ux_error::UxError::new("GitHub device authorization failed")
                    .why(e.to_string())
                    .fix("Complete the device authorization in your browser")
                    .fix("Retry login if the code expired")
                    .suggest("aeterna auth login");
                err.display();
                return Err(anyhow::anyhow!("Device authorization failed: {e}"));
            }
        }
    };

    if !args.json {
        output::info(&format!(
            "Logging in to {} (profile: {}) …",
            server_url, resolved.profile_name
        ));
    }

    // Exchange GitHub token for Aeterna credentials
    let token_resp = client::bootstrap_github(&server_url, &github_token)
        .await
        .map_err(|e| {
            let err = ux_error::UxError::new("Authentication failed")
                .why(e.to_string())
                .fix("Check your GitHub token has the required scopes")
                .fix("Ensure the server URL is reachable")
                .suggest(format!("aeterna auth login --server-url {server_url}"));
            err.display();
            anyhow::anyhow!("Authentication failed: {e}")
        })?;

    let expires_at = chrono::Utc::now().timestamp() + token_resp.expires_in as i64;

    let cred = credentials::StoredCredential {
        profile_name: resolved.profile_name.clone(),
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
        github_login: token_resp.github_login.clone(),
        email: token_resp.email.clone(),
    };
    credentials::save(&cred)?;

    // If the profile doesn't exist in config yet, save it now
    if resolved.profile_source == profile::ConfigSource::Default {
        let new_profile = profile::Profile {
            name: resolved.profile_name.clone(),
            server_url: server_url.clone(),
            auth_method: profile::AuthMethod::GitHub,
            tenant_id: resolved.tenant_id.clone(),
            label: None,
            github_client_id: resolved.profile.github_client_id.clone(),
        };
        profile::save_profile(&new_profile)?;
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "logged_in",
                "profile": resolved.profile_name,
                "server_url": server_url,
                "github_login": token_resp.github_login,
                "email": token_resp.email,
                "expires_at": expires_at,
            }))?
        );
    } else {
        output::success(&format!(
            "Logged in to {} (profile: {})",
            server_url, resolved.profile_name
        ));
        if let Some(ref login) = token_resp.github_login {
            println!("  GitHub login: {login}");
        }
        if let Some(ref email) = token_resp.email {
            println!("  Email:        {email}");
        }
        println!();
        output::hint("Credentials stored. Run 'aeterna auth status' to verify.");
    }

    Ok(())
}

fn try_open_browser(url: &str) -> bool {
    let os = std::env::consts::OS;
    let status = match os {
        "macos" => Command::new("open").arg(url).status(),
        "linux" => Command::new("xdg-open").arg(url).status(),
        _ => return false,
    };

    status.map(|s| s.success()).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// logout
// ---------------------------------------------------------------------------

async fn run_logout(args: LogoutArgs) -> anyhow::Result<()> {
    let resolved = profile::load_resolved(args.profile.as_deref(), None)?;
    let profile_name = &resolved.profile_name;

    // Check we have credentials at all
    let cred = credentials::load(profile_name)?;
    if cred.is_none() {
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "not_logged_in",
                    "profile": profile_name,
                }))?
            );
        } else {
            println!("Not logged in (profile: {profile_name}).");
            output::warn(&format!("Not logged in (profile: {profile_name})."));
        }
        return Ok(());
    }
    let cred = cred.unwrap();

    // Confirm
    if !args.yes && !args.json {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Log out from {} (profile: {})?",
                resolved.server_url, profile_name
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            output::info("Logout cancelled.");
            return Ok(());
        }
    }

    // Best-effort server-side revocation (don't fail the local logout)
    if !resolved.server_url.is_empty()
        && let Err(e) = client::server_logout(&resolved.server_url, &cred.refresh_token).await {
            output::warn(&format!("Server-side revocation failed (continuing): {e}"));
        }

    // Remove local credentials
    credentials::delete(profile_name)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "logged_out",
                "profile": profile_name,
            }))?
        );
    } else {
        output::success(&format!("Logged out (profile: {profile_name})."));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

async fn run_status(args: StatusArgs) -> anyhow::Result<()> {
    let resolved = profile::load_resolved(args.profile.as_deref(), None)?;
    let profile_name = &resolved.profile_name;

    let cred = credentials::load(profile_name)?;

    // Determine auth state
    let (state, detail) = match &cred {
        None => ("unauthenticated", None),
        Some(c) if c.is_expired() => ("expired", Some(c)),
        Some(c) => ("authenticated", Some(c)),
    };

    if args.json {
        let mut obj = json!({
            "profile": profile_name,
            "server_url": resolved.server_url,
            "status": state,
        });
        if let Some(c) = &cred {
            obj["github_login"] = json!(c.github_login);
            obj["email"] = json!(c.email);
            obj["expires_at"] = json!(c.expires_at);
        }
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(());
    }

    output::header("Auth Status");
    println!();
    println!("  Profile:    {profile_name}");
    println!("  Server:     {}", resolved.server_url);
    println!("  Source:     {}", resolved.profile_source);
    println!();

    match state {
        "authenticated" => {
            let c = detail.unwrap();
            output::success("Authenticated");
            if let Some(ref login) = c.github_login {
                println!("  GitHub login: {login}");
            }
            if let Some(ref email) = c.email {
                println!("  Email:        {email}");
            }
            let expires = chrono::DateTime::<chrono::Utc>::from_timestamp(c.expires_at, 0)
                .map_or_else(
                    || "unknown".to_string(),
                    |d| d.format("%Y-%m-%d %H:%M UTC").to_string(),
                );
            println!("  Token valid until: {expires}");
        }
        "expired" => {
            output::warn("Session expired — will refresh automatically on next command.");
            output::hint("aeterna auth login to re-authenticate manually.");
        }
        _ => {
            output::warn("Not authenticated.");
            output::hint(&format!("aeterna auth login --profile {profile_name}"));
        }
    }

    println!();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_args_defaults() {
        let args = LoginArgs {
            github_token: None,
            profile: None,
            server_url: None,
            json: false,
        };
        assert!(args.github_token.is_none());
        assert!(args.profile.is_none());
        assert!(args.server_url.is_none());
        assert!(!args.json);
    }

    #[test]
    fn test_login_args_with_values() {
        let args = LoginArgs {
            github_token: Some("gho_abc".to_string()),
            profile: Some("prod".to_string()),
            server_url: Some("https://aeterna.example.com".to_string()),
            json: true,
        };
        assert_eq!(args.github_token, Some("gho_abc".to_string()));
        assert_eq!(args.profile, Some("prod".to_string()));
        assert_eq!(
            args.server_url,
            Some("https://aeterna.example.com".to_string())
        );
        assert!(args.json);
    }

    #[test]
    fn test_logout_args_defaults() {
        let args = LogoutArgs {
            profile: None,
            yes: false,
            json: false,
        };
        assert!(args.profile.is_none());
        assert!(!args.yes);
        assert!(!args.json);
    }

    #[test]
    fn test_logout_args_with_values() {
        let args = LogoutArgs {
            profile: Some("staging".to_string()),
            yes: true,
            json: true,
        };
        assert_eq!(args.profile, Some("staging".to_string()));
        assert!(args.yes);
        assert!(args.json);
    }

    #[test]
    fn test_status_args_defaults() {
        let args = StatusArgs {
            profile: None,
            json: false,
        };
        assert!(args.profile.is_none());
        assert!(!args.json);
    }

    #[test]
    fn test_status_args_json_mode() {
        let args = StatusArgs {
            profile: Some("local".to_string()),
            json: true,
        };
        assert_eq!(args.profile, Some("local".to_string()));
        assert!(args.json);
    }
}
