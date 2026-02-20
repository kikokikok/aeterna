//! # Code Search Init Command
//!
//! Initialize Code Search for a project directory.

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

pub async fn handle(args: InitArgs) -> anyhow::Result<()> {
    // Verify project path exists
    if !args.path.exists() {
        anyhow::bail!("Project path does not exist: {}", args.path.display());
    }

    // Check if Code Search backend is installed
    let codesearch_check = Command::new("codesearch").arg("--version").output();

    match codesearch_check {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            if !args.json {
                println!("✓ Code Search backend found: {}", version.trim());
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Code Search backend not found. Please install the required components first."
            ));
        }
    }

    // Build init command
    let mut cmd = Command::new("codesearch");
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
        println!(
            "Initializing Code Search for project: {}",
            args.path.display()
        );
        println!(
            "Embedder: {} ({})",
            args.embedder,
            args.model.as_deref().unwrap_or("default")
        );
        println!("Store: {}", args.store);
    }

    // Execute init
    let output = cmd.output()?;

    if output.status.success() {
        if args.json {
            println!(
                "{{\"success\": true, \"path\": \"{}\", \"message\": \"Code Search initialized successfully\"}}",
                args.path.display()
            );
        } else {
            println!("✓ Code Search initialized successfully!");
            println!("\nNext steps:");
            println!("  1. Run 'aeterna code-search status' to check indexing progress");
            println!("  2. Run 'aeterna code-search search \"your query\"' to search code");
            println!(
                "  3. Run 'aeterna code-search trace callers <symbol>' for call graph analysis"
            );
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Code Search initialization failed: {}",
            stderr
        ))
    }
}
