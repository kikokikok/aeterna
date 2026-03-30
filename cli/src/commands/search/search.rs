//! # Code Search Command

use clap::Args;

#[derive(Args)]
pub struct SearchArgs {
    /// Natural language search query
    pub query: String,

    /// Maximum number of results
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Minimum relevance score threshold (0.0-1.0)
    #[arg(short, long, default_value = "0.7")]
    pub threshold: f32,

    /// File path pattern filter (glob)
    #[arg(long)]
    pub file_pattern: Option<String>,

    /// Language filter (rust, python, go, etc.)
    #[arg(long)]
    pub language: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show only file paths (no content)
    #[arg(long)]
    pub files_only: bool,
}

pub async fn handle(args: SearchArgs) -> anyhow::Result<()> {
    let _ = args;
    Err(super::legacy_codesearch_binary_removed("search"))
}
