//! # Code Search Status Command

use clap::Args;

#[derive(Args)]
pub struct StatusArgs {
    /// Project name/path (optional, shows all if not specified)
    #[arg(short, long)]
    pub project: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Watch mode - continuously update status
    #[arg(short, long)]
    pub watch: bool,
}

pub async fn handle(args: StatusArgs) -> anyhow::Result<()> {
    let _ = args;
    Err(super::legacy_codesearch_binary_removed("status"))
}
