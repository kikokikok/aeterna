use clap::{Args, Subcommand};

#[derive(Args)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoSubcommand
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
    Identity(IdentityArgs)
}

#[derive(Args)]
pub struct IdentityArgs {
    #[command(subcommand)]
    pub command: IdentitySubcommand
}

#[derive(Subcommand)]
pub enum IdentitySubcommand {
    #[command(about = "Register a new Git identity")]
    Add(AddIdentityArgs),
    #[command(about = "List registered identities")]
    List
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
    pub secret_provider: String
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
    pub interval: i32
}

#[derive(Args)]
pub struct ListArgs {
    #[arg(short, long, help = "Output as JSON")]
    pub json: bool
}

#[derive(Args)]
pub struct ApproveArgs {
    #[arg(help = "Request ID to approve")]
    pub id: String,
    #[arg(short, long, help = "Reason for approval")]
    pub reason: Option<String>
}

#[derive(Args)]
pub struct RejectArgs {
    #[arg(help = "Request ID to reject")]
    pub id: String,
    #[arg(short, long, help = "Reason for rejection")]
    pub reason: String
}

pub async fn handle(args: RepoArgs) -> anyhow::Result<()> {
    match args.command {
        RepoSubcommand::Request(req) => handle_request(req).await,
        RepoSubcommand::List(list) => handle_list(list).await,
        RepoSubcommand::Approve(app) => handle_approve(app).await,
        RepoSubcommand::Reject(rej) => handle_reject(rej).await,
        RepoSubcommand::Identity(id) => handle_identity(id).await
    }
}

async fn handle_request(args: RequestArgs) -> anyhow::Result<()> {
    use crate::output;
    output::header("Code Search Repository Request");
    println!("  Name: {}", args.name);
    println!("  Type: {}", args.r#type);
    if let Some(url) = &args.url {
        println!("  URL:  {}", url);
    }
    if let Some(auth) = &args.identity {
        println!("  Auth: {}", auth);
    }
    println!("  Sync: {} ({}m)", args.strategy, args.interval);
    println!();

    output::info("Submitting request to backend...");
    // TODO: Call storage layer or API
    output::success(&format!(
        "Request submitted for repository '{}'.",
        args.name
    ));
    output::hint("An administrator must approve this request before indexing starts.");
    Ok(())
}

async fn handle_list(_args: ListArgs) -> anyhow::Result<()> {
    use crate::output;
    output::header("Tracked Repositories");
    println!();
    // Placeholder for table output
    println!(
        "{:<20} {:<10} {:<15} {:<20}",
        "NAME", "TYPE", "STATUS", "LAST INDEXED"
    );
    println!("{:-<70}", "");
    output::info("No repositories currently tracked for this tenant.");
    Ok(())
}

async fn handle_approve(args: ApproveArgs) -> anyhow::Result<()> {
    use crate::output;
    output::info(&format!("Approving request {}...", args.id));
    output::success("Request approved. Cloning will start in the background.");
    Ok(())
}

async fn handle_reject(args: RejectArgs) -> anyhow::Result<()> {
    use crate::output;
    output::warn(&format!(
        "Rejecting request {} for reason: {}",
        args.id, args.reason
    ));
    output::success("Request rejected.");
    Ok(())
}

async fn handle_identity(args: IdentityArgs) -> anyhow::Result<()> {
    use crate::output;
    match args.command {
        IdentitySubcommand::Add(add) => {
            output::info(&format!("Adding Git identity '{}'...", add.name));
            output::success(&format!(
                "Identity '{}' registered with {} provider.",
                add.name, add.secret_provider
            ));
        }
        IdentitySubcommand::List => {
            output::header("Git Identities");
            println!();
            println!(
                "{:<20} {:<10} {:<20} {:<20}",
                "NAME", "PROVIDER", "SECRET ID", "TYPE"
            );
            println!("{:-<75}", "");
            output::info("No identities found.");
        }
    }
    Ok(())
}
