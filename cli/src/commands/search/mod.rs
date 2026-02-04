//! # Code Search CLI Commands
//!
//! CLI commands for code search and call graph analysis.

pub mod init;
pub mod search;
pub mod status;
pub mod trace;
pub mod repo;
pub mod index;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum CodeSearchCommand {
    #[command(about = "Initialize code search for a project directory")]
    Init(init::InitArgs),

    #[command(about = "Search code using semantic queries")]
    Search(search::SearchArgs),

    #[command(about = "Trace function callers or callees")]
    Trace(trace::TraceArgs),

    #[command(about = "Show indexing status")]
    Status(status::StatusArgs),

    #[command(about = "Manage repositories")]
    Repo(repo::RepoArgs),

    #[command(about = "Trigger repository re-indexing")]
    Index(index::IndexArgs),
}

pub async fn handle_command(cmd: CodeSearchCommand) -> anyhow::Result<()> {
    match cmd {
        CodeSearchCommand::Init(args) => init::handle(args).await,
        CodeSearchCommand::Search(args) => search::handle(args).await,
        CodeSearchCommand::Trace(args) => trace::handle(args).await,
        CodeSearchCommand::Status(args) => status::handle(args).await,
        CodeSearchCommand::Repo(args) => repo::handle(args).await,
        CodeSearchCommand::Index(args) => index::handle(args).await,
    }
}
