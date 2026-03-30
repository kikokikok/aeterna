//! # Code Search Trace Command

use clap::{Args, Subcommand};

#[derive(Args)]
pub struct TraceArgs {
    #[command(subcommand)]
    pub command: TraceCommand,
}

#[derive(Subcommand)]
pub enum TraceCommand {
    #[command(about = "Find all functions that call a symbol")]
    Callers(CallersArgs),

    #[command(about = "Find all functions called by a symbol")]
    Callees(CalleesArgs),

    #[command(about = "Build full call graph for a symbol")]
    Graph(GraphArgs),
}

#[derive(Args)]
pub struct CallersArgs {
    /// Symbol name to trace (function, method, class)
    pub symbol: String,

    /// File path where symbol is defined (improves accuracy)
    #[arg(short, long)]
    pub file: Option<String>,

    /// Include indirect callers (recursive)
    #[arg(short, long)]
    pub recursive: bool,

    /// Maximum depth for recursive tracing
    #[arg(long, default_value = "3")]
    pub max_depth: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct CalleesArgs {
    /// Symbol name to trace
    pub symbol: String,

    /// File path where symbol is defined
    #[arg(short, long)]
    pub file: Option<String>,

    /// Include indirect callees (recursive)
    #[arg(short, long)]
    pub recursive: bool,

    /// Maximum depth for recursive tracing
    #[arg(long, default_value = "3")]
    pub max_depth: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GraphArgs {
    /// Symbol name to build graph around
    pub symbol: String,

    /// File path where symbol is defined
    #[arg(short, long)]
    pub file: Option<String>,

    /// Graph depth (1 = direct neighbors, 2 = neighbors of neighbors)
    #[arg(short, long, default_value = "2")]
    pub depth: usize,

    /// Include callers in graph
    #[arg(long, default_value = "true")]
    pub include_callers: bool,

    /// Include callees in graph
    #[arg(long, default_value = "true")]
    pub include_callees: bool,

    /// Output format (json, dot, mermaid)
    #[arg(long, default_value = "json")]
    pub format: String,
}

pub async fn handle(args: TraceArgs) -> anyhow::Result<()> {
    let _ = args;
    Err(super::legacy_codesearch_binary_removed("trace"))
}
