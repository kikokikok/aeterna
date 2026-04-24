//! `aeterna config` — profile and configuration management subcommands.
//!
//! Subcommands: show, set, validate, default-profile

use clap::{Args, Subcommand};
use serde_json::json;

use crate::{output, profile, ux_error};

// ---------------------------------------------------------------------------
// Clap surface
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum ConfigCommand {
    #[command(about = "Show the effective configuration and active profile")]
    Show(ShowArgs),

    #[command(about = "Set or update a profile value")]
    Set(SetArgs),

    #[command(about = "Validate the current configuration")]
    Validate(ValidateArgs),

    #[command(name = "default-profile", about = "Set the default profile")]
    DefaultProfile(DefaultProfileArgs),
}

#[derive(Args)]
pub struct ShowArgs {
    /// Profile to display (defaults to the configured default profile).
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,

    /// Show all profiles, not just the active one.
    #[arg(long)]
    pub all: bool,
}

#[derive(Args)]
pub struct SetArgs {
    /// Profile name to create or update.
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Server URL for this profile.
    #[arg(long)]
    pub server_url: Option<String>,

    /// Auth method for this profile: "github" or "api_key".
    #[arg(long)]
    pub auth_method: Option<String>,

    /// Tenant ID override for this profile.
    #[arg(long)]
    pub tenant_id: Option<String>,

    /// Free-form label for this profile (e.g. "production", "dev").
    #[arg(long)]
    pub label: Option<String>,
}

#[derive(Args)]
pub struct ValidateArgs {
    /// Profile to validate (validates all profiles if omitted).
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DefaultProfileArgs {
    /// Name of the profile to set as default.
    pub profile_name: String,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub async fn run(cmd: ConfigCommand) -> anyhow::Result<()> {
    match cmd {
        ConfigCommand::Show(args) => run_show(args),
        ConfigCommand::Set(args) => run_set(args),
        ConfigCommand::Validate(args) => run_validate(args),
        ConfigCommand::DefaultProfile(args) => run_default_profile(args),
    }
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn run_show(args: ShowArgs) -> anyhow::Result<()> {
    let resolved = profile::load_resolved(args.profile.as_deref(), None)?;

    // Load both config files for display
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_path = profile::project_config_path(&cwd);
    let user_path = profile::user_config_path();

    let project_cfg = project_path
        .as_deref()
        .map(profile::load_config_file)
        .transpose()?;
    let user_cfg = user_path
        .as_ref()
        .filter(|p| p.exists())
        .map(|p| profile::load_config_file(p))
        .transpose()?;

    if args.json {
        let mut profiles_map = serde_json::Map::new();

        if let Some(ref cfg) = user_cfg {
            for (k, v) in &cfg.profiles {
                profiles_map.insert(k.clone(), json!(v));
            }
        }
        if let Some(ref cfg) = project_cfg {
            for (k, v) in &cfg.profiles {
                profiles_map.insert(k.clone(), json!(v)); // project overrides user
            }
        }

        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "active_profile": resolved.profile_name,
                "server_url": resolved.server_url,
                "profile_source": resolved.profile_source.to_string(),
                "user_config": user_path.as_ref().map(|p| p.display().to_string()),
                "project_config": project_path.as_ref().map(|p| p.display().to_string()),
                "profiles": profiles_map,
            }))?
        );
        return Ok(());
    }

    output::header("Aeterna Configuration");
    println!();

    // Precedence model documentation
    output::subheader("Config file locations (canonical)");
    if let Some(ref p) = user_path {
        println!(
            "  User-level:    {}{}",
            p.display(),
            if p.exists() { "" } else { "  (not found)" }
        );
    }
    if let Some(ref p) = project_path {
        println!("  Project-level: {}", p.display());
    } else {
        println!("  Project-level: (none found — searched from CWD)");
    }
    println!();

    output::subheader("Precedence (highest → lowest)");
    println!("  1. CLI flags");
    println!("  2. AETERNA_* environment variables");
    println!("  3. Project config (.aeterna/config.toml)");
    println!("  4. User config (~/.config/aeterna/config.toml)");
    println!("  5. Built-in defaults");
    println!();

    output::subheader(&format!("Active profile: {}", resolved.profile_name));
    println!("  Source:     {}", resolved.profile_source);
    println!("  Server URL: {}", resolved.server_url);
    if let Some(ref tid) = resolved.tenant_id {
        println!("  Tenant ID:  {tid}");
    }
    println!();

    if args.all {
        // Show all profiles from both files
        let mut all_profiles: std::collections::HashMap<String, (profile::Profile, String)> =
            std::collections::HashMap::new();

        if let Some(ref cfg) = user_cfg {
            for (k, v) in &cfg.profiles {
                all_profiles.insert(k.clone(), (v.clone(), "user".to_string()));
            }
        }
        if let Some(ref cfg) = project_cfg {
            for (k, v) in &cfg.profiles {
                all_profiles.insert(k.clone(), (v.clone(), "project".to_string()));
            }
        }

        if all_profiles.is_empty() {
            output::hint("No profiles configured. Run 'aeterna auth login' to create one.");
        } else {
            output::subheader("All profiles");
            let mut names: Vec<&String> = all_profiles.keys().collect();
            names.sort();
            for name in names {
                let (p, source) = &all_profiles[name];
                let active_marker = if name == &resolved.profile_name {
                    " (active)"
                } else {
                    ""
                };
                println!("  {name}{active_marker}  [{source}]");
                println!("    Server:      {}", p.server_url);
                println!("    Auth method: {}", p.auth_method);
                if let Some(ref tid) = p.tenant_id {
                    println!("    Tenant ID:   {tid}");
                }
                if let Some(ref label) = p.label {
                    println!("    Label:       {label}");
                }
                println!();
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// set
// ---------------------------------------------------------------------------

fn run_set(args: SetArgs) -> anyhow::Result<()> {
    let profile_name = args
        .profile
        .clone()
        .unwrap_or_else(|| "default".to_string());

    // Load existing profile if present, otherwise start from scratch
    let user_path = profile::user_config_path();
    let existing = user_path
        .as_ref()
        .filter(|p| p.exists())
        .map(|p| profile::load_config_file(p))
        .transpose()?
        .and_then(|c| c.profiles.get(&profile_name).cloned());

    let server_url = args
        .server_url
        .or_else(|| existing.as_ref().map(|p| p.server_url.clone()))
        .unwrap_or_default();

    let auth_method = match args
        .auth_method
        .as_deref()
        .or_else(|| existing.as_ref().map(|_| "github"))
    {
        Some("api_key") => profile::AuthMethod::ApiKey,
        _ => profile::AuthMethod::GitHub,
    };

    let tenant_id = args
        .tenant_id
        .or_else(|| existing.as_ref().and_then(|p| p.tenant_id.clone()));

    let label = args
        .label
        .or_else(|| existing.as_ref().and_then(|p| p.label.clone()));

    let new_profile = profile::Profile {
        name: profile_name.clone(),
        server_url,
        auth_method,
        tenant_id,
        label,
        github_client_id: existing.as_ref().and_then(|p| p.github_client_id.clone()),
    };

    // Validate
    if new_profile.server_url.is_empty() {
        let err = ux_error::UxError::new("Profile is missing a server URL")
            .why("A server URL is required to use this profile")
            .fix("Provide --server-url when calling 'aeterna config set'")
            .suggest(format!(
                "aeterna config set --profile {profile_name} --server-url https://aeterna.example.com"
            ));
        err.display();
        return Err(anyhow::anyhow!("Missing server URL"));
    }

    let saved_path = profile::save_profile(&new_profile)?;

    output::success(&format!(
        "Profile '{profile_name}' saved to {}",
        saved_path.display()
    ));
    println!("  Server URL: {}", new_profile.server_url);
    println!("  Auth method: {}", new_profile.auth_method);
    if let Some(ref tid) = new_profile.tenant_id {
        println!("  Tenant ID: {tid}");
    }
    println!();
    output::hint(&format!(
        "Run 'aeterna auth login --profile {profile_name}' to authenticate."
    ));

    Ok(())
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

fn run_validate(args: ValidateArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let user_path = profile::user_config_path();
    let project_path = profile::project_config_path(&cwd);

    let mut issues: Vec<String> = Vec::new();
    let mut all_profiles: Vec<(String, profile::Profile)> = Vec::new();

    let sources: &[(&Option<std::path::PathBuf>, &str)] =
        &[(&user_path, "user"), (&project_path, "project")];

    for (path_opt, source) in sources {
        if let Some(path) = path_opt
            && path.exists()
        {
            match profile::load_config_file(path) {
                Ok(cfg) => {
                    for (name, p) in &cfg.profiles {
                        if p.server_url.is_empty() {
                            issues.push(format!("[{source}] Profile '{name}': missing server_url"));
                        } else if !p.server_url.starts_with("http://")
                            && !p.server_url.starts_with("https://")
                        {
                            issues.push(format!(
                                    "[{source}] Profile '{name}': server_url must start with http:// or https://"
                                ));
                        }
                        // Filter by --profile if provided
                        if args.profile.as_deref().is_none_or(|n| n == name) {
                            all_profiles.push((name.clone(), p.clone()));
                        }
                    }
                }
                Err(e) => {
                    issues.push(format!("[{source}] Cannot parse {}: {e}", path.display()));
                }
            }
        }
    }

    let valid = issues.is_empty();

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "valid": valid,
                "issues": issues,
                "profiles_checked": all_profiles.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }

    output::header("Config Validation");
    println!();

    if all_profiles.is_empty() {
        output::warn("No profiles found in any config file.");
        output::hint("Run 'aeterna auth login' to create a profile.");
    } else {
        for (name, p) in &all_profiles {
            println!("  Profile: {name}");
            println!("    Server URL: {}", p.server_url);
            println!("    Auth method: {}", p.auth_method);
        }
        println!();
    }

    if valid {
        output::success("Configuration is valid.");
    } else {
        output::error("Configuration has issues:");
        for issue in &issues {
            println!("  - {issue}");
        }
        println!();
        return Err(anyhow::anyhow!("Configuration validation failed"));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// default-profile
// ---------------------------------------------------------------------------

fn run_default_profile(args: DefaultProfileArgs) -> anyhow::Result<()> {
    let saved_path = profile::set_default_profile(&args.profile_name)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "default_profile": args.profile_name,
                "config_file": saved_path.display().to_string(),
            }))?
        );
        return Ok(());
    }

    output::success(&format!(
        "Default profile set to '{}' in {}",
        args.profile_name,
        saved_path.display()
    ));
    output::hint("Run 'aeterna config show' to verify the active configuration.");

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_args_defaults() {
        let args = ShowArgs {
            profile: None,
            json: false,
            all: false,
        };
        assert!(args.profile.is_none());
        assert!(!args.json);
        assert!(!args.all);
    }

    #[test]
    fn test_show_args_with_profile() {
        let args = ShowArgs {
            profile: Some("prod".to_string()),
            json: true,
            all: true,
        };
        assert_eq!(args.profile, Some("prod".to_string()));
        assert!(args.json);
        assert!(args.all);
    }

    #[test]
    fn test_set_args_defaults() {
        let args = SetArgs {
            profile: None,
            server_url: None,
            auth_method: None,
            tenant_id: None,
            label: None,
        };
        assert!(args.profile.is_none());
        assert!(args.server_url.is_none());
    }

    #[test]
    fn test_set_args_with_values() {
        let args = SetArgs {
            profile: Some("staging".to_string()),
            server_url: Some("https://staging.example.com".to_string()),
            auth_method: Some("api_key".to_string()),
            tenant_id: Some("t-123".to_string()),
            label: Some("staging env".to_string()),
        };
        assert_eq!(args.profile, Some("staging".to_string()));
        assert_eq!(
            args.server_url,
            Some("https://staging.example.com".to_string())
        );
        assert_eq!(args.auth_method, Some("api_key".to_string()));
        assert_eq!(args.tenant_id, Some("t-123".to_string()));
        assert_eq!(args.label, Some("staging env".to_string()));
    }

    #[test]
    fn test_validate_args_defaults() {
        let args = ValidateArgs {
            profile: None,
            json: false,
        };
        assert!(args.profile.is_none());
        assert!(!args.json);
    }

    #[test]
    fn test_default_profile_args() {
        let args = DefaultProfileArgs {
            profile_name: "production".to_string(),
            json: false,
        };
        assert_eq!(args.profile_name, "production");
        assert!(!args.json);
    }

    #[test]
    fn test_default_profile_args_json() {
        let args = DefaultProfileArgs {
            profile_name: "staging".to_string(),
            json: true,
        };
        assert_eq!(args.profile_name, "staging");
        assert!(args.json);
    }
}
