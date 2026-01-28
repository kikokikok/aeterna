use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::json;

use crate::output;
use crate::ux_error;

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
    Revoke(AgentRevokeArgs)
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
    pub dry_run: bool
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
    pub json: bool
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
    pub json: bool
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
    pub json: bool
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
    pub json: bool
}

pub async fn run(cmd: AgentCommand) -> anyhow::Result<()> {
    match cmd {
        AgentCommand::Register(args) => run_register(args).await,
        AgentCommand::List(args) => run_list(args).await,
        AgentCommand::Show(args) => run_show(args).await,
        AgentCommand::Permissions(args) => run_permissions(args).await,
        AgentCommand::Revoke(args) => run_revoke(args).await
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

    let err = ux_error::server_not_connected();
    err.display();
    output::info("Run with --dry-run to see what would be created.");

    Ok(())
}

async fn run_list(args: AgentListArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    if args.json {
        let output = json!({
            "operation": "agent_list",
            "filters": {
                "delegatedBy": args.delegated_by,
                "agentType": args.agent_type,
                "all": args.all,
            },
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Agents");
        println!();

        if args.all {
            output::info("Showing all agents you have access to.");
        }

        if let Some(ref user) = args.delegated_by {
            println!("  Filter: delegated_by = {user}");
        }
        if let Some(ref t) = args.agent_type {
            println!("  Filter: type = {t}");
        }
        println!();

        output::header("Example Output (would show)");
        println!("  AGENT ID            TYPE       DELEGATED BY         STATUS    LAST ACTIVE");
        println!("  agent-opencode-1234 opencode   alice@acme.com       active    2 min ago");
        println!("  agent-bot-5678      langchain  bob@acme.com         active    1 hour ago");
        println!("  agent-test-9012     custom     carol@acme.com       inactive  3 days ago");
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_show(args: AgentShowArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    if args.json {
        let output = json!({
            "operation": "agent_show",
            "agentId": args.agent_id,
            "verbose": args.verbose,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Agent: {}", args.agent_id));
        println!();

        output::header("Would Show");
        println!("  - Agent name and description");
        println!("  - Agent type (opencode, langchain, etc.)");
        println!("  - Delegating user");
        println!("  - Current permissions");
        println!("  - Status (active/inactive)");
        println!("  - Last activity");

        if args.verbose {
            println!();
            output::header("Verbose Details");
            println!("  - Full permission matrix");
            println!("  - Recent operations");
            println!("  - Memory access history");
            println!("  - Policy violations (if any)");
            println!("  - Audit trail");
        }
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_permissions(args: AgentPermissionsArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let valid_permissions = [
        "memory:read",
        "memory:write",
        "memory:delete",
        "knowledge:read",
        "knowledge:write",
        "policy:read",
        "policy:propose",
        "graph:read",
        "graph:write"
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

        if args.json {
            let output = json!({
                "operation": "agent_permission_grant",
                "agentId": args.agent_id,
                "permission": perm_to_grant,
                "scope": args.scope,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Grant Agent Permission");
            println!();
            println!("  Agent:      {}", args.agent_id);
            println!("  Permission: {perm_to_grant}");
            if let Some(ref scope) = args.scope {
                println!("  Scope:      {scope}");
            }
            println!();

            output::header("Would Do");
            println!("  1. Verify you can delegate this permission");
            println!("  2. Update agent's Cedar policies");
            println!("  3. Log audit event");
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if let Some(ref perm_to_revoke) = args.revoke {
        if args.json {
            let output = json!({
                "operation": "agent_permission_revoke",
                "agentId": args.agent_id,
                "permission": perm_to_revoke,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Revoke Agent Permission");
            println!();
            println!("  Agent:      {}", args.agent_id);
            println!("  Permission: {perm_to_revoke}");
            println!();

            output::header("Would Do");
            println!("  1. Remove permission from agent");
            println!("  2. Update Cedar policies");
            println!("  3. Log audit event");
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if args.json {
        let output = json!({
            "operation": "agent_permissions_list",
            "agentId": args.agent_id,
            "context": {
                "tenantId": resolved.tenant_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Permissions for: {}", args.agent_id));
        println!();

        output::header("Example Output (would show)");
        println!("  PERMISSION       SCOPE           GRANTED BY          DATE");
        println!("  memory:read      project         alice@acme.com      2024-06-01");
        println!("  memory:write     project         alice@acme.com      2024-06-01");
        println!("  knowledge:read   org             alice@acme.com      2024-06-01");
        println!("  policy:read      company         system              2024-06-01");
        println!();

        output::header("Available Permissions");
        println!("  memory:read     - Search and retrieve memories");
        println!("  memory:write    - Add new memories");
        println!("  memory:delete   - Delete memories (restricted)");
        println!("  knowledge:read  - Query knowledge repository");
        println!("  knowledge:write - Modify knowledge (restricted)");
        println!("  policy:read     - Check constraints");
        println!("  policy:propose  - Propose new policies");
        println!("  graph:read      - Query memory graph");
        println!("  graph:write     - Modify graph relationships");
        println!();

        output::header("Actions");
        println!(
            "  Grant:  aeterna agent permissions {} --grant <permission>",
            args.agent_id
        );
        println!(
            "  Revoke: aeterna agent permissions {} --revoke <permission>",
            args.agent_id
        );
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_revoke(args: AgentRevokeArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    if args.json {
        let output = json!({
            "operation": "agent_revoke",
            "agentId": args.agent_id,
            "force": args.force,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Revoke Agent: {}", args.agent_id));
        println!();

        if !args.force {
            output::warn("This will permanently revoke the agent's access.");
            println!();
        }

        output::header("Would Do");
        println!("  1. Verify your permission to revoke this agent");
        println!("  2. Invalidate all agent tokens");
        println!("  3. Remove all Cedar policies for this agent");
        println!("  4. Log audit event");
        println!("  5. Agent status set to 'revoked'");
        println!();

        output::header("Effect");
        println!("  - Agent can no longer access any Aeterna resources");
        println!("  - All active sessions will be terminated");
        println!("  - Historical data and audit trail preserved");
        println!();

        if !args.force {
            output::info("Add --force to proceed without confirmation.");
        }

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}
