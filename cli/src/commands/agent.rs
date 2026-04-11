use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::json;

use crate::output;
use crate::ux_error;

async fn get_live_client() -> Option<crate::client::AeternaClient> {
    crate::backend::connect()
        .await
        .ok()
        .map(|(client, _)| client)
}

fn agent_server_required(operation: &str, message: &str) -> anyhow::Result<()> {
    ux_error::UxError::new(message)
        .why("This command requires a live control-plane backend")
        .fix("Start the Aeterna server: aeterna serve")
        .fix("Ensure AETERNA_SERVER_URL is set or a profile is configured and reachable")
        .suggest("aeterna auth login")
        .display();
    anyhow::bail!("Aeterna server not connected for operation: {operation}")
}

#[derive(Subcommand)]
pub enum AgentCommand {
    #[command(about = "Register an AI agent")]
    Register(AgentRegisterArgs),

    #[command(about = "List registered agents")]
    List(AgentListArgs),

    #[command(about = "Show agent details")]
    Show(AgentShowArgs),

    #[command(about = "Manage agent permissions")]
    Permissions(AgentPermissionsArgs),

    #[command(about = "Revoke an agent's access")]
    Revoke(AgentRevokeArgs),
}

#[derive(Args)]
pub struct AgentRegisterArgs {
    /// Agent name/identifier
    pub name: String,

    /// Agent description
    #[arg(short, long)]
    pub description: Option<String>,

    /// User who delegates permissions to this agent
    #[arg(long)]
    pub delegated_by: Option<String>,

    /// Agent type (opencode, langchain, autogen, custom)
    #[arg(long, default_value = "custom")]
    pub agent_type: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run - show what would be created
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct AgentListArgs {
    /// Filter by delegating user
    #[arg(long)]
    pub delegated_by: Option<String>,

    /// Filter by agent type
    #[arg(long)]
    pub agent_type: Option<String>,

    /// Show all agents you can see
    #[arg(long)]
    pub all: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct AgentShowArgs {
    /// Agent ID to show
    pub agent_id: String,

    /// Show full details including audit trail
    #[arg(short, long)]
    pub verbose: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct AgentPermissionsArgs {
    /// Agent ID
    pub agent_id: String,

    /// Grant permission
    #[arg(long)]
    pub grant: Option<String>,

    /// Revoke permission
    #[arg(long)]
    pub revoke: Option<String>,

    /// List current permissions
    #[arg(short, long)]
    pub list: bool,

    /// Scope for permission (memory-read, memory-write, knowledge-read,
    /// policy-read)
    #[arg(long)]
    pub scope: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct AgentRevokeArgs {
    /// Agent ID to revoke
    pub agent_id: String,

    /// Force revocation without confirmation
    #[arg(short, long)]
    pub force: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(cmd: AgentCommand) -> anyhow::Result<()> {
    match cmd {
        AgentCommand::Register(args) => run_register(args).await,
        AgentCommand::List(args) => run_list(args).await,
        AgentCommand::Show(args) => run_show(args).await,
        AgentCommand::Permissions(args) => run_permissions(args).await,
        AgentCommand::Revoke(args) => run_revoke(args).await,
    }
}

async fn run_register(args: AgentRegisterArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let delegated_by = args
        .delegated_by
        .clone()
        .unwrap_or_else(|| resolved.user_id.value.clone());

    let valid_types = ["opencode", "langchain", "autogen", "crewai", "custom"];
    if !valid_types.contains(&args.agent_type.to_lowercase().as_str()) {
        let err = ux_error::UxError::new(format!("Invalid agent type: '{}'", args.agent_type))
            .why("Agent type helps categorize and apply appropriate defaults")
            .fix("Use one of: opencode, langchain, autogen, crewai, custom")
            .suggest(format!(
                "aeterna agent register {} --agent-type opencode",
                args.name
            ));
        err.display();
        return Err(anyhow::anyhow!("Invalid agent type"));
    }

    let agent_id = format!(
        "agent-{}-{}",
        args.name.to_lowercase().replace(' ', "-"),
        chrono::Utc::now().timestamp() % 10000
    );

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "agent_register",
                "agent": {
                    "id": agent_id,
                    "name": args.name,
                    "description": args.description,
                    "type": args.agent_type,
                    "delegatedBy": delegated_by,
                },
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                },
                "nextSteps": [
                    "Review agent configuration",
                    "Run without --dry-run to register",
                    "Configure permissions with 'aeterna agent permissions'"
                ]
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Agent Registration (Dry Run)");
            println!();
            println!("  Agent ID:     {agent_id}");
            println!("  Name:         {}", args.name);
            println!("  Type:         {}", args.agent_type);
            println!("  Delegated By: {delegated_by}");
            if let Some(ref desc) = args.description {
                println!("  Description:  {desc}");
            }
            println!();

            output::header("What Would Happen");
            println!("  1. Create agent identity '{agent_id}'");
            println!("  2. Delegate permissions from '{delegated_by}' to agent");
            println!("  3. Generate Cedar policies for agent authorization");
            println!("  4. Agent inherits user's permissions (scoped down)");
            println!();

            output::header("Default Permissions (inherited from delegating user)");
            println!("  - memory:read    - Search and retrieve memories");
            println!("  - memory:write   - Add new memories");
            println!("  - knowledge:read - Query knowledge repository");
            println!("  - policy:read    - Check constraints (no create/modify)");
            println!();

            output::header("Next Steps");
            println!("  1. Run without --dry-run to register the agent");
            println!(
                "  2. Customize permissions: aeterna agent permissions {agent_id} --grant <perm>"
            );
            println!("  3. Use in your application with agent_id=\"{agent_id}\"");
            println!();

            output::info("Dry run mode - agent not registered.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let body = json!({
            "id": agent_id,
            "name": args.name,
            "description": args.description,
            "type": args.agent_type,
            "delegatedBy": delegated_by,
        });
        let result = client.agent_register(&body).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Agent Registered");
            println!();
            println!("  ID:   {}", result["id"].as_str().unwrap_or("?"));
            println!("  Name: {}", result["name"].as_str().unwrap_or("?"));
            println!("  Type: {}", result["type"].as_str().unwrap_or("?"));
            println!();
        }
        return Ok(());
    }

    agent_server_required(
        "agent_register",
        "Cannot register agent: server not connected",
    )
}

async fn run_list(args: AgentListArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client
            .agent_list(
                args.delegated_by.as_deref(),
                args.agent_type.as_deref(),
                args.all,
            )
            .await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Agents");
            println!();
            if let Some(items) = result.as_array() {
                if items.is_empty() {
                    println!("  (no agents found)");
                } else {
                    for item in items {
                        println!(
                            "  {:<28} {:<12} {:<22} {}",
                            item["id"].as_str().unwrap_or("?"),
                            item["type"].as_str().unwrap_or("?"),
                            item["delegatedBy"]
                                .as_str()
                                .or_else(|| item["delegated_by"].as_str())
                                .unwrap_or("?"),
                            item["status"].as_str().unwrap_or("?")
                        );
                    }
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            println!();
        }
        return Ok(());
    }

    agent_server_required("agent_list", "Cannot list agents: server not connected")
}

async fn run_show(args: AgentShowArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client.agent_show(&args.agent_id).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Agent: {}", args.agent_id));
            println!();
            println!("  ID:          {}", result["id"].as_str().unwrap_or("?"));
            println!("  Name:        {}", result["name"].as_str().unwrap_or("?"));
            println!("  Type:        {}", result["type"].as_str().unwrap_or("?"));
            println!(
                "  DelegatedBy: {}",
                result["delegatedBy"]
                    .as_str()
                    .or_else(|| result["delegated_by"].as_str())
                    .unwrap_or("?")
            );
            println!(
                "  Status:      {}",
                result["status"].as_str().unwrap_or("?")
            );
            if args.verbose {
                println!();
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            println!();
        }
        return Ok(());
    }

    agent_server_required("agent_show", "Cannot show agent: server not connected")
}

async fn run_permissions(args: AgentPermissionsArgs) -> anyhow::Result<()> {
    let valid_permissions = [
        "memory:read",
        "memory:write",
        "memory:delete",
        "knowledge:read",
        "knowledge:write",
        "policy:read",
        "policy:propose",
        "graph:read",
        "graph:write",
    ];

    if let Some(ref perm_to_grant) = args.grant {
        if !valid_permissions.contains(&perm_to_grant.as_str()) {
            let err = ux_error::UxError::new(format!("Invalid permission: '{perm_to_grant}'"))
                .why("Permission must be a valid agent capability")
                .fix(format!("Use one of: {}", valid_permissions.join(", ")))
                .suggest(format!(
                    "aeterna agent permissions {} --grant memory:read",
                    args.agent_id
                ));
            err.display();
            return Err(anyhow::anyhow!("Invalid permission"));
        }

        if let Some(client) = get_live_client().await {
            let result = client
                .agent_permission_grant(
                    &args.agent_id,
                    &json!({"permission": perm_to_grant, "scope": args.scope}),
                )
                .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                output::header("Grant Agent Permission");
                println!();
                println!("  Agent:      {}", args.agent_id);
                println!("  Permission: {perm_to_grant}");
                if let Some(ref scope) = args.scope {
                    println!("  Scope:      {scope}");
                }
                println!();
            }
            return Ok(());
        }
        return agent_server_required(
            "agent_permission_grant",
            "Cannot grant agent permission: server not connected",
        );
    }

    if let Some(ref perm_to_revoke) = args.revoke {
        if let Some(client) = get_live_client().await {
            let result = client
                .agent_permission_revoke(&args.agent_id, perm_to_revoke)
                .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                output::header("Revoke Agent Permission");
                println!();
                println!("  Agent:      {}", args.agent_id);
                println!("  Permission: {perm_to_revoke}");
                println!();
            }
            return Ok(());
        }
        return agent_server_required(
            "agent_permission_revoke",
            "Cannot revoke agent permission: server not connected",
        );
    }

    if let Some(client) = get_live_client().await {
        let result = client.agent_permissions_list(&args.agent_id).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Permissions for: {}", args.agent_id));
            println!();
            if let Some(items) = result.as_array() {
                for item in items {
                    println!(
                        "  {:<20} {:<15} {}",
                        item["permission"].as_str().unwrap_or("?"),
                        item["scope"].as_str().unwrap_or("?"),
                        item["grantedBy"]
                            .as_str()
                            .or_else(|| item["granted_by"].as_str())
                            .unwrap_or("?")
                    );
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            println!();
        }
        return Ok(());
    }

    agent_server_required(
        "agent_permissions_list",
        "Cannot list agent permissions: server not connected",
    )
}

async fn run_revoke(args: AgentRevokeArgs) -> anyhow::Result<()> {
    if !args.force {
        output::warn("This will permanently revoke the agent's access.");
        output::info("Add --force to proceed without confirmation.");
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let result = client.agent_revoke(&args.agent_id).await?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Revoked Agent: {}", args.agent_id));
            println!();
            println!(
                "  Status: {}",
                result["status"].as_str().unwrap_or("revoked")
            );
            println!();
        }
        return Ok(());
    }

    agent_server_required("agent_revoke", "Cannot revoke agent: server not connected")
}
