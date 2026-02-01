//! # GrepAI CLI Commands
//!
//! CLI commands for GrepAI semantic code search and call graph analysis.

pub mod init;
pub mod search;
pub mod status;
pub mod trace;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum GrepAICommand {
    #[command(about = "Initialize GrepAI for a project directory")]
    Init(init::InitArgs),

    #[command(about = "Search code using semantic queries")]
    Search(search::SearchArgs),

    #[command(about = "Trace function callers or callees")]
    Trace(trace::TraceArgs),

    #[command(about = "Show GrepAI indexing status")]
    Status(status::StatusArgs),
}

pub async fn handle_command(cmd: GrepAICommand) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        GrepAICommand::Init(args) => init::handle(args).await,
        GrepAICommand::Search(args) => search::handle(args).await,
        GrepAICommand::Trace(args) => trace::handle(args).await,
        GrepAICommand::Status(args) => status::handle(args).await,
    }
}
