//! # Code Search Index Command
//!
//! Trigger re-indexing for a repository.

use crate::output;
use crate::ux_error;
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
    output::header("Code Search Indexing");

    let strategy = if args.incremental {
        "incremental"
    } else {
        "full"
    };
    println!("  Target Repository: {}", args.repo);
    println!("  Strategy:          {}", strategy);
    println!(
        "  Mode:              {}",
        if args.r#async {
            "Asynchronous"
        } else {
            "Synchronous"
        }
    );
    println!();

    output::info(&format!(
        "Triggering {} index for '{}'...",
        strategy, args.repo
    ));

    ux_error::server_not_connected().display();
    anyhow::bail!(
        "Code Search indexing is not available without a live Aeterna backend. \
         Set AETERNA_SERVER_URL and ensure the server is running."
    )
}
