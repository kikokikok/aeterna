use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::output;
use crate::ux_error;

#[derive(Subcommand)]
pub enum TeamCommand {
    #[command(about = "Create a new team")]
    Create(TeamCreateArgs),

    #[command(about = "List teams in your organization")]
    List(TeamListArgs),

    #[command(about = "Show team details")]
    Show(TeamShowArgs),

    #[command(about = "Manage team members")]
    Members(TeamMembersArgs),

    #[command(about = "Set default team for current context")]
    Use(TeamUseArgs),
}

#[derive(Args)]
pub struct TeamCreateArgs {
    /// Team name
    pub name: String,

    /// Team description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Parent organization ID (auto-detected if not provided)
    #[arg(long)]
    pub org: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run - show what would be created
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TeamListArgs {
    /// Filter by organization
    #[arg(long)]
    pub org: Option<String>,

    /// Show all teams you have access to
    #[arg(long)]
    pub all: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TeamShowArgs {
    /// Team ID (uses current context if not provided)
    pub team_id: Option<String>,

    /// Show full details including policies and projects
    #[arg(short, long)]
    pub verbose: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TeamMembersArgs {
    /// Team ID (uses current context if not provided)
    #[arg(long)]
    pub team: Option<String>,

    /// Add member by user ID
    #[arg(long)]
    pub add: Option<String>,

    /// Remove member by user ID
    #[arg(long)]
    pub remove: Option<String>,

    /// Set role for a member
    #[arg(long, value_name = "USER_ID")]
    pub set_role: Option<String>,

    /// Role to assign (developer, techlead, architect)
    #[arg(long)]
    pub role: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TeamUseArgs {
    /// Team ID to set as default
    pub team_id: String,
}

pub async fn run(cmd: TeamCommand) -> anyhow::Result<()> {
    match cmd {
        TeamCommand::Create(args) => run_create(args).await,
        TeamCommand::List(args) => run_list(args).await,
        TeamCommand::Show(args) => run_show(args).await,
        TeamCommand::Members(args) => run_members(args).await,
        TeamCommand::Use(args) => run_use(args).await,
    }
}

fn team_server_required(operation: &str, message: &str) -> anyhow::Result<()> {
    ux_error::UxError::new(message)
        .why("This command requires a live control-plane backend")
        .fix("Start the Aeterna server: aeterna serve")
        .fix("Ensure AETERNA_SERVER_URL is set and the server is reachable")
        .suggest("aeterna admin health")
        .display();
    anyhow::bail!("Aeterna server not connected for operation: {operation}")
}

async fn get_live_client() -> Option<crate::client::AeternaClient> {
    crate::backend::connect()
        .await
        .ok()
        .map(|(client, _)| client)
}

fn normalize_team_role(role: &str, suggestion: &str) -> anyhow::Result<String> {
    let role = role.to_lowercase();
    let valid_roles = ["developer", "techlead", "architect"];
    if valid_roles.contains(&role.as_str()) {
        Ok(role)
    } else {
        ux_error::UxError::new(format!("Invalid team role: '{role}'"))
            .why("Team roles determine user permissions within the team")
            .fix("Use one of: developer, techlead, architect")
            .suggest(suggestion)
            .display();
        anyhow::bail!("Invalid role")
    }
}

fn resolve_current_team(explicit_team: Option<String>) -> anyhow::Result<String> {
    if let Some(team_id) = explicit_team {
        return Ok(team_id);
    }

    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;
    if let Some(team) = resolved.team_id.as_ref() {
        Ok(team.value.clone())
    } else {
        ux_error::UxError::new("No team specified")
            .why("This command needs a team ID or an active team context")
            .fix("Pass a team ID explicitly")
            .suggest("aeterna team show <team-id>")
            .display();
        anyhow::bail!("No team specified")
    }
}

fn set_context_value(key: &str, value: &str) -> anyhow::Result<()> {
    let aeterna_dir = Path::new(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    let mut config = if context_file.exists() {
        let content = fs::read_to_string(&context_file)?;
        toml::from_str::<toml::Value>(&content)
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    if let Some(table) = config.as_table_mut() {
        table.insert(key.to_string(), toml::Value::String(value.to_string()));
    }

    fs::create_dir_all(aeterna_dir)?;
    fs::write(&context_file, toml::to_string_pretty(&config)?)?;
    Ok(())
}

fn get_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(found) = value.get(*key).and_then(|v| v.as_str()) {
            return Some(found);
        }
    }
    None
}

async fn run_create(args: TeamCreateArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let org_id = args.org.clone().unwrap_or_else(|| {
        resolved
            .org_id
            .as_ref()
            .map_or_else(|| "default-org".to_string(), |o| o.value.clone())
    });

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "team_create",
                "team": {
                    "name": args.name,
                    "description": args.description,
                    "orgId": org_id,
                },
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Team Create (Dry Run)");
            println!();
            println!("  Name:         {}", args.name);
            if let Some(ref desc) = args.description {
                println!("  Description:  {desc}");
            }
            println!("  Organization: {org_id}");
            println!();
            output::info("Dry run mode - team not created.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let body = json!({
            "name": args.name,
            "description": args.description,
            "orgId": org_id,
        });
        let result = client.team_create(&body).await.inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Team Created");
            println!();
            println!(
                "  ID:           {}",
                get_str(&result, &["id"]).unwrap_or("?")
            );
            println!(
                "  Name:         {}",
                get_str(&result, &["name"]).unwrap_or("?")
            );
            println!(
                "  Organization: {}",
                get_str(&result, &["parentId", "parent_id"]).unwrap_or("?")
            );
            if let Some(description) = result
                .get("metadata")
                .and_then(|v| v.get("description"))
                .and_then(|v| v.as_str())
            {
                println!("  Description:  {description}");
            }
            println!();
            output::hint("Add members with 'aeterna team members --add <user-id> --role <role>'");
        }
        return Ok(());
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "success": false,
                "error": "server_not_connected",
                "operation": "team_create"
            }))?
        );
        anyhow::bail!("Aeterna server not connected for operation: team_create");
    }

    team_server_required("team_create", "Cannot create team: server not connected")
}

async fn run_list(args: TeamListArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client
            .team_list(args.org.as_deref(), args.all)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Teams");
            println!();
            if let Some(teams) = result.as_array() {
                if teams.is_empty() {
                    println!("  (no teams found)");
                } else {
                    for team in teams {
                        let id = get_str(team, &["id"]).unwrap_or("?");
                        let name = get_str(team, &["name"]).unwrap_or("?");
                        let org = get_str(team, &["parentId", "parent_id"]).unwrap_or("-");
                        println!("  {id:<24} {name:<32} {org}");
                    }
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "success": false,
                "error": "server_not_connected",
                "operation": "team_list"
            }))?
        );
        anyhow::bail!("Aeterna server not connected for operation: team_list");
    }

    team_server_required("team_list", "Cannot list teams: server not connected")
}

async fn run_show(args: TeamShowArgs) -> anyhow::Result<()> {
    let team_id = resolve_current_team(args.team_id.clone())?;

    if let Some(client) = get_live_client().await {
        let result = client.team_show(&team_id).await.inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Team: {team_id}"));
            println!();
            println!(
                "  ID:           {}",
                get_str(&result, &["id"]).unwrap_or("?")
            );
            println!(
                "  Name:         {}",
                get_str(&result, &["name"]).unwrap_or("?")
            );
            println!(
                "  Organization: {}",
                get_str(&result, &["parentId", "parent_id"]).unwrap_or("?")
            );
            println!(
                "  Type:         {}",
                get_str(&result, &["unitType", "unit_type"]).unwrap_or("team")
            );
            if let Some(description) = result
                .get("metadata")
                .and_then(|v| v.get("description"))
                .and_then(|v| v.as_str())
            {
                println!("  Description:  {description}");
            }
            if args.verbose {
                if let Some(source_owner) = get_str(&result, &["sourceOwner", "source_owner"]) {
                    println!("  Source:       {source_owner}");
                }
                if let Some(metadata) = result.get("metadata") {
                    println!("  Metadata: {}", serde_json::to_string_pretty(metadata)?);
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "success": false,
                "error": "server_not_connected",
                "operation": "team_show",
                "teamId": team_id
            }))?
        );
        anyhow::bail!("Aeterna server not connected for operation: team_show");
    }

    team_server_required(
        "team_show",
        &format!("Cannot show team '{team_id}': server not connected"),
    )
}

async fn run_members(args: TeamMembersArgs) -> anyhow::Result<()> {
    let team_id = resolve_current_team(args.team.clone())?;

    if let Some(ref user_to_add) = args.add {
        let role = normalize_team_role(
            args.role.as_deref().unwrap_or("developer"),
            &format!("aeterna team members --add {user_to_add} --role developer"),
        )?;

        if let Some(client) = get_live_client().await {
            let result = client
                .team_member_add(&team_id, &json!({"userId": user_to_add, "role": role}))
                .await
                .inspect_err(|e| {
                    if args.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &json!({"success": false, "error": e.to_string()})
                            )
                            .unwrap()
                        );
                    } else {
                        ux_error::UxError::new(e.to_string())
                            .fix("Run: aeterna auth login")
                            .display();
                    }
                })?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                output::header("Team Member Added");
                println!();
                println!("  Team: {team_id}");
                println!(
                    "  User: {}",
                    get_str(&result, &["userId", "user_id"]).unwrap_or(user_to_add)
                );
                println!(
                    "  Role: {}",
                    get_str(&result, &["role"]).unwrap_or(role.as_str())
                );
                println!();
            }
            return Ok(());
        }

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "success": false,
                    "error": "server_not_connected",
                    "operation": "team_member_add",
                    "teamId": team_id,
                    "userId": user_to_add
                }))?
            );
            anyhow::bail!("Aeterna server not connected for operation: team_member_add");
        }

        return team_server_required(
            "team_member_add",
            &format!("Cannot add member to team '{team_id}': server not connected"),
        );
    }

    if let Some(ref user_to_remove) = args.remove {
        if let Some(client) = get_live_client().await {
            let result = client
                .team_member_remove(&team_id, user_to_remove)
                .await
                .inspect_err(|e| {
                    if args.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &json!({"success": false, "error": e.to_string()})
                            )
                            .unwrap()
                        );
                    } else {
                        ux_error::UxError::new(e.to_string())
                            .fix("Run: aeterna auth login")
                            .display();
                    }
                })?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                output::header("Team Member Removed");
                println!();
                println!("  Team: {team_id}");
                println!("  User: {user_to_remove}");
                println!();
            }
            return Ok(());
        }

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "success": false,
                    "error": "server_not_connected",
                    "operation": "team_member_remove",
                    "teamId": team_id,
                    "userId": user_to_remove
                }))?
            );
            anyhow::bail!("Aeterna server not connected for operation: team_member_remove");
        }

        return team_server_required(
            "team_member_remove",
            &format!("Cannot remove member from team '{team_id}': server not connected"),
        );
    }

    if let Some(ref user_id) = args.set_role {
        let role_value = args.role.clone().ok_or_else(|| {
            ux_error::UxError::new("Missing --role for --set-role")
                .why("Must specify which role to assign")
                .fix("Add --role with the desired role")
                .suggest(format!(
                    "aeterna team members --set-role {user_id} --role techlead"
                ))
                .display();
            anyhow::anyhow!("Missing role")
        })?;
        let role = normalize_team_role(
            &role_value,
            &format!("aeterna team members --set-role {user_id} --role techlead"),
        )?;

        if let Some(client) = get_live_client().await {
            let result = client
                .team_member_set_role(&team_id, user_id, &json!({"role": role}))
                .await
                .inspect_err(|e| {
                    if args.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &json!({"success": false, "error": e.to_string()})
                            )
                            .unwrap()
                        );
                    } else {
                        ux_error::UxError::new(e.to_string())
                            .fix("Run: aeterna auth login")
                            .display();
                    }
                })?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                output::header("Team Member Role Updated");
                println!();
                println!("  Team:     {team_id}");
                println!(
                    "  User:     {}",
                    get_str(&result, &["userId", "user_id"]).unwrap_or(user_id)
                );
                println!(
                    "  New Role: {}",
                    get_str(&result, &["role"]).unwrap_or(role.as_str())
                );
                println!();
            }
            return Ok(());
        }

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "success": false,
                    "error": "server_not_connected",
                    "operation": "team_member_set_role",
                    "teamId": team_id,
                    "userId": user_id
                }))?
            );
            anyhow::bail!("Aeterna server not connected for operation: team_member_set_role");
        }

        return team_server_required(
            "team_member_set_role",
            &format!("Cannot update member role in team '{team_id}': server not connected"),
        );
    }

    if let Some(client) = get_live_client().await {
        let result = client.team_members_list(&team_id).await.inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Members of: {team_id}"));
            println!();
            if let Some(members) = result.as_array() {
                if members.is_empty() {
                    println!("  (no members found)");
                } else {
                    for member in members {
                        let user = get_str(member, &["userId", "user_id"]).unwrap_or("?");
                        let role = get_str(member, &["role"]).unwrap_or("?");
                        println!("  {user:<36} {role}");
                    }
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            println!();
            output::hint("Add member: aeterna team members --add <user> --role <role>");
            output::hint("Change role: aeterna team members --set-role <user> --role <role>");
        }
        return Ok(());
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "success": false,
                "error": "server_not_connected",
                "operation": "team_members_list",
                "teamId": team_id
            }))?
        );
        anyhow::bail!("Aeterna server not connected for operation: team_members_list");
    }

    team_server_required(
        "team_members_list",
        &format!("Cannot list members for team '{team_id}': server not connected"),
    )
}

async fn run_use(args: TeamUseArgs) -> anyhow::Result<()> {
    set_context_value("team_id", &args.team_id)?;

    output::header("Set Default Team");
    println!();
    println!("  Setting default team: {}", args.team_id);
    println!();
    println!("  ✓ Updated .aeterna/context.toml");
    println!("  team_id = \"{}\"", args.team_id);

    Ok(())
}
