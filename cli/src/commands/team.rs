use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::json;

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
    Use(TeamUseArgs)
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
    pub dry_run: bool
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
    pub json: bool
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
    pub json: bool
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
    pub json: bool
}

#[derive(Args)]
pub struct TeamUseArgs {
    /// Team ID to set as default
    pub team_id: String
}

pub async fn run(cmd: TeamCommand) -> anyhow::Result<()> {
    match cmd {
        TeamCommand::Create(args) => run_create(args).await,
        TeamCommand::List(args) => run_list(args).await,
        TeamCommand::Show(args) => run_show(args).await,
        TeamCommand::Members(args) => run_members(args).await,
        TeamCommand::Use(args) => run_use(args).await
    }
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
                },
                "nextSteps": [
                    "Review team configuration",
                    "Run without --dry-run to create",
                    "Add members with 'aeterna team members --add <user>'",
                    "Create projects with 'aeterna project create <name> --team <team-id>'"
                ]
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

            output::header("What Would Happen");
            println!(
                "  1. Create team '{}' under organization '{}'",
                args.name, org_id
            );
            println!("  2. Add you ({}) as team lead", resolved.user_id.value);
            println!("  3. Inherit organization-level policies");
            println!();

            output::header("Next Steps");
            println!("  1. Run without --dry-run to create the team");
            println!("  2. Add members: aeterna team members --add <user-id>");
            println!(
                "  3. Create projects: aeterna project create <name> --team {}",
                args.name
            );
            println!();

            output::info("Dry run mode - team not created.");
        }
        return Ok(());
    }

    let err = ux_error::server_not_connected();
    err.display();
    output::info("Run with --dry-run to see what would be created.");

    Ok(())
}

async fn run_list(args: TeamListArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    if args.json {
        let output = json!({
            "operation": "team_list",
            "filters": {
                "org": args.org,
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
        output::header("Teams");
        println!();

        if args.all {
            output::info("Showing all teams you have access to.");
        }

        if let Some(ref org) = args.org {
            println!("  Filter: org = {org}");
        }
        println!();

        output::header("Example Output (would show)");
        println!("  ID            NAME          ORG              PROJECTS  MEMBERS  ROLE");
        println!("  api-team      API Team      platform-eng        3         5    techlead");
        println!("  web-team      Web Team      product-eng         2         4    member");
        println!("  data-team     Data Team     platform-eng        2         3    member");
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_show(args: TeamShowArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let team_id = args.team_id.clone().unwrap_or_else(|| {
        resolved
            .team_id
            .as_ref()
            .map_or_else(|| "current".to_string(), |t| t.value.clone())
    });

    if args.json {
        let output = json!({
            "operation": "team_show",
            "teamId": team_id,
            "verbose": args.verbose,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Team: {team_id}"));
        println!();

        output::header("Would Show");
        println!("  - Name and description");
        println!("  - Parent organization");
        println!("  - Projects count");
        println!("  - Members count and roles");
        println!("  - Your role in this team");

        if args.verbose {
            println!();
            output::header("Verbose Details");
            println!("  - List of projects");
            println!("  - Member list with roles");
            println!("  - Active policies");
            println!("  - Policy inheritance chain (company → org → team)");
        }
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_members(args: TeamMembersArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let team_id = args.team.clone().unwrap_or_else(|| {
        resolved
            .team_id
            .as_ref()
            .map_or_else(|| "current".to_string(), |t| t.value.clone())
    });

    if let Some(ref user_to_add) = args.add {
        let role = args.role.clone().unwrap_or_else(|| "developer".to_string());

        let valid_roles = ["developer", "techlead", "architect"];
        if !valid_roles.contains(&role.to_lowercase().as_str()) {
            let err = ux_error::UxError::new(format!("Invalid team role: '{role}'"))
                .why("Team roles determine user permissions within the team")
                .fix("Use one of: developer, techlead, architect")
                .suggest(format!(
                    "aeterna team members --add {user_to_add} --role developer"
                ));
            err.display();
            return Err(anyhow::anyhow!("Invalid role"));
        }

        if args.json {
            let output = json!({
                "operation": "team_member_add",
                "teamId": team_id,
                "userId": user_to_add,
                "role": role,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Add Team Member");
            println!();
            println!("  Team: {team_id}");
            println!("  User: {user_to_add}");
            println!("  Role: {role}");
            println!();

            output::header("Would Do");
            println!("  1. Verify user '{user_to_add}' exists in parent org");
            println!("  2. Check your permission to add members");
            println!("  3. Add user with role '{role}'");
            println!("  4. Grant access to team resources and projects");
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if let Some(ref user_to_remove) = args.remove {
        if args.json {
            let output = json!({
                "operation": "team_member_remove",
                "teamId": team_id,
                "userId": user_to_remove,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Remove Team Member");
            println!();
            println!("  Team: {team_id}");
            println!("  User: {user_to_remove}");
            println!();

            output::header("Would Do");
            println!("  1. Check your permission to remove members");
            println!("  2. Remove user from team");
            println!("  3. Revoke access to team resources");
            println!("  4. Keep user in parent org (if applicable)");
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if let Some(ref user_id) = args.set_role {
        let role = args.role.clone().ok_or_else(|| {
            let err = ux_error::UxError::new("Missing --role for --set-role")
                .why("Must specify which role to assign")
                .fix("Add --role with the desired role")
                .suggest(format!(
                    "aeterna team members --set-role {user_id} --role techlead"
                ));
            err.display();
            anyhow::anyhow!("Missing role")
        })?;

        if args.json {
            let output = json!({
                "operation": "team_member_set_role",
                "teamId": team_id,
                "userId": user_id,
                "newRole": role,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Set Member Role");
            println!();
            println!("  Team:     {team_id}");
            println!("  User:     {user_id}");
            println!("  New Role: {role}");
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if args.json {
        let output = json!({
            "operation": "team_members_list",
            "teamId": team_id,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Members of: {team_id}"));
        println!();

        output::header("Example Output (would show)");
        println!("  USER ID              NAME               ROLE        PROJECTS");
        println!("  alice@acme.com       Alice Smith        techlead    payments, auth");
        println!("  bob@acme.com         Bob Jones          developer   payments");
        println!("  carol@acme.com       Carol Williams     developer   auth, gateway");
        println!();

        output::header("Actions");
        println!("  Add member:    aeterna team members --add <user> --role <role>");
        println!("  Remove member: aeterna team members --remove <user>");
        println!("  Change role:   aeterna team members --set-role <user> --role <role>");
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_use(args: TeamUseArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _resolved = resolver.resolve()?;

    output::header("Set Default Team");
    println!();
    println!("  Setting default team: {}", args.team_id);
    println!();

    output::header("Would Update");
    println!("  File: .aeterna/context.toml");
    println!("  team_id = \"{}\"", args.team_id);
    println!();

    output::header("Effect");
    println!(
        "  - All commands will use '{}' as default team",
        args.team_id
    );
    println!("  - Project commands scoped to this team");
    println!("  - Policies from this team will apply");
    println!();

    let err = ux_error::server_not_connected();
    err.display();

    Ok(())
}
