use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use context::ContextResolver;

use crate::output;

#[derive(Args)]
pub struct InitArgs {
    #[arg(short, long, help = "Directory to initialize (defaults to current)")]
    pub path: Option<PathBuf>,

    #[arg(long, help = "Tenant ID to use")]
    pub tenant_id: Option<String>,

    #[arg(long, help = "User ID (defaults to git user.email)")]
    pub user_id: Option<String>,

    #[arg(long, help = "Organization ID")]
    pub org_id: Option<String>,

    #[arg(long, help = "Team ID")]
    pub team_id: Option<String>,

    #[arg(long, help = "Project ID (defaults to git remote org/repo)")]
    pub project_id: Option<String>,

    #[arg(long, help = "Default hints preset", default_value = "standard")]
    pub preset: String,

    #[arg(long, help = "Force overwrite existing context.toml")]
    pub force: bool,

    #[arg(long, help = "Skip interactive prompts")]
    pub yes: bool,
}

pub fn run(args: InitArgs) -> Result<()> {
    let target_dir = args.path.unwrap_or_else(|| PathBuf::from("."));
    let aeterna_dir = target_dir.join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    if context_file.exists() && !args.force {
        output::warn(&format!(
            "Context already exists at {}",
            context_file.display()
        ));
        output::info("Use --force to overwrite");
        return Ok(());
    }

    let resolver = ContextResolver::from_dir(&target_dir).skip_env();

    let ctx = resolver.resolve()?;

    let tenant_id = args
        .tenant_id
        .or_else(|| {
            if ctx.tenant_id.value != "default" {
                Some(ctx.tenant_id.value.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "default".to_string());

    let user_id = args.user_id.unwrap_or_else(|| ctx.user_id.value.clone());

    let project_id = args
        .project_id
        .or_else(|| ctx.project_id.as_ref().map(|p| p.value.clone()));

    let mut toml_content = String::new();
    toml_content.push_str(&format!("tenant-id = \"{tenant_id}\"\n"));
    toml_content.push_str(&format!("user-id = \"{user_id}\"\n"));

    if let Some(org) = &args.org_id {
        toml_content.push_str(&format!("org-id = \"{org}\"\n"));
    }

    if let Some(team) = &args.team_id {
        toml_content.push_str(&format!("team-id = \"{team}\"\n"));
    }

    if let Some(project) = &project_id {
        toml_content.push_str(&format!("project-id = \"{project}\"\n"));
    }

    toml_content.push_str(&format!(
        r#"
[hints]
preset = "{}"
"#,
        args.preset
    ));

    fs::create_dir_all(&aeterna_dir)
        .with_context(|| format!("Failed to create {}", aeterna_dir.display()))?;

    fs::write(&context_file, toml_content)
        .with_context(|| format!("Failed to write {}", context_file.display()))?;

    println!(
        "{} Initialized Aeterna at {}",
        "✓".green().bold(),
        aeterna_dir.display()
    );

    println!("\n{}", "Resolved context:".bold());
    println!("  tenant_id:  {}", tenant_id.cyan());
    println!("  user_id:    {}", user_id.cyan());
    if let Some(project) = &project_id {
        println!("  project_id: {}", project.cyan());
    }
    println!("  preset:     {}", args.preset.cyan());

    println!(
        "\n{}",
        "Run 'aeterna status' to see the full resolved context.".dimmed()
    );

    Ok(())
}
