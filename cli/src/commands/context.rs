use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use context::ContextResolver;

#[derive(Subcommand)]
pub enum ContextCommand {
    #[command(about = "Show resolved context")]
    Show(ShowArgs),

    #[command(about = "Set a context value in .aeterna/context.toml")]
    Set(SetArgs),

    #[command(about = "Clear context file")]
    Clear(ClearArgs),
}

#[derive(Args)]
pub struct ShowArgs {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Args)]
pub struct SetArgs {
    #[arg(help = "Key to set (tenant-id, user-id, org-id, team-id, project-id)")]
    pub key: String,

    #[arg(help = "Value to set")]
    pub value: String,
}

#[derive(Args)]
pub struct ClearArgs {
    #[arg(long, help = "Also remove .aeterna directory")]
    pub all: bool,
}

pub fn run(cmd: ContextCommand) -> Result<()> {
    match cmd {
        ContextCommand::Show(args) => show(args),
        ContextCommand::Set(args) => set(args),
        ContextCommand::Clear(args) => clear(args),
    }
}

fn show(args: ShowArgs) -> Result<()> {
    let resolver = ContextResolver::new();
    let ctx = resolver.resolve()?;

    if args.json {
        let explanations = ctx.explain();
        let output: Vec<_> = explanations
            .into_iter()
            .map(|(name, value, source)| {
                serde_json::json!({
                    "name": name,
                    "value": value,
                    "source": source
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{}", "Resolved Context".bold().underline());
    println!();

    for (name, value, source) in ctx.explain() {
        println!(
            "  {:<12} {} {}",
            format!("{name}:"),
            value.cyan(),
            format!("({source})").dimmed()
        );
    }

    Ok(())
}

fn set(args: SetArgs) -> Result<()> {
    use std::fs;
    use std::path::Path;

    let aeterna_dir = Path::new(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    let mut config = if context_file.exists() {
        let content = fs::read_to_string(&context_file)?;
        toml::from_str::<toml::Value>(&content)
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    if let Some(table) = config.as_table_mut() {
        table.insert(args.key.clone(), toml::Value::String(args.value.clone()));
    }

    fs::create_dir_all(aeterna_dir)?;
    fs::write(&context_file, toml::to_string_pretty(&config)?)?;

    println!(
        "{} Set {} = {}",
        "✓".green().bold(),
        args.key.cyan(),
        args.value.cyan()
    );

    Ok(())
}

fn clear(args: ClearArgs) -> Result<()> {
    use std::fs;
    use std::path::Path;

    let aeterna_dir = Path::new(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    if args.all {
        if aeterna_dir.exists() {
            fs::remove_dir_all(aeterna_dir)?;
            println!("{} Removed {}", "✓".green().bold(), aeterna_dir.display());
        } else {
            println!("{}", "No .aeterna directory found".dimmed());
        }
    } else if context_file.exists() {
        fs::remove_file(&context_file)?;
        println!("{} Removed {}", "✓".green().bold(), context_file.display());
    } else {
        println!("{}", "No context.toml found".dimmed());
    }

    Ok(())
}
