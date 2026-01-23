use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::json;

use crate::output;
use crate::ux_error;

#[derive(Subcommand)]
pub enum OrgCommand {
    #[command(about = "Create a new organization")]
    Create(OrgCreateArgs),

    #[command(about = "List organizations in your company")]
    List(OrgListArgs),

    #[command(about = "Show organization details")]
    Show(OrgShowArgs),

    #[command(about = "Manage organization members")]
    Members(OrgMembersArgs),

    #[command(about = "Set default organization for current context")]
    Use(OrgUseArgs),
}

#[derive(Args)]
pub struct OrgCreateArgs {
    /// Organization name
    pub name: String,

    /// Organization description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Parent company ID (auto-detected if not provided)
    #[arg(long)]
    pub company: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run - show what would be created
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct OrgListArgs {
    /// Filter by company
    #[arg(long)]
    pub company: Option<String>,

    /// Show all organizations you have access to
    #[arg(long)]
    pub all: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct OrgShowArgs {
    /// Organization ID (uses current context if not provided)
    pub org_id: Option<String>,

    /// Show full details including policies and teams
    #[arg(short, long)]
    pub verbose: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct OrgMembersArgs {
    /// Organization ID (uses current context if not provided)
    #[arg(long)]
    pub org: Option<String>,

    /// Add member by user ID
    #[arg(long)]
    pub add: Option<String>,

    /// Remove member by user ID
    #[arg(long)]
    pub remove: Option<String>,

    /// Set role for a member (with --add or --set-role)
    #[arg(long, value_name = "USER_ID")]
    pub set_role: Option<String>,

    /// Role to assign (developer, techlead, architect, admin)
    #[arg(long)]
    pub role: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct OrgUseArgs {
    /// Organization ID to set as default
    pub org_id: String,
}

pub async fn run(cmd: OrgCommand) -> anyhow::Result<()> {
    match cmd {
        OrgCommand::Create(args) => run_create(args).await,
        OrgCommand::List(args) => run_list(args).await,
        OrgCommand::Show(args) => run_show(args).await,
        OrgCommand::Members(args) => run_members(args).await,
        OrgCommand::Use(args) => run_use(args).await,
    }
}

async fn run_create(args: OrgCreateArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let company_id = args
        .company
        .clone()
        .unwrap_or_else(|| resolved.tenant_id.value.clone());

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "org_create",
                "org": {
                    "name": args.name,
                    "description": args.description,
                    "companyId": company_id,
                },
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                },
                "nextSteps": [
                    "Review organization configuration",
                    "Run without --dry-run to create",
                    "Add members with 'aeterna org members --add <user>'",
                    "Create teams with 'aeterna team create <name> --org <org-id>'"
                ]
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Organization Create (Dry Run)");
            println!();
            println!("  Name:        {}", args.name);
            if let Some(ref desc) = args.description {
                println!("  Description: {}", desc);
            }
            println!("  Company:     {}", company_id);
            println!();

            output::header("What Would Happen");
            println!(
                "  1. Create organization '{}' under company '{}'",
                args.name, company_id
            );
            println!("  2. Add you ({}) as org admin", resolved.user_id.value);
            println!("  3. Inherit company-level policies");
            println!();

            output::header("Next Steps");
            println!("  1. Run without --dry-run to create the organization");
            println!("  2. Add members: aeterna org members --add <user-id>");
            println!(
                "  3. Create teams: aeterna team create <name> --org {}",
                args.name
            );
            println!();

            output::info("Dry run mode - organization not created.");
        }
        return Ok(());
    }

    let err = ux_error::server_not_connected();
    err.display();
    output::info("Run with --dry-run to see what would be created.");

    Ok(())
}

async fn run_list(args: OrgListArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    if args.json {
        let output = json!({
            "operation": "org_list",
            "filters": {
                "company": args.company,
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
        output::header("Organizations");
        println!();

        if args.all {
            output::info("Showing all organizations you have access to.");
        }

        if let Some(ref company) = args.company {
            println!("  Filter: company = {}", company);
        }
        println!();

        output::header("Example Output (would show)");
        println!("  ID                  NAME                 TEAMS  MEMBERS  ROLE");
        println!("  platform-eng        Platform Engineering   3       12    admin");
        println!("  product-eng         Product Engineering    2        8    member");
        println!("  security            Security               1        4    member");
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_show(args: OrgShowArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let org_id = args.org_id.clone().unwrap_or_else(|| {
        resolved
            .org_id
            .as_ref()
            .map(|o| o.value.clone())
            .unwrap_or_else(|| "current".to_string())
    });

    if args.json {
        let output = json!({
            "operation": "org_show",
            "orgId": org_id,
            "verbose": args.verbose,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Organization: {}", org_id));
        println!();

        output::header("Would Show");
        println!("  - Name and description");
        println!("  - Parent company");
        println!("  - Teams count");
        println!("  - Members count and roles");
        println!("  - Your role in this org");

        if args.verbose {
            println!();
            output::header("Verbose Details");
            println!("  - List of teams");
            println!("  - Member list with roles");
            println!("  - Active policies");
            println!("  - Policy inheritance chain");
        }
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_members(args: OrgMembersArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let org_id = args.org.clone().unwrap_or_else(|| {
        resolved
            .org_id
            .as_ref()
            .map(|o| o.value.clone())
            .unwrap_or_else(|| "current".to_string())
    });

    if let Some(ref user_to_add) = args.add {
        let role = args.role.clone().unwrap_or_else(|| "developer".to_string());

        let valid_roles = ["developer", "techlead", "architect", "admin"];
        if !valid_roles.contains(&role.to_lowercase().as_str()) {
            let err = ux_error::UxError::new(format!("Invalid role: '{}'", role))
                .why("Role determines user permissions within the organization")
                .fix("Use one of: developer, techlead, architect, admin")
                .suggest(&format!(
                    "aeterna org members --add {} --role developer",
                    user_to_add
                ));
            err.display();
            return Err(anyhow::anyhow!("Invalid role"));
        }

        if args.json {
            let output = json!({
                "operation": "org_member_add",
                "orgId": org_id,
                "userId": user_to_add,
                "role": role,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Add Organization Member");
            println!();
            println!("  Organization: {}", org_id);
            println!("  User:         {}", user_to_add);
            println!("  Role:         {}", role);
            println!();

            output::header("Would Do");
            println!("  1. Verify user '{}' exists", user_to_add);
            println!("  2. Check your permission to add members");
            println!("  3. Add user with role '{}'", role);
            println!("  4. Grant access to org resources");
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if let Some(ref user_to_remove) = args.remove {
        if args.json {
            let output = json!({
                "operation": "org_member_remove",
                "orgId": org_id,
                "userId": user_to_remove,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Remove Organization Member");
            println!();
            println!("  Organization: {}", org_id);
            println!("  User:         {}", user_to_remove);
            println!();

            output::header("Would Do");
            println!("  1. Check your permission to remove members");
            println!("  2. Remove user from organization");
            println!("  3. Revoke access to org resources");
            println!("  4. Remove from all teams in this org");
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
                .suggest(&format!(
                    "aeterna org members --set-role {} --role techlead",
                    user_id
                ));
            err.display();
            anyhow::anyhow!("Missing role")
        })?;

        if args.json {
            let output = json!({
                "operation": "org_member_set_role",
                "orgId": org_id,
                "userId": user_id,
                "newRole": role,
                "status": "not_connected"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Set Member Role");
            println!();
            println!("  Organization: {}", org_id);
            println!("  User:         {}", user_id);
            println!("  New Role:     {}", role);
            println!();

            let err = ux_error::server_not_connected();
            err.display();
        }
        return Ok(());
    }

    if args.json {
        let output = json!({
            "operation": "org_members_list",
            "orgId": org_id,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
            },
            "status": "not_connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Members of: {}", org_id));
        println!();

        output::header("Example Output (would show)");
        println!("  USER ID              NAME               ROLE        TEAMS");
        println!("  alice@acme.com       Alice Smith        admin       api, data");
        println!("  bob@acme.com         Bob Jones          techlead    api");
        println!("  carol@acme.com       Carol Williams     developer   web");
        println!();

        output::header("Actions");
        println!("  Add member:    aeterna org members --add <user> --role <role>");
        println!("  Remove member: aeterna org members --remove <user>");
        println!("  Change role:   aeterna org members --set-role <user> --role <role>");
        println!();

        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_use(args: OrgUseArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _resolved = resolver.resolve()?;

    output::header("Set Default Organization");
    println!();
    println!("  Setting default org: {}", args.org_id);
    println!();

    output::header("Would Update");
    println!("  File: .aeterna/context.toml");
    println!("  org_id = \"{}\"", args.org_id);
    println!();

    output::header("Effect");
    println!("  - All commands will use '{}' as default org", args.org_id);
    println!("  - Team/project commands scoped to this org");
    println!("  - Policies from this org will apply");
    println!();

    let err = ux_error::server_not_connected();
    err.display();

    Ok(())
}
