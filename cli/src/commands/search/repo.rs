use clap::{Args, Subcommand};

#[derive(Args)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoSubcommand,
}

#[derive(Subcommand)]
pub enum RepoSubcommand {
    #[command(about = "Request indexing for a new repository")]
    Request(RequestArgs),
    #[command(about = "List tracked repositories")]
    List(ListArgs),
    #[command(about = "Approve a repository request")]
    Approve(ApproveArgs),
    #[command(about = "Reject a repository request")]
    Reject(RejectArgs),
    #[command(about = "Manage Git identities")]
    Identity(IdentityArgs),
}

#[derive(Args)]
pub struct IdentityArgs {
    #[command(subcommand)]
    pub command: IdentitySubcommand,
}

#[derive(Subcommand)]
pub enum IdentitySubcommand {
    #[command(about = "Register a new Git identity")]
    Add(AddIdentityArgs),
    #[command(about = "List registered identities")]
    List,
}

#[derive(Args)]
pub struct AddIdentityArgs {
    #[arg(short, long, help = "Name for the identity (e.g., 'primary-gh')")]
    pub name: String,
    #[arg(short, long, help = "Provider (github, gitlab)")]
    pub provider: String,
    #[arg(short = 'i', long, help = "Secret ID in cloud vault")]
    pub secret_id: String,
    #[arg(
        short = 'P',
        long,
        help = "Secret provider (aws-secrets, vault)",
        default_value = "aws-secrets"
    )]
    pub secret_provider: String,
}

#[derive(Args)]
pub struct RequestArgs {
    #[arg(short, long, help = "Name for the repository")]
    pub name: String,
    #[arg(short, long, help = "Repository type (local, remote, hybrid)")]
    pub r#type: String,
    #[arg(short, long, help = "Remote URL (for remote/hybrid)")]
    pub url: Option<String>,
    #[arg(short, long, help = "Local path (for local/hybrid)")]
    pub path: Option<String>,
    #[arg(short, long, help = "Identity ID to use for auth")]
    pub identity: Option<String>,
    #[arg(
        short,
        long,
        help = "Strategy (hook, job, manual)",
        default_value = "manual"
    )]
    pub strategy: String,
    #[arg(
        short = 'm',
        long,
        help = "Sync interval in minutes",
        default_value = "15"
    )]
    pub interval: i32,
}

#[derive(Args)]
pub struct ListArgs {
    #[arg(short, long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Args)]
pub struct ApproveArgs {
    #[arg(help = "Request ID to approve")]
    pub id: String,
    #[arg(short, long, help = "Reason for approval")]
    pub reason: Option<String>,
}

#[derive(Args)]
pub struct RejectArgs {
    #[arg(help = "Request ID to reject")]
    pub id: String,
    #[arg(short, long, help = "Reason for rejection")]
    pub reason: String,
}

pub async fn handle(args: RepoArgs) -> anyhow::Result<()> {
    let _ = args;
    Err(super::legacy_codesearch_binary_removed("repo"))
}
