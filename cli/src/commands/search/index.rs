//! # Code Search Index Command
//!
//! Trigger re-indexing for a repository.

use clap::Args;

#[derive(Args)]
pub struct IndexArgs {
    /// Repository name or ID
    #[arg(short, long)]
    pub repo: String,

    /// Perform incremental indexing (only changed files)
    #[arg(short, long)]
    pub incremental: bool,

    /// Force full re-index
    #[arg(short, long)]
    pub force: bool,

    /// Run indexing in the background (async)
    #[arg(long)]
    pub r#async: bool,
}

pub async fn handle(args: IndexArgs) -> anyhow::Result<()> {
    let _ = args;
    Err(super::legacy_codesearch_binary_removed("index"))
}
