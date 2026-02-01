pub mod admin;
pub mod agent;
pub mod check;
pub mod completion;
pub mod context;
pub mod govern;
pub mod grepai;
pub mod hints;
pub mod init;
pub mod knowledge;
pub mod memory;
pub mod org;
pub mod policy;
pub mod setup;
pub mod status;
pub mod sync;
pub mod team;
pub mod user;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "aeterna",
    author,
    version,
    about = "Aeterna - Universal Memory & Knowledge Framework",
    long_about = "A breeze to setup. Sensible defaults for everything.\n\nCommands work without \
                  configuration - just run them.\nContext is auto-detected from git, env vars, or \
                  .aeterna/context.toml"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Initialize Aeterna in current directory")]
    Init(init::InitArgs),

    #[command(about = "Show current context and system status")]
    Status(status::StatusArgs),

    #[command(about = "Sync memory and knowledge systems")]
    Sync(sync::SyncArgs),

    #[command(about = "Run constraint validation checks")]
    Check(check::CheckArgs),

    #[command(subcommand, about = "Manage context (tenant, user, project, etc.)")]
    Context(context::ContextCommand),

    #[command(subcommand, about = "Manage operation hints (presets, toggles)")]
    Hints(hints::HintsCommand),

    #[command(subcommand, about = "Search, add, and manage memories")]
    Memory(memory::MemoryCommand),

    #[command(subcommand, about = "Search, get, and check knowledge")]
    Knowledge(knowledge::KnowledgeCommand),

    #[command(subcommand, about = "Create, validate, and manage policies")]
    Policy(policy::PolicyCommand),

    #[command(subcommand, about = "Manage organizations")]
    Org(org::OrgCommand),

    #[command(subcommand, about = "Manage teams")]
    Team(team::TeamCommand),

    #[command(subcommand, about = "Manage users and roles")]
    User(user::UserCommand),

    #[command(subcommand, about = "Manage AI agents and permissions")]
    Agent(agent::AgentCommand),

    #[command(subcommand, about = "Governance workflow (approve, reject, audit)")]
    Govern(govern::GovernCommand),

    #[command(subcommand, about = "System administration (health, migrate, export)")]
    Admin(admin::AdminCommand),

    #[command(subcommand, about = "Semantic code search and call graph analysis (GrepAI)")]
    GrepAI(grepai::GrepAICommand),

    #[command(about = "Generate shell completions")]
    Completion(completion::CompletionArgs),

    #[command(about = "Interactive setup wizard for deployment configuration")]
    Setup(setup::SetupArgs)
}
