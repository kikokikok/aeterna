//! # GrepAI Init Command
//!
//! Initialize GrepAI for a project directory.

use clap::Args;
use std::path::PathBuf;
use std::process::Command;

#[derive(Args)]
pub struct InitArgs {
    /// Project directory to initialize (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Embedder provider (ollama or openai)
    #[arg(long, default_value = "ollama")]
    pub embedder: String,

    /// Embedding model (e.g., nomic-embed-text for ollama, text-embedding-3-small for openai)
    #[arg(long)]
    pub model: Option<String>,

    /// Vector store backend (qdrant, postgres, or gob)
    #[arg(long, default_value = "gob")]
    pub store: String,

    /// Qdrant URL (if using qdrant store)
    #[arg(long)]
    pub qdrant_url: Option<String>,

    /// PostgreSQL connection string (if using postgres store)
    #[arg(long)]
    pub postgres_url: Option<String>,

    /// Force re-initialization
    #[arg(short, long)]
    pub force: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn handle(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Verify project path exists
    if !args.path.exists() {
        return Err(format!("Project path does not exist: {}", args.path.display()).into());
    }

    // Check if GrepAI is installed
    let grepai_check = Command::new("grepai")
        .arg("--version")
        .output();

    match grepai_check {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            if !args.json {
                println!("✓ GrepAI found: {}", version.trim());
            }
        }
        _ => {
            return Err(
                "GrepAI not found. Please install GrepAI first:\n\
                 cargo install grepai\n\
                 or download from: https://github.com/greptileai/grepai"
                    .into(),
            );
        }
    }

    // Build grepai init command
    let mut cmd = Command::new("grepai");
    cmd.arg("init").arg(&args.path);

    // Add embedder configuration
    cmd.arg("--embedder").arg(&args.embedder);
    if let Some(model) = &args.model {
        cmd.arg("--model").arg(model);
    }

    // Add store configuration
    cmd.arg("--store").arg(&args.store);
    if let Some(url) = &args.qdrant_url {
        cmd.arg("--qdrant-url").arg(url);
    }
    if let Some(url) = &args.postgres_url {
        cmd.arg("--postgres-url").arg(url);
    }

    if args.force {
        cmd.arg("--force");
    }

    if !args.json {
        println!("Initializing GrepAI for project: {}", args.path.display());
        println!("Embedder: {} ({})", args.embedder, args.model.as_deref().unwrap_or("default"));
        println!("Store: {}", args.store);
    }

    // Execute grepai init
    let output = cmd.output()?;

    if output.status.success() {
        if args.json {
            println!(
                "{{\"success\": true, \"path\": \"{}\", \"message\": \"GrepAI initialized successfully\"}}",
                args.path.display()
            );
        } else {
            println!("✓ GrepAI initialized successfully!");
            println!("\nNext steps:");
            println!("  1. Run 'aeterna grepai status' to check indexing progress");
            println!("  2. Run 'aeterna grepai search \"your query\"' to search code");
            println!("  3. Run 'aeterna grepai trace callers <symbol>' for call graph analysis");
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("GrepAI initialization failed: {}", stderr).into())
    }
}
