//! `aeterna profile` — manage CLI connection profiles.
//!
//! Subcommands: add, update, remove, list, default

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::json;

use crate::{credentials, output, profile, ux_error};

// ---------------------------------------------------------------------------
// Clap surface
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum ProfileCommand {
    #[command(about = "Add a new profile (interactive wizard or --from-file)")]
    Add(ProfileAddArgs),

    #[command(about = "Update an existing profile")]
    Update(ProfileUpdateArgs),

    #[command(about = "Remove a profile and its stored credentials")]
    Remove(ProfileRemoveArgs),

    #[command(about = "List all configured profiles")]
    List(ProfileListArgs),

    #[command(about = "Set the default profile")]
    Default(ProfileDefaultArgs),
}

#[derive(Args)]
pub struct ProfileAddArgs {
    /// Profile name (e.g. "production", "staging", "local").
    pub name: Option<String>,

    /// Aeterna server URL.
    #[arg(long)]
    pub server_url: Option<String>,

    /// Auth method: "github" or "api_key".
    #[arg(long)]
    pub auth_method: Option<String>,

    /// Tenant ID for this profile.
    #[arg(long)]
    pub tenant_id: Option<String>,

    /// Free-form label.
    #[arg(long)]
    pub label: Option<String>,

    /// GitHub App OAuth client_id for device-flow auth.
    #[arg(long)]
    pub github_client_id: Option<String>,

    /// Import profile from a TOML file instead of flags/wizard.
    #[arg(long, value_name = "FILE")]
    pub from_file: Option<PathBuf>,

    /// Set this profile as the default after creation.
    #[arg(long)]
    pub set_default: bool,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ProfileUpdateArgs {
    /// Profile name to update.
    pub name: String,

    /// New server URL.
    #[arg(long)]
    pub server_url: Option<String>,

    /// New auth method: "github" or "api_key".
    #[arg(long)]
    pub auth_method: Option<String>,

    /// New tenant ID (use "" to clear).
    #[arg(long)]
    pub tenant_id: Option<String>,

    /// New label (use "" to clear).
    #[arg(long)]
    pub label: Option<String>,

    /// New GitHub App OAuth client_id (use "" to clear).
    #[arg(long)]
    pub github_client_id: Option<String>,

    /// Import updated values from a TOML file.
    #[arg(long, value_name = "FILE")]
    pub from_file: Option<PathBuf>,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ProfileRemoveArgs {
    /// Profile name to remove.
    pub name: String,

    /// Skip confirmation prompt.
    #[arg(long, short)]
    pub yes: bool,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ProfileListArgs {
    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ProfileDefaultArgs {
    /// Profile name to set as default.
    pub name: String,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub async fn run(cmd: ProfileCommand) -> Result<()> {
    match cmd {
        ProfileCommand::Add(args) => run_add(args),
        ProfileCommand::Update(args) => run_update(args),
        ProfileCommand::Remove(args) => run_remove(args),
        ProfileCommand::List(args) => run_list(args),
        ProfileCommand::Default(args) => run_default(args),
    }
}

// ---------------------------------------------------------------------------
// add
// ---------------------------------------------------------------------------

fn run_add(args: ProfileAddArgs) -> Result<()> {
    let new_profile = if let Some(ref path) = args.from_file {
        load_profile_from_file(path)?
    } else if has_any_field(&args) {
        build_profile_from_flags(&args)?
    } else {
        run_interactive_wizard(args.name.as_deref())?
    };

    // Check for duplicates
    let existing = profile::list_profiles()?;
    if existing.iter().any(|(n, _)| n == &new_profile.name) {
        let err = ux_error::UxError::new("Profile already exists")
            .why(format!(
                "A profile named '{}' already exists",
                new_profile.name
            ))
            .fix("Use 'aeterna profile update' to modify it")
            .suggest(format!(
                "aeterna profile update {} --server-url <url>",
                new_profile.name
            ));
        err.display();
        bail!("Profile '{}' already exists", new_profile.name);
    }

    validate_profile(&new_profile)?;
    let saved_path = profile::save_profile(&new_profile)?;

    if args.set_default {
        profile::set_default_profile(&new_profile.name)?;
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "action": "added",
                "profile": new_profile.name,
                "server_url": new_profile.server_url,
                "auth_method": new_profile.auth_method.to_string(),
                "tenant_id": new_profile.tenant_id,
                "label": new_profile.label,
                "github_client_id": new_profile.github_client_id,
                "config_file": saved_path.display().to_string(),
                "is_default": args.set_default,
            }))?
        );
        return Ok(());
    }

    output::success(&format!(
        "Profile '{}' added to {}",
        new_profile.name,
        saved_path.display()
    ));
    print_profile_summary(&new_profile);
    if args.set_default {
        output::hint("Set as default profile.");
    }
    output::hint(&format!(
        "Run 'aeterna auth login --profile {}' to authenticate.",
        new_profile.name
    ));

    Ok(())
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

fn run_update(args: ProfileUpdateArgs) -> Result<()> {
    let profiles = profile::list_profiles()?;
    let existing = profiles
        .iter()
        .find(|(n, _)| n == &args.name)
        .map(|(_, p)| p.clone());

    let Some(mut p) = existing else {
        let err = ux_error::UxError::new("Profile not found")
            .why(format!("No profile named '{}'", args.name))
            .fix("Check available profiles with 'aeterna profile list'")
            .suggest(format!("aeterna profile add {}", args.name));
        err.display();
        bail!("Profile '{}' not found", args.name);
    };

    if let Some(ref path) = args.from_file {
        let from_file = load_profile_from_file(path)?;
        // Merge: file values override existing, name stays the same
        p.server_url = from_file.server_url;
        p.auth_method = from_file.auth_method;
        p.tenant_id = from_file.tenant_id;
        p.label = from_file.label;
        p.github_client_id = from_file.github_client_id;
    } else {
        if let Some(ref url) = args.server_url {
            p.server_url = url.clone();
        }
        if let Some(ref method) = args.auth_method {
            p.auth_method = parse_auth_method(method);
        }
        if let Some(ref tid) = args.tenant_id {
            p.tenant_id = if tid.is_empty() {
                None
            } else {
                Some(tid.clone())
            };
        }
        if let Some(ref lbl) = args.label {
            p.label = if lbl.is_empty() {
                None
            } else {
                Some(lbl.clone())
            };
        }
        if let Some(ref cid) = args.github_client_id {
            p.github_client_id = if cid.is_empty() {
                None
            } else {
                Some(cid.clone())
            };
        }
    }

    validate_profile(&p)?;
    let saved_path = profile::save_profile(&p)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "action": "updated",
                "profile": p.name,
                "server_url": p.server_url,
                "auth_method": p.auth_method.to_string(),
                "tenant_id": p.tenant_id,
                "label": p.label,
                "github_client_id": p.github_client_id,
                "config_file": saved_path.display().to_string(),
            }))?
        );
        return Ok(());
    }

    output::success(&format!("Profile '{}' updated", p.name));
    print_profile_summary(&p);

    Ok(())
}

// ---------------------------------------------------------------------------
// remove
// ---------------------------------------------------------------------------

fn run_remove(args: ProfileRemoveArgs) -> Result<()> {
    if !args.yes && !args.json {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Remove profile '{}' and its stored credentials?",
                args.name
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            output::info("Cancelled.");
            return Ok(());
        }
    }

    let (removed, config_path) = profile::delete_profile(&args.name)?;
    let creds_removed = credentials::delete(&args.name)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "action": "removed",
                "profile": args.name,
                "profile_removed": removed,
                "credentials_removed": creds_removed,
                "config_file": config_path.display().to_string(),
            }))?
        );
        return Ok(());
    }

    if removed {
        output::success(&format!("Profile '{}' removed", args.name));
        if creds_removed {
            println!("  Stored credentials also cleared.");
        }
    } else {
        output::warn(&format!("Profile '{}' was not found in config", args.name));
        if creds_removed {
            println!("  (Orphaned credentials for this profile were cleared.)");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn run_list(args: ProfileListArgs) -> Result<()> {
    let profiles = profile::list_profiles()?;
    let default_name = profile::default_profile_name()?;

    if args.json {
        let items: Vec<_> = profiles
            .iter()
            .map(|(name, p)| {
                json!({
                    "name": name,
                    "server_url": p.server_url,
                    "auth_method": p.auth_method.to_string(),
                    "tenant_id": p.tenant_id,
                    "label": p.label,
                    "github_client_id": p.github_client_id,
                    "is_default": default_name.as_deref() == Some(name.as_str()),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "profiles": items }))?
        );
        return Ok(());
    }

    if profiles.is_empty() {
        output::warn("No profiles configured.");
        output::hint("Run 'aeterna profile add' to create one.");
        return Ok(());
    }

    output::header("Profiles");
    println!();

    for (name, p) in &profiles {
        let marker = if default_name.as_deref() == Some(name.as_str()) {
            " (default)"
        } else {
            ""
        };
        println!("  {name}{marker}");
        println!("    Server:      {}", p.server_url);
        println!("    Auth method: {}", p.auth_method);
        if let Some(ref tid) = p.tenant_id {
            println!("    Tenant ID:   {tid}");
        }
        if let Some(ref label) = p.label {
            println!("    Label:       {label}");
        }
        if let Some(ref cid) = p.github_client_id {
            println!("    Client ID:   {cid}");
        }
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// default
// ---------------------------------------------------------------------------

fn run_default(args: ProfileDefaultArgs) -> Result<()> {
    // Verify the profile exists
    let profiles = profile::list_profiles()?;
    if !profiles.iter().any(|(n, _)| n == &args.name) {
        let err = ux_error::UxError::new("Profile not found")
            .why(format!("No profile named '{}'", args.name))
            .fix("Check available profiles with 'aeterna profile list'");
        err.display();
        bail!("Profile '{}' not found", args.name);
    }

    let saved_path = profile::set_default_profile(&args.name)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "action": "default_set",
                "profile": args.name,
                "config_file": saved_path.display().to_string(),
            }))?
        );
        return Ok(());
    }

    output::success(&format!("Default profile set to '{}'", args.name));

    Ok(())
}

// ---------------------------------------------------------------------------
// Interactive wizard
// ---------------------------------------------------------------------------

fn run_interactive_wizard(suggested_name: Option<&str>) -> Result<profile::Profile> {
    output::header("New Profile Setup");
    println!();

    let name: String = if let Some(n) = suggested_name {
        output::info(&format!("Profile name: {n}"));
        n.to_string()
    } else {
        dialoguer::Input::new()
            .with_prompt("Profile name")
            .default("default".to_string())
            .interact_text()?
    };

    let server_url: String = dialoguer::Input::new()
        .with_prompt("Aeterna server URL")
        .validate_with(|input: &String| -> std::result::Result<(), String> {
            if input.starts_with("http://") || input.starts_with("https://") {
                Ok(())
            } else {
                Err("URL must start with http:// or https://".to_string())
            }
        })
        .interact_text()?;

    let auth_methods = &["github", "api_key"];
    let auth_idx = dialoguer::Select::new()
        .with_prompt("Auth method")
        .items(auth_methods)
        .default(0)
        .interact()?;
    let auth_method = parse_auth_method(auth_methods[auth_idx]);

    let tenant_id: String = dialoguer::Input::new()
        .with_prompt("Tenant ID (leave empty to skip)")
        .default(String::new())
        .allow_empty(true)
        .interact_text()?;
    let tenant_id = if tenant_id.is_empty() {
        None
    } else {
        Some(tenant_id)
    };

    let label: String = dialoguer::Input::new()
        .with_prompt("Label (e.g. 'production', 'dev') (leave empty to skip)")
        .default(String::new())
        .allow_empty(true)
        .interact_text()?;
    let label = if label.is_empty() { None } else { Some(label) };

    let github_client_id: String = dialoguer::Input::new()
        .with_prompt("GitHub App client_id for device flow (leave empty to skip)")
        .default(String::new())
        .allow_empty(true)
        .interact_text()?;
    let github_client_id = if github_client_id.is_empty() {
        None
    } else {
        Some(github_client_id)
    };

    println!();

    Ok(profile::Profile {
        name,
        server_url,
        auth_method,
        tenant_id,
        label,
        github_client_id,
    })
}

// ---------------------------------------------------------------------------
// Manifest (TOML file) loading
// ---------------------------------------------------------------------------

fn load_profile_from_file(path: &PathBuf) -> Result<profile::Profile> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read profile file: {}", path.display()))?;
    let p: profile::Profile = toml::from_str(&raw)
        .with_context(|| format!("Invalid TOML in profile file: {}", path.display()))?;
    Ok(p)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn has_any_field(args: &ProfileAddArgs) -> bool {
    args.server_url.is_some()
        || args.auth_method.is_some()
        || args.tenant_id.is_some()
        || args.label.is_some()
        || args.github_client_id.is_some()
}

fn build_profile_from_flags(args: &ProfileAddArgs) -> Result<profile::Profile> {
    let name = args.name.clone().unwrap_or_else(|| "default".to_string());
    let server_url = args.server_url.clone().unwrap_or_default();
    let auth_method = args
        .auth_method
        .as_deref()
        .map(parse_auth_method)
        .unwrap_or_default();
    let tenant_id = args.tenant_id.clone();
    let label = args.label.clone();
    let github_client_id = args.github_client_id.clone();

    Ok(profile::Profile {
        name,
        server_url,
        auth_method,
        tenant_id,
        label,
        github_client_id,
    })
}

fn parse_auth_method(s: &str) -> profile::AuthMethod {
    match s {
        "api_key" => profile::AuthMethod::ApiKey,
        _ => profile::AuthMethod::GitHub,
    }
}

fn validate_profile(p: &profile::Profile) -> Result<()> {
    if p.name.is_empty() {
        bail!("Profile name cannot be empty");
    }
    if p.server_url.is_empty() {
        let err = ux_error::UxError::new("Profile is missing a server URL")
            .why("A server URL is required for the profile to work")
            .fix("Provide --server-url or set it in the TOML file")
            .suggest(format!(
                "aeterna profile add {} --server-url https://aeterna.example.com",
                p.name
            ));
        err.display();
        bail!("Missing server URL");
    }
    if !p.server_url.starts_with("http://") && !p.server_url.starts_with("https://") {
        bail!(
            "Server URL must start with http:// or https://, got: {}",
            p.server_url
        );
    }
    Ok(())
}

fn print_profile_summary(p: &profile::Profile) {
    println!("  Server URL:  {}", p.server_url);
    println!("  Auth method: {}", p.auth_method);
    if let Some(ref tid) = p.tenant_id {
        println!("  Tenant ID:   {tid}");
    }
    if let Some(ref label) = p.label {
        println!("  Label:       {label}");
    }
    if let Some(ref cid) = p.github_client_id {
        println!("  Client ID:   {cid}");
    }
    println!();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auth_method_github() {
        assert_eq!(parse_auth_method("github"), profile::AuthMethod::GitHub);
    }

    #[test]
    fn test_parse_auth_method_api_key() {
        assert_eq!(parse_auth_method("api_key"), profile::AuthMethod::ApiKey);
    }

    #[test]
    fn test_parse_auth_method_unknown_defaults_to_github() {
        assert_eq!(parse_auth_method("unknown"), profile::AuthMethod::GitHub);
    }

    #[test]
    fn test_validate_profile_valid() {
        let p = profile::Profile::new("test", "https://example.com");
        assert!(validate_profile(&p).is_ok());
    }

    #[test]
    fn test_validate_profile_empty_name() {
        let p = profile::Profile::new("", "https://example.com");
        assert!(validate_profile(&p).is_err());
    }

    #[test]
    fn test_validate_profile_empty_url() {
        let p = profile::Profile::new("test", "");
        assert!(validate_profile(&p).is_err());
    }

    #[test]
    fn test_validate_profile_bad_url_scheme() {
        let p = profile::Profile::new("test", "ftp://example.com");
        assert!(validate_profile(&p).is_err());
    }

    #[test]
    fn test_has_any_field_none() {
        let args = ProfileAddArgs {
            name: None,
            server_url: None,
            auth_method: None,
            tenant_id: None,
            label: None,
            github_client_id: None,
            from_file: None,
            set_default: false,
            json: false,
        };
        assert!(!has_any_field(&args));
    }

    #[test]
    fn test_has_any_field_server_url() {
        let args = ProfileAddArgs {
            name: Some("test".to_string()),
            server_url: Some("https://example.com".to_string()),
            auth_method: None,
            tenant_id: None,
            label: None,
            github_client_id: None,
            from_file: None,
            set_default: false,
            json: false,
        };
        assert!(has_any_field(&args));
    }

    #[test]
    fn test_build_profile_from_flags_defaults() {
        let args = ProfileAddArgs {
            name: None,
            server_url: Some("https://example.com".to_string()),
            auth_method: None,
            tenant_id: None,
            label: None,
            github_client_id: None,
            from_file: None,
            set_default: false,
            json: false,
        };
        let p = build_profile_from_flags(&args).unwrap();
        assert_eq!(p.name, "default");
        assert_eq!(p.server_url, "https://example.com");
        assert_eq!(p.auth_method, profile::AuthMethod::GitHub);
    }

    #[test]
    fn test_build_profile_from_flags_all_set() {
        let args = ProfileAddArgs {
            name: Some("staging".to_string()),
            server_url: Some("https://staging.example.com".to_string()),
            auth_method: Some("api_key".to_string()),
            tenant_id: Some("t-1".to_string()),
            label: Some("staging".to_string()),
            github_client_id: Some("Iv1.abc123".to_string()),
            from_file: None,
            set_default: false,
            json: false,
        };
        let p = build_profile_from_flags(&args).unwrap();
        assert_eq!(p.name, "staging");
        assert_eq!(p.auth_method, profile::AuthMethod::ApiKey);
        assert_eq!(p.tenant_id, Some("t-1".to_string()));
        assert_eq!(p.github_client_id, Some("Iv1.abc123".to_string()));
    }

    #[test]
    fn test_add_args_defaults() {
        let args = ProfileAddArgs {
            name: None,
            server_url: None,
            auth_method: None,
            tenant_id: None,
            label: None,
            github_client_id: None,
            from_file: None,
            set_default: false,
            json: false,
        };
        assert!(args.name.is_none());
        assert!(args.from_file.is_none());
        assert!(!args.set_default);
        assert!(!args.json);
    }

    #[test]
    fn test_update_args_clear_optional_fields() {
        let args = ProfileUpdateArgs {
            name: "prod".to_string(),
            server_url: None,
            auth_method: None,
            tenant_id: Some(String::new()),
            label: Some(String::new()),
            github_client_id: Some(String::new()),
            from_file: None,
            json: false,
        };
        // Empty strings signal "clear this field"
        assert_eq!(args.tenant_id, Some(String::new()));
        assert_eq!(args.label, Some(String::new()));
        assert_eq!(args.github_client_id, Some(String::new()));
    }

    #[test]
    fn test_remove_args() {
        let args = ProfileRemoveArgs {
            name: "old".to_string(),
            yes: true,
            json: false,
        };
        assert_eq!(args.name, "old");
        assert!(args.yes);
    }

    #[test]
    fn test_list_args_json() {
        let args = ProfileListArgs { json: true };
        assert!(args.json);
    }

    #[test]
    fn test_default_args() {
        let args = ProfileDefaultArgs {
            name: "prod".to_string(),
            json: false,
        };
        assert_eq!(args.name, "prod");
    }

    #[test]
    fn test_load_profile_from_file_valid() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("profile.toml");
        std::fs::write(
            &path,
            r#"
name = "ci"
server_url = "https://ci.example.com"
auth_method = "git_hub"
"#,
        )
        .unwrap();
        let p = load_profile_from_file(&path).unwrap();
        assert_eq!(p.name, "ci");
        assert_eq!(p.server_url, "https://ci.example.com");
    }

    #[test]
    fn test_load_profile_from_file_with_all_fields() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("profile.toml");
        std::fs::write(
            &path,
            r#"
name = "prod"
server_url = "https://prod.example.com"
auth_method = "api_key"
tenant_id = "tenant-42"
label = "production"
github_client_id = "Iv1.xyz789"
"#,
        )
        .unwrap();
        let p = load_profile_from_file(&path).unwrap();
        assert_eq!(p.name, "prod");
        assert_eq!(p.auth_method, profile::AuthMethod::ApiKey);
        assert_eq!(p.tenant_id, Some("tenant-42".to_string()));
        assert_eq!(p.github_client_id, Some("Iv1.xyz789".to_string()));
    }

    #[test]
    fn test_load_profile_from_file_invalid_toml() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "not valid [toml").unwrap();
        assert!(load_profile_from_file(&path).is_err());
    }

    #[test]
    fn test_load_profile_from_file_missing() {
        let path = PathBuf::from("/nonexistent/profile.toml");
        assert!(load_profile_from_file(&path).is_err());
    }
}
