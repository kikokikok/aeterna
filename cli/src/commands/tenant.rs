use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::output;
use crate::ux_error;

// ---------------------------------------------------------------------------
// Top-level command
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TenantCommand {
    #[command(about = "Create a new tenant")]
    Create(TenantCreateArgs),

    #[command(about = "List tenants (platform admin)")]
    List(TenantListArgs),

    #[command(about = "Show tenant details")]
    Show(TenantShowArgs),

    #[command(about = "Update tenant properties")]
    Update(TenantUpdateArgs),

    #[command(about = "Deactivate a tenant")]
    Deactivate(TenantDeactivateArgs),

    #[command(about = "Set default tenant for current context")]
    Use(TenantUseArgs),

    #[command(
        name = "domain-map",
        about = "Add a verified domain mapping for a tenant"
    )]
    DomainMap(TenantDomainMapArgs),

    #[command(
        name = "repo-binding",
        subcommand,
        about = "Manage tenant repository bindings"
    )]
    RepoBinding(TenantRepoBindingCommand),

    #[command(name = "config", subcommand, about = "Manage tenant configuration")]
    Config(TenantConfigCommand),

    #[command(name = "secret", subcommand, about = "Manage tenant secret entries")]
    Secret(TenantSecretCommand),

    #[command(
        name = "connection",
        subcommand,
        about = "Manage Git provider connection visibility for tenants (PlatformAdmin)"
    )]
    Connection(TenantConnectionCommand),
}

// ---------------------------------------------------------------------------
// repo-binding sub-commands
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TenantRepoBindingCommand {
    #[command(about = "Show the repository binding for a tenant")]
    Show(TenantRepoBindingShowArgs),

    #[command(about = "Set the repository binding for a tenant")]
    Set(TenantRepoBindingSetArgs),

    #[command(about = "Validate the repository binding for a tenant")]
    Validate(TenantRepoBindingValidateArgs),
}

#[derive(Subcommand)]
pub enum TenantConfigCommand {
    #[command(about = "Inspect tenant configuration")]
    Inspect(TenantConfigInspectArgs),

    #[command(about = "Upsert tenant configuration from a JSON file")]
    Upsert(TenantConfigUpsertArgs),

    #[command(about = "Validate tenant configuration from a JSON file")]
    Validate(TenantConfigValidateArgs),
}

#[derive(Subcommand)]
pub enum TenantSecretCommand {
    #[command(about = "Set a tenant secret entry")]
    Set(TenantSecretSetArgs),

    #[command(about = "Delete a tenant secret entry")]
    Delete(TenantSecretDeleteArgs),
}

// ---------------------------------------------------------------------------
// Args structs
// ---------------------------------------------------------------------------

#[derive(Args)]
pub struct TenantCreateArgs {
    /// Tenant slug (URL-safe identifier)
    #[arg(long)]
    pub slug: String,

    /// Human-readable tenant name
    #[arg(long)]
    pub name: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run – show what would be created without calling the server
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantListArgs {
    /// Include inactive tenants in output
    #[arg(long)]
    pub include_inactive: bool,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantShowArgs {
    /// Tenant slug or ID
    pub tenant: String,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantUpdateArgs {
    /// Tenant slug or ID to update
    pub tenant: String,

    /// New slug value
    #[arg(long)]
    pub new_slug: Option<String>,

    /// New human-readable name
    #[arg(long)]
    pub name: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run – show what would change without calling the server
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantDeactivateArgs {
    /// Tenant slug or ID to deactivate
    pub tenant: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantUseArgs {
    /// Tenant slug to set as default context
    pub tenant: String,
}

#[derive(Args)]
pub struct TenantDomainMapArgs {
    /// Tenant slug or ID
    pub tenant: String,

    /// Domain to map (e.g. acme.example.com)
    #[arg(long)]
    pub domain: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantRepoBindingShowArgs {
    /// Tenant slug or ID
    pub tenant: String,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantRepoBindingSetArgs {
    /// Tenant slug or ID
    pub tenant: String,

    #[arg(long)]
    pub kind: String,

    /// Local path (for kind=local)
    #[arg(long)]
    pub local_path: Option<String>,

    /// Remote URL (for kind=remote)
    #[arg(long)]
    pub remote_url: Option<String>,

    /// Branch name
    #[arg(long)]
    pub branch: Option<String>,

    #[arg(long)]
    pub branch_policy: Option<String>,

    #[arg(long)]
    pub credential_kind: Option<String>,

    /// Credential reference (key name in secret store)
    #[arg(long)]
    pub credential_ref: Option<String>,

    /// GitHub organization owner (for kind=github)
    #[arg(long)]
    pub github_owner: Option<String>,

    /// GitHub repository name (for kind=github)
    #[arg(long)]
    pub github_repo: Option<String>,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run – show what would be set without calling the server
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantRepoBindingValidateArgs {
    /// Tenant slug or ID
    pub tenant: String,

    #[arg(long)]
    pub kind: String,

    /// Local path (for kind=local)
    #[arg(long)]
    pub local_path: Option<String>,

    /// Remote URL (for kind=remote)
    #[arg(long)]
    pub remote_url: Option<String>,

    /// Branch name
    #[arg(long)]
    pub branch: Option<String>,

    #[arg(long)]
    pub branch_policy: Option<String>,

    #[arg(long)]
    pub credential_kind: Option<String>,

    /// Credential reference (key name in secret store)
    #[arg(long)]
    pub credential_ref: Option<String>,

    /// GitHub organization owner (for kind=github)
    #[arg(long)]
    pub github_owner: Option<String>,

    /// GitHub repository name (for kind=github)
    #[arg(long)]
    pub github_repo: Option<String>,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConfigInspectArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConfigUpsertArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    #[arg(long)]
    pub file: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantConfigValidateArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    #[arg(long)]
    pub file: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantSecretSetArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    pub logical_name: String,

    #[arg(long)]
    pub value: String,

    #[arg(long, default_value = "tenant")]
    pub ownership: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantSecretDeleteArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    pub logical_name: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub async fn run(cmd: TenantCommand) -> anyhow::Result<()> {
    match cmd {
        TenantCommand::Create(args) => run_create(args).await,
        TenantCommand::List(args) => run_list(args).await,
        TenantCommand::Show(args) => run_show(args).await,
        TenantCommand::Update(args) => run_update(args).await,
        TenantCommand::Deactivate(args) => run_deactivate(args).await,
        TenantCommand::Use(args) => run_use(args).await,
        TenantCommand::DomainMap(args) => run_domain_map(args).await,
        TenantCommand::RepoBinding(sub) => match sub {
            TenantRepoBindingCommand::Show(args) => run_repo_binding_show(args).await,
            TenantRepoBindingCommand::Set(args) => run_repo_binding_set(args).await,
            TenantRepoBindingCommand::Validate(args) => run_repo_binding_validate(args).await,
        },
        TenantCommand::Config(sub) => match sub {
            TenantConfigCommand::Inspect(args) => run_config_inspect(args).await,
            TenantConfigCommand::Upsert(args) => run_config_upsert(args).await,
            TenantConfigCommand::Validate(args) => run_config_validate(args).await,
        },
        TenantCommand::Secret(sub) => match sub {
            TenantSecretCommand::Set(args) => run_secret_set(args).await,
            TenantSecretCommand::Delete(args) => run_secret_delete(args).await,
        },
        TenantCommand::Connection(sub) => match sub {
            TenantConnectionCommand::List(args) => run_connection_list(args).await,
            TenantConnectionCommand::Grant(args) => run_connection_grant(args).await,
            TenantConnectionCommand::Revoke(args) => run_connection_revoke(args).await,
        },
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tenant_server_required(operation: &str, message: &str) -> anyhow::Result<()> {
    ux_error::UxError::new(message)
        .why("This tenant command requires a live control-plane backend")
        .fix("Start the Aeterna server: aeterna serve")
        .fix("Ensure AETERNA_SERVER_URL is set and the server is reachable")
        .suggest("aeterna admin health")
        .display();
    anyhow::bail!("Aeterna server not connected for operation: {operation}")
}

async fn get_live_client() -> Option<crate::client::AeternaClient> {
    get_live_client_for(None).await
}

async fn get_live_client_for(target_tenant: Option<&str>) -> Option<crate::client::AeternaClient> {
    let resolved = crate::profile::load_resolved(None, None);
    if let Ok(ref r) = resolved {
        let client = crate::client::AeternaClient::from_profile(r).await.ok()?;
        if let Some(tenant) = target_tenant {
            Some(client.with_target_tenant(tenant))
        } else {
            Some(client)
        }
    } else {
        None
    }
}

fn repo_binding_body(
    kind: &str,
    local_path: Option<&str>,
    remote_url: Option<&str>,
    branch: Option<&str>,
    branch_policy: Option<&str>,
    credential_kind: Option<&str>,
    credential_ref: Option<&str>,
    github_owner: Option<&str>,
    github_repo: Option<&str>,
) -> serde_json::Value {
    let mut body = json!({ "kind": kind, "sourceOwner": "admin" });
    if let Some(v) = local_path {
        body["localPath"] = json!(v);
    }
    if let Some(v) = remote_url {
        body["remoteUrl"] = json!(v);
    }
    if let Some(v) = branch {
        body["branch"] = json!(v);
    }
    if let Some(v) = branch_policy {
        body["branchPolicy"] = json!(v);
    }
    if let Some(v) = credential_kind {
        body["credentialKind"] = json!(v);
    }
    if let Some(v) = credential_ref {
        body["credentialRef"] = json!(v);
    }
    if let Some(v) = github_owner {
        body["githubOwner"] = json!(v);
    }
    if let Some(v) = github_repo {
        body["githubRepo"] = json!(v);
    }
    body
}

fn redact_secret_values(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map.iter_mut() {
                if key == "secretValue" || key == "secret_value" {
                    *nested = json!("[REDACTED]");
                } else {
                    redact_secret_values(nested);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_secret_values(item);
            }
        }
        _ => {}
    }
}

fn redacted_json(mut value: Value) -> Value {
    redact_secret_values(&mut value);
    value
}

fn tenant_config_ownership(ownership: &str) -> anyhow::Result<&'static str> {
    match ownership {
        "tenant" => Ok("tenant"),
        "platform" => Ok("platform"),
        _ => {
            ux_error::UxError::new(format!("Invalid ownership: '{ownership}'"))
                .why("Supported ownership values are: tenant, platform")
                .fix("Use --ownership tenant or --ownership platform")
                .display();
            anyhow::bail!("Invalid tenant config ownership")
        }
    }
}

fn read_json_file(path: &str) -> anyhow::Result<Value> {
    let raw = fs::read_to_string(path)?;
    let payload: Value =
        serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("Invalid JSON in '{path}': {e}"))?;
    Ok(payload)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn run_create(args: TenantCreateArgs) -> anyhow::Result<()> {
    if args.dry_run {
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_create",
                "tenant": { "slug": args.slug, "name": args.name },
                "nextSteps": [
                    "Run without --dry-run to create",
                    "Use 'aeterna tenant use <slug>' to set as default context"
                ]
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Tenant Create (Dry Run)");
            println!();
            println!("  Slug: {}", args.slug);
            println!("  Name: {}", args.name);
            println!();
            output::info("Dry run mode – tenant not created.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let body = json!({ "slug": args.slug, "name": args.name });
        let result = client.tenant_create(&body).await.inspect_err(|e| {
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
            output::header("Tenant Created");
            println!();
            if let Some(t) = result["tenant"].as_object() {
                println!(
                    "  Slug:   {}",
                    t.get("slug").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Name:   {}",
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  ID:     {}",
                    t.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Status: {}",
                    t.get("status").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            println!();
            output::hint(
                "Use 'aeterna tenant use <slug>' to set this tenant as your default context",
            );
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_create"
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_create");
    }
    tenant_server_required(
        "tenant_create",
        "Cannot create tenant: server not connected",
    )
}

async fn run_list(args: TenantListArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client
            .tenant_list(args.include_inactive)
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
            output::header("Tenants");
            println!();
            if let Some(tenants) = result["tenants"].as_array() {
                if tenants.is_empty() {
                    println!("  (no tenants found)");
                } else {
                    for t in tenants {
                        let slug = t["slug"].as_str().unwrap_or("?");
                        let name = t["name"].as_str().unwrap_or("?");
                        let status = t["status"].as_str().unwrap_or("?");
                        println!("  {slug:<24} {name:<32} [{status}]");
                    }
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_list"
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_list");
    }
    tenant_server_required("tenant_list", "Cannot list tenants: server not connected")
}

async fn run_show(args: TenantShowArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client.tenant_show(&args.tenant).await.inspect_err(|e| {
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
            output::header(&format!("Tenant: {}", args.tenant));
            println!();
            if let Some(t) = result["tenant"].as_object() {
                println!(
                    "  ID:      {}",
                    t.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Slug:    {}",
                    t.get("slug").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Name:    {}",
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Status:  {}",
                    t.get("status").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Source:  {}",
                    t.get("sourceOwner").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_show",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_show");
    }
    tenant_server_required(
        "tenant_show",
        &format!("Cannot show tenant '{}': server not connected", args.tenant),
    )
}

async fn run_update(args: TenantUpdateArgs) -> anyhow::Result<()> {
    if args.dry_run {
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_update",
                "tenant": args.tenant,
                "changes": {
                    "slug": args.new_slug,
                    "name": args.name
                }
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Tenant Update (Dry Run)");
            println!();
            println!("  Tenant: {}", args.tenant);
            if let Some(ref s) = args.new_slug {
                println!("  New Slug: {s}");
            }
            if let Some(ref n) = args.name {
                println!("  New Name: {n}");
            }
            println!();
            output::info("Dry run mode – tenant not updated.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let mut body = json!({});
        if let Some(ref s) = args.new_slug {
            body["slug"] = json!(s);
        }
        if let Some(ref n) = args.name {
            body["name"] = json!(n);
        }
        let result = client
            .tenant_update(&args.tenant, &body)
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
            output::header("Tenant Updated");
            println!();
            if let Some(t) = result["tenant"].as_object() {
                println!(
                    "  Slug:   {}",
                    t.get("slug").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Name:   {}",
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Status: {}",
                    t.get("status").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_update",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_update");
    }
    tenant_server_required(
        "tenant_update",
        &format!(
            "Cannot update tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_deactivate(args: TenantDeactivateArgs) -> anyhow::Result<()> {
    if !args.yes {
        eprintln!(
            "This will deactivate tenant '{}'. Use --yes to confirm.",
            args.tenant
        );
        eprintln!("Use --yes to skip this confirmation.");
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let result = client
            .tenant_deactivate(&args.tenant)
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
            output::header("Tenant Deactivated");
            println!();
            println!("  Tenant '{}' has been deactivated.", args.tenant);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_deactivate",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_deactivate");
    }
    tenant_server_required(
        "tenant_deactivate",
        &format!(
            "Cannot deactivate tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_use(args: TenantUseArgs) -> anyhow::Result<()> {
    let _resolver = ContextResolver::new();

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
        table.insert(
            "tenant_id".to_string(),
            toml::Value::String(args.tenant.clone()),
        );
    }

    fs::create_dir_all(aeterna_dir)?;
    fs::write(&context_file, toml::to_string_pretty(&config)?)?;

    output::header("Set Default Tenant");
    println!();
    println!("  Setting default tenant: {}", args.tenant);
    println!();
    println!("  ✓ Updated .aeterna/context.toml");
    println!("  tenant_id = \"{}\"", args.tenant);

    Ok(())
}

async fn run_domain_map(args: TenantDomainMapArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let body = json!({ "domain": args.domain });
        let result = client
            .tenant_add_domain_mapping(&args.tenant, &body)
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
            output::header("Domain Mapping Added");
            println!();
            println!("  Tenant: {}", args.tenant);
            println!("  Domain: {}", args.domain);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_domain_map",
            "tenant": args.tenant,
            "domain": args.domain
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_domain_map");
    }
    tenant_server_required(
        "tenant_domain_map",
        &format!(
            "Cannot add domain mapping for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_repo_binding_show(args: TenantRepoBindingShowArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client
            .tenant_repo_binding_show(&args.tenant)
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
            output::header(&format!("Repository Binding: {}", args.tenant));
            println!();
            if let Some(b) = result["binding"].as_object() {
                println!(
                    "  Kind:          {}",
                    b.get("kind").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Branch:        {}",
                    b.get("branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(default)")
                );
                println!(
                    "  Branch Policy: {}",
                    b.get("branchPolicy")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                );
                println!(
                    "  Credential:    {}",
                    b.get("credentialKind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none")
                );
            } else {
                println!("  (no binding configured)");
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_repo_binding_show",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_repo_binding_show");
    }
    tenant_server_required(
        "tenant_repo_binding_show",
        &format!(
            "Cannot show repo binding for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_repo_binding_set(args: TenantRepoBindingSetArgs) -> anyhow::Result<()> {
    let valid_kinds = ["local", "gitRemote", "github"];
    if !valid_kinds.contains(&args.kind.as_str()) {
        ux_error::UxError::new(format!("Invalid repository kind: '{}'", args.kind))
            .why("Supported kinds are: local, gitRemote, github")
            .fix("Use --kind local, --kind gitRemote, or --kind github")
            .display();
        anyhow::bail!("Invalid repository kind");
    }

    if args.dry_run {
        let body = repo_binding_body(
            &args.kind,
            args.local_path.as_deref(),
            args.remote_url.as_deref(),
            args.branch.as_deref(),
            args.branch_policy.as_deref(),
            args.credential_kind.as_deref(),
            args.credential_ref.as_deref(),
            args.github_owner.as_deref(),
            args.github_repo.as_deref(),
        );
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_repo_binding_set",
                "tenant": args.tenant,
                "binding": body
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Repository Binding Set (Dry Run)");
            println!();
            println!("  Tenant: {}", args.tenant);
            println!("  Kind:   {}", args.kind);
            if let Some(ref p) = args.local_path {
                println!("  Path:   {p}");
            }
            if let Some(ref u) = args.remote_url {
                println!("  URL:    {u}");
            }
            println!();
            output::info("Dry run mode – binding not set.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let body = repo_binding_body(
            &args.kind,
            args.local_path.as_deref(),
            args.remote_url.as_deref(),
            args.branch.as_deref(),
            args.branch_policy.as_deref(),
            args.credential_kind.as_deref(),
            args.credential_ref.as_deref(),
            args.github_owner.as_deref(),
            args.github_repo.as_deref(),
        );
        let result = client
            .tenant_repo_binding_set(&args.tenant, &body)
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
            output::header("Repository Binding Set");
            println!();
            println!("  Tenant: {}", args.tenant);
            println!("  Kind:   {}", args.kind);
            println!();
            output::hint(
                "Use 'aeterna tenant repo-binding validate <tenant>' to verify the binding",
            );
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_repo_binding_set",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_repo_binding_set");
    }
    tenant_server_required(
        "tenant_repo_binding_set",
        &format!(
            "Cannot set repo binding for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_repo_binding_validate(args: TenantRepoBindingValidateArgs) -> anyhow::Result<()> {
    let valid_kinds = ["local", "gitRemote", "github"];
    if !valid_kinds.contains(&args.kind.as_str()) {
        ux_error::UxError::new(format!("Invalid repository kind: '{}'", args.kind))
            .why("Supported kinds are: local, gitRemote, github")
            .fix("Use --kind local, --kind gitRemote, or --kind github")
            .display();
        anyhow::bail!("Invalid repository kind");
    }

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let body = repo_binding_body(
            &args.kind,
            args.local_path.as_deref(),
            args.remote_url.as_deref(),
            args.branch.as_deref(),
            args.branch_policy.as_deref(),
            args.credential_kind.as_deref(),
            args.credential_ref.as_deref(),
            args.github_owner.as_deref(),
            args.github_repo.as_deref(),
        );
        let result = client
            .tenant_repo_binding_validate(&args.tenant, &body)
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
            output::header("Repository Binding Validation");
            println!();
            println!("  Tenant: {}", args.tenant);
            let valid = result["valid"].as_bool().unwrap_or(false);
            let icon = if valid { "✓" } else { "✗" };
            println!(
                "  Result: {} {}",
                icon,
                if valid { "valid" } else { "invalid" }
            );
            if let Some(msg) = result["message"].as_str() {
                if !msg.is_empty() {
                    println!("  Detail: {msg}");
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_repo_binding_validate",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_repo_binding_validate");
    }
    tenant_server_required(
        "tenant_repo_binding_validate",
        &format!(
            "Cannot validate repo binding for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_config_inspect(args: TenantConfigInspectArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client.tenant_config_inspect(tenant).await
        } else {
            client.my_tenant_config_inspect().await
        }
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

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Config");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            if let Some(config) = redacted["config"].as_object() {
                let field_count = config
                    .get("fields")
                    .and_then(|v| v.as_object())
                    .map_or(0, serde_json::Map::len);
                let secret_ref_count = config
                    .get("secretReferences")
                    .and_then(|v| v.as_object())
                    .map_or(0, serde_json::Map::len);
                println!("  Fields:            {field_count}");
                println!("  Secret References: {secret_ref_count}");
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_config_inspect",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_config_inspect");
    }
    tenant_server_required(
        "tenant_config_inspect",
        "Cannot inspect tenant config: server not connected",
    )
}

async fn run_config_upsert(args: TenantConfigUpsertArgs) -> anyhow::Result<()> {
    let payload = read_json_file(&args.file)?;

    if args.dry_run {
        let redacted_payload = redacted_json(payload);
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_config_upsert",
                "tenant": args.tenant,
                "payload": redacted_payload,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Tenant Config Upsert (Dry Run)");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            println!("  File:   {}", args.file);
            println!();
            output::info("Dry run mode – tenant config not updated.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client.tenant_config_upsert(tenant, &payload).await
        } else {
            client.my_tenant_config_upsert(&payload).await
        }
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

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Config Upserted");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            println!("  File:   {}", args.file);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_config_upsert",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_config_upsert");
    }
    tenant_server_required(
        "tenant_config_upsert",
        "Cannot upsert tenant config: server not connected",
    )
}

async fn run_config_validate(args: TenantConfigValidateArgs) -> anyhow::Result<()> {
    let payload = read_json_file(&args.file)?;

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client.tenant_config_validate(tenant, &payload).await
        } else {
            client.my_tenant_config_validate(&payload).await
        }
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

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Config Validation");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            let valid = redacted["valid"].as_bool().unwrap_or(false);
            let icon = if valid { "✓" } else { "✗" };
            println!(
                "  Result: {} {}",
                icon,
                if valid { "valid" } else { "invalid" }
            );
            if let Some(msg) = redacted["message"].as_str() {
                if !msg.is_empty() {
                    println!("  Detail: {msg}");
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_config_validate",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_config_validate");
    }
    tenant_server_required(
        "tenant_config_validate",
        "Cannot validate tenant config: server not connected",
    )
}

async fn run_secret_set(args: TenantSecretSetArgs) -> anyhow::Result<()> {
    let ownership = tenant_config_ownership(args.ownership.as_str())?;
    let body = json!({
        "ownership": ownership,
        "secretValue": args.value,
    });

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client
                .tenant_secret_set(tenant, &args.logical_name, &body)
                .await
        } else {
            client.my_tenant_secret_set(&args.logical_name, &body).await
        }
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

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Secret Set");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant:       {tenant}");
            } else {
                println!("  Scope:        current tenant context");
            }
            println!("  Logical Name: {}", args.logical_name);
            println!("  Ownership:    {ownership}");
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_secret_set",
            "tenant": args.tenant,
            "logicalName": args.logical_name,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_secret_set");
    }
    tenant_server_required(
        "tenant_secret_set",
        "Cannot set tenant secret: server not connected",
    )
}

async fn run_secret_delete(args: TenantSecretDeleteArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client
                .tenant_secret_delete(tenant, &args.logical_name)
                .await
        } else {
            client.my_tenant_secret_delete(&args.logical_name).await
        }
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

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Secret Deleted");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant:       {tenant}");
            } else {
                println!("  Scope:        current tenant context");
            }
            println!("  Logical Name: {}", args.logical_name);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_secret_delete",
            "tenant": args.tenant,
            "logicalName": args.logical_name,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_secret_delete");
    }
    tenant_server_required(
        "tenant_secret_delete",
        "Cannot delete tenant secret: server not connected",
    )
}

// ---------------------------------------------------------------------------
// connection sub-commands
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TenantConnectionCommand {
    #[command(
        about = "List Git provider connections visible to a tenant (PlatformAdmin or TenantAdmin)"
    )]
    List(TenantConnectionListArgs),

    #[command(about = "Grant a tenant visibility of a Git provider connection (PlatformAdmin)")]
    Grant(TenantConnectionGrantArgs),

    #[command(about = "Revoke a tenant's visibility of a Git provider connection (PlatformAdmin)")]
    Revoke(TenantConnectionRevokeArgs),
}

#[derive(Args)]
pub struct TenantConnectionListArgs {
    /// Tenant slug to list connections for
    pub tenant: String,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConnectionGrantArgs {
    /// Tenant slug to grant visibility to
    pub tenant: String,

    /// Git provider connection ID to grant
    #[arg(long)]
    pub connection: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConnectionRevokeArgs {
    /// Tenant slug to revoke visibility from
    pub tenant: String,

    /// Git provider connection ID to revoke
    #[arg(long)]
    pub connection: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

// ---------------------------------------------------------------------------
// connection handlers
// ---------------------------------------------------------------------------

async fn run_connection_list(args: TenantConnectionListArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client
            .tenant_git_provider_connections_list(&args.tenant)
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
            output::header(&format!("Git Provider Connections: {}", args.tenant));
            println!();
            if let Some(connections) = result["connections"].as_array() {
                if connections.is_empty() {
                    println!("  (no connections visible to this tenant)");
                } else {
                    for c in connections {
                        let id = c["id"].as_str().unwrap_or("?");
                        let name = c["name"].as_str().unwrap_or("?");
                        let kind = c["providerKind"].as_str().unwrap_or("?");
                        println!("  {id:<32} {name:<32} [{kind}]");
                    }
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "connection_list",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: connection_list");
    }
    tenant_server_required(
        "connection_list",
        &format!(
            "Cannot list connections for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_connection_grant(args: TenantConnectionGrantArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client
            .git_provider_connection_grant_tenant(&args.connection, &args.tenant)
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
            output::header("Connection Granted");
            println!();
            println!("  Tenant:     {}", args.tenant);
            println!("  Connection: {}", args.connection);
            println!();
            output::hint(
                "Use 'aeterna tenant connection list <tenant>' to verify the connection is visible",
            );
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "connection_grant",
            "tenant": args.tenant,
            "connection": args.connection
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: connection_grant");
    }
    tenant_server_required(
        "connection_grant",
        "Cannot grant connection: server not connected",
    )
}

async fn run_connection_revoke(args: TenantConnectionRevokeArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client
            .git_provider_connection_revoke_tenant(&args.connection, &args.tenant)
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
            output::header("Connection Revoked");
            println!();
            println!("  Tenant:     {}", args.tenant);
            println!("  Connection: {}", args.connection);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "connection_revoke",
            "tenant": args.tenant,
            "connection": args.connection
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: connection_revoke");
    }
    tenant_server_required(
        "connection_revoke",
        "Cannot revoke connection: server not connected",
    )
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_create_args_dry_run_fields() {
        let args = TenantCreateArgs {
            slug: "acme".to_string(),
            name: "Acme Corp".to_string(),
            json: false,
            dry_run: true,
        };
        assert_eq!(args.slug, "acme");
        assert_eq!(args.name, "Acme Corp");
        assert!(args.dry_run);
        assert!(!args.json);
    }

    #[test]
    fn test_tenant_list_args_defaults() {
        let args = TenantListArgs {
            include_inactive: false,
            target_tenant: None,
            json: false,
        };
        assert!(!args.include_inactive);
        assert!(!args.json);
    }

    #[test]
    fn test_tenant_list_args_include_inactive() {
        let args = TenantListArgs {
            include_inactive: true,
            target_tenant: None,
            json: true,
        };
        assert!(args.include_inactive);
        assert!(args.json);
    }

    #[test]
    fn test_tenant_show_args() {
        let args = TenantShowArgs {
            tenant: "acme".to_string(),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.tenant, "acme");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_update_args_partial() {
        let args = TenantUpdateArgs {
            tenant: "acme".to_string(),
            new_slug: None,
            name: Some("Acme Corporation".to_string()),
            json: false,
            dry_run: false,
        };
        assert!(args.new_slug.is_none());
        assert_eq!(args.name, Some("Acme Corporation".to_string()));
    }

    #[test]
    fn test_tenant_update_args_dry_run() {
        let args = TenantUpdateArgs {
            tenant: "acme".to_string(),
            new_slug: Some("acme-corp".to_string()),
            name: None,
            json: false,
            dry_run: true,
        };
        assert!(args.dry_run);
        assert_eq!(args.new_slug, Some("acme-corp".to_string()));
    }

    #[test]
    fn test_tenant_deactivate_args_requires_yes() {
        let args = TenantDeactivateArgs {
            tenant: "acme".to_string(),
            yes: false,
            json: false,
        };
        assert!(!args.yes);
    }

    #[test]
    fn test_tenant_deactivate_args_confirmed() {
        let args = TenantDeactivateArgs {
            tenant: "acme".to_string(),
            yes: true,
            json: true,
        };
        assert!(args.yes);
        assert!(args.json);
    }

    #[test]
    fn test_tenant_use_args() {
        let args = TenantUseArgs {
            tenant: "acme".to_string(),
        };
        assert_eq!(args.tenant, "acme");
    }

    #[test]
    fn test_tenant_domain_map_args() {
        let args = TenantDomainMapArgs {
            tenant: "acme".to_string(),
            domain: "acme.example.com".to_string(),
            json: false,
        };
        assert_eq!(args.domain, "acme.example.com");
    }

    #[test]
    fn test_repo_binding_body_local() {
        let body = repo_binding_body(
            "local",
            Some("/repos/acme"),
            None,
            Some("main"),
            Some("directCommit"),
            None,
            None,
            None,
            None,
        );
        assert_eq!(body["kind"], "local");
        assert_eq!(body["localPath"], "/repos/acme");
        assert_eq!(body["branch"], "main");
        assert_eq!(body["branchPolicy"], "directCommit");
        assert_eq!(body["sourceOwner"], "admin");
    }

    #[test]
    fn test_repo_binding_body_github() {
        let body = repo_binding_body(
            "github",
            None,
            None,
            Some("main"),
            Some("directCommit"),
            Some("githubApp"),
            Some("my-app-cred"),
            Some("acme-org"),
            Some("knowledge-repo"),
        );
        assert_eq!(body["kind"], "github");
        assert_eq!(body["githubOwner"], "acme-org");
        assert_eq!(body["githubRepo"], "knowledge-repo");
        assert_eq!(body["credentialKind"], "githubApp");
        assert_eq!(body["credentialRef"], "my-app-cred");
    }

    #[test]
    fn test_repo_binding_body_remote() {
        let body = repo_binding_body(
            "gitRemote",
            None,
            Some("https://github.com/acme/knowledge.git"),
            None,
            None,
            Some("sshKey"),
            Some("acme-deploy-key"),
            None,
            None,
        );
        assert_eq!(body["kind"], "gitRemote");
        assert_eq!(body["remoteUrl"], "https://github.com/acme/knowledge.git");
        assert_eq!(body["credentialKind"], "sshKey");
    }

    #[test]
    fn test_repo_binding_body_minimal() {
        let body = repo_binding_body("local", None, None, None, None, None, None, None, None);
        assert_eq!(body["kind"], "local");
        assert_eq!(body["sourceOwner"], "admin");
        assert!(body.get("localPath").is_none());
        assert!(body.get("remoteUrl").is_none());
    }

    #[test]
    fn test_tenant_repo_binding_show_args() {
        let args = TenantRepoBindingShowArgs {
            tenant: "acme".to_string(),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.tenant, "acme");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_repo_binding_set_args() {
        let args = TenantRepoBindingSetArgs {
            tenant: "acme".to_string(),
            kind: "github".to_string(),
            local_path: None,
            remote_url: None,
            branch: Some("main".to_string()),
            branch_policy: Some("directCommit".to_string()),
            credential_kind: Some("githubApp".to_string()),
            credential_ref: Some("my-cred".to_string()),
            github_owner: Some("acme-org".to_string()),
            github_repo: Some("knowledge-repo".to_string()),
            target_tenant: None,
            json: false,
            dry_run: false,
        };
        assert_eq!(args.kind, "github");
        assert_eq!(args.github_owner, Some("acme-org".to_string()));
        assert!(!args.dry_run);
    }

    #[test]
    fn test_tenant_repo_binding_validate_args() {
        let args = TenantRepoBindingValidateArgs {
            tenant: "acme".to_string(),
            kind: "local".to_string(),
            local_path: Some("/repos/acme".to_string()),
            remote_url: None,
            branch: None,
            branch_policy: None,
            credential_kind: None,
            credential_ref: None,
            github_owner: None,
            github_repo: None,
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.kind, "local");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_config_inspect_args() {
        let args = TenantConfigInspectArgs {
            tenant: Some("acme".to_string()),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.tenant.as_deref(), Some("acme"));
        assert!(args.json);
    }

    #[test]
    fn test_tenant_config_upsert_args_dry_run() {
        let args = TenantConfigUpsertArgs {
            tenant: None,
            file: "config.json".to_string(),
            target_tenant: Some("acme".to_string()),
            json: false,
            dry_run: true,
        };
        assert!(args.dry_run);
        assert_eq!(args.file, "config.json");
        assert_eq!(args.target_tenant.as_deref(), Some("acme"));
    }

    #[test]
    fn test_tenant_secret_set_args() {
        let args = TenantSecretSetArgs {
            tenant: Some("acme".to_string()),
            logical_name: "repo.token".to_string(),
            value: "s3cr3t".to_string(),
            ownership: "tenant".to_string(),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.logical_name, "repo.token");
        assert_eq!(args.ownership, "tenant");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_secret_delete_args() {
        let args = TenantSecretDeleteArgs {
            tenant: None,
            logical_name: "repo.token".to_string(),
            target_tenant: Some("acme".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("acme"));
        assert_eq!(args.logical_name, "repo.token");
    }

    #[test]
    fn test_redact_secret_values() {
        let mut payload = json!({
            "secretValue": "raw-secret",
            "nested": {
                "secret_value": "also-raw"
            }
        });
        redact_secret_values(&mut payload);
        assert_eq!(payload["secretValue"], "[REDACTED]");
        assert_eq!(payload["nested"]["secret_value"], "[REDACTED]");
    }

    #[test]
    fn test_tenant_config_ownership_validation() {
        assert_eq!(tenant_config_ownership("tenant").unwrap(), "tenant");
        assert_eq!(tenant_config_ownership("platform").unwrap(), "platform");
        assert!(tenant_config_ownership("invalid").is_err());
    }

    #[test]
    fn test_tenant_list_args_target_tenant() {
        let args = TenantListArgs {
            include_inactive: false,
            target_tenant: Some("platform-tenant".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("platform-tenant"));
    }

    #[test]
    fn test_tenant_show_args_target_tenant() {
        let args = TenantShowArgs {
            tenant: "acme".to_string(),
            target_tenant: Some("parent-tenant".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("parent-tenant"));
    }

    #[test]
    fn test_tenant_repo_binding_show_args_target_tenant() {
        let args = TenantRepoBindingShowArgs {
            tenant: "acme".to_string(),
            target_tenant: Some("admin-context".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("admin-context"));
    }
}
