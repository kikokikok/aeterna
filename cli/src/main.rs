use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

mod commands;
mod output;
pub mod ux_error;

use commands::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => commands::init::run(args),
        Commands::Status(args) => commands::status::run(args),
        Commands::Sync(args) => commands::sync::run(args).await,
        Commands::Check(args) => commands::check::run(args).await,
        Commands::Context(args) => commands::context::run(args),
        Commands::Hints(args) => commands::hints::run(args),
        Commands::Memory(cmd) => commands::memory::run(cmd).await,
        Commands::Knowledge(cmd) => commands::knowledge::run(cmd).await,
        Commands::Policy(cmd) => commands::policy::run(cmd).await,
        Commands::Org(cmd) => commands::org::run(cmd).await,
        Commands::Team(cmd) => commands::team::run(cmd).await,
        Commands::User(cmd) => commands::user::run(cmd).await,
        Commands::Agent(cmd) => commands::agent::run(cmd).await,
        Commands::Govern(cmd) => commands::govern::run(cmd).await,
        Commands::Admin(cmd) => commands::admin::run(cmd).await,
        Commands::Completion(args) => commands::completion::run(args),
    }
}
