//! # Code Search Init Command
//!
//! Initialize Code Search for a project directory.

use clap::Args;
use std::path::PathBuf;

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
    if !args.path.exists() {
        anyhow::bail!("Project path does not exist: {}", args.path.display());
    }
    let _ = args;
    Err(super::legacy_codesearch_binary_removed("init"))
}
