use clap::{Args, Subcommand, ValueEnum};
use context::ContextResolver;
use serde_json::json;

use crate::output;
use crate::ux_error;

#[derive(Subcommand)]
pub enum GovernCommand {
    #[command(about = "Show governance status and pending approvals")]
    Status(GovernStatusArgs),

    #[command(about = "List pending approval requests")]
    Pending(GovernPendingArgs),

    #[command(about = "Approve a governance request")]
    Approve(GovernApproveArgs),

    #[command(about = "Reject a governance request")]
    Reject(GovernRejectArgs),

    #[command(about = "Configure governance settings")]
    Configure(GovernConfigureArgs),

    #[command(about = "Manage governance roles")]
    Roles(GovernRolesArgs),

    #[command(about = "View governance audit trail")]
    Audit(GovernAuditArgs)
}

#[derive(Args)]
pub struct GovernStatusArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Include detailed metrics
    #[arg(short, long)]
    pub verbose: bool
}

#[derive(Args)]
pub struct GovernPendingArgs {
    /// Filter by type (policy, knowledge, memory, all)
    #[arg(short = 't', long, default_value = "all")]
    pub request_type: String,

    /// Filter by layer (company, org, team, project)
    #[arg(short, long)]
    pub layer: Option<String>,

    /// Filter by requestor
    #[arg(long)]
    pub requestor: Option<String>,

    /// Show only my pending requests
    #[arg(long)]
    pub mine: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct GovernApproveArgs {
    /// Request ID to approve
    pub request_id: String,

    /// Approval comment
    #[arg(short, long)]
    pub comment: Option<String>,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct GovernRejectArgs {
    /// Request ID to reject
    pub request_id: String,

    /// Rejection reason (required)
    #[arg(short, long)]
    pub reason: String,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct GovernConfigureArgs {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Use a predefined governance template (standard, strict, permissive)
    #[arg(long, value_enum)]
    pub template: Option<GovernanceTemplate>,

    /// List available templates and their settings
    #[arg(long)]
    pub list_templates: bool,

    /// Set approval requirement (single, quorum, unanimous)
    #[arg(long)]
    pub approval_mode: Option<ApprovalMode>,

    /// Set minimum approvers required (for quorum mode)
    #[arg(long)]
    pub min_approvers: Option<u32>,

    /// Set approval timeout in hours
    #[arg(long)]
    pub timeout_hours: Option<u32>,

    /// Enable/disable auto-approve for low-risk changes
    #[arg(long)]
    pub auto_approve: Option<bool>,

    /// Set escalation contact
    #[arg(long)]
    pub escalation_contact: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct GovernRolesArgs {
    /// Role action (list, assign, revoke)
    #[arg(default_value = "list")]
    pub action: String,

    /// User or agent ID for assign/revoke
    #[arg(long)]
    pub principal: Option<String>,

    /// Role to assign/revoke
    #[arg(long)]
    pub role: Option<String>,

    /// Scope for the role (company, org, team, project)
    #[arg(long)]
    pub scope: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct GovernAuditArgs {
    /// Filter by action type (approve, reject, escalate, expire, all)
    #[arg(short, long, default_value = "all")]
    pub action: String,

    /// Filter by time range (1h, 24h, 7d, 30d, 90d)
    #[arg(long, default_value = "7d")]
    pub since: String,

    /// Filter by actor (user or agent ID)
    #[arg(long)]
    pub actor: Option<String>,

    /// Filter by target type (policy, knowledge, memory)
    #[arg(long)]
    pub target_type: Option<String>,

    /// Maximum number of entries to show
    #[arg(short, long, default_value = "50")]
    pub limit: usize,

    /// Export format (json, csv, none)
    #[arg(long, default_value = "none")]
    pub export: ExportFormat,

    /// Output file for export
    #[arg(short, long)]
    pub output: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Clone, ValueEnum)]
pub enum ApprovalMode {
    Single,
    Quorum,
    Unanimous
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
    None
}

#[derive(Clone, ValueEnum)]
pub enum GovernanceTemplate {
    Standard,
    Strict,
    Permissive
}

pub async fn run(cmd: GovernCommand) -> anyhow::Result<()> {
    match cmd {
        GovernCommand::Status(args) => run_status(args).await,
        GovernCommand::Pending(args) => run_pending(args).await,
        GovernCommand::Approve(args) => run_approve(args).await,
        GovernCommand::Reject(args) => run_reject(args).await,
        GovernCommand::Configure(args) => run_configure(args).await,
        GovernCommand::Roles(args) => run_roles(args).await,
        GovernCommand::Audit(args) => run_audit(args).await
    }
}

async fn run_status(args: GovernStatusArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let ctx = resolver.resolve()?;

    // Simulated governance status
    let status = GovernanceStatus {
        approval_mode: "quorum".to_string(),
        min_approvers: 2,
        timeout_hours: 72,
        auto_approve_enabled: true,
        pending_requests: 3,
        approved_today: 7,
        rejected_today: 1,
        escalated: 0,
        your_pending_approvals: 2
    };

    if args.json {
        let output = json!({
            "context": {
                "tenant_id": ctx.tenant_id.value,
                "user_id": ctx.user_id.value,
            },
            "config": {
                "approval_mode": status.approval_mode,
                "min_approvers": status.min_approvers,
                "timeout_hours": status.timeout_hours,
                "auto_approve_enabled": status.auto_approve_enabled,
            },
            "metrics": {
                "pending_requests": status.pending_requests,
                "approved_today": status.approved_today,
                "rejected_today": status.rejected_today,
                "escalated": status.escalated,
                "your_pending_approvals": status.your_pending_approvals,
            },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Governance Status");
        println!();

        output::subheader("Configuration");
        println!("  Approval Mode:    {}", status.approval_mode);
        println!("  Min Approvers:    {}", status.min_approvers);
        println!("  Timeout:          {} hours", status.timeout_hours);
        println!(
            "  Auto-approve:     {}",
            if status.auto_approve_enabled {
                "enabled (low-risk)"
            } else {
                "disabled"
            }
        );
        println!();

        output::subheader("Activity (Today)");
        println!("  Pending Requests: {}", status.pending_requests);
        println!("  Approved:         {}", status.approved_today);
        println!("  Rejected:         {}", status.rejected_today);
        println!("  Escalated:        {}", status.escalated);
        println!();

        if status.your_pending_approvals > 0 {
            println!(
                "  ⚡ You have {} request(s) awaiting your approval",
                status.your_pending_approvals
            );
            println!();
            output::hint("Run 'aeterna govern pending --mine' to see your pending approvals");
        }

        if args.verbose {
            output::subheader("Recent Activity");
            println!("  • alice approved policy 'security-baseline' (2h ago)");
            println!("  • bob requested knowledge promotion 'ADR-042' (4h ago)");
            println!("  • system auto-approved memory feedback (6h ago)");
        }
    }

    Ok(())
}

async fn run_pending(args: GovernPendingArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _ctx = resolver.resolve()?;

    // Simulated pending requests
    let requests = [
        PendingRequest {
            id: "req_abc123".to_string(),
            request_type: "policy".to_string(),
            title: "Add security-baseline policy".to_string(),
            requestor: "alice".to_string(),
            layer: "org".to_string(),
            created_at: "2024-01-15T10:30:00Z".to_string(),
            approvals: 1,
            required_approvals: 2,
            status: "pending".to_string()
        },
        PendingRequest {
            id: "req_def456".to_string(),
            request_type: "knowledge".to_string(),
            title: "Promote ADR-042 to company layer".to_string(),
            requestor: "bob".to_string(),
            layer: "company".to_string(),
            created_at: "2024-01-15T08:15:00Z".to_string(),
            approvals: 0,
            required_approvals: 2,
            status: "pending".to_string()
        },
        PendingRequest {
            id: "req_ghi789".to_string(),
            request_type: "memory".to_string(),
            title: "Promote high-value learning to team".to_string(),
            requestor: "agent_codex".to_string(),
            layer: "team".to_string(),
            created_at: "2024-01-15T06:00:00Z".to_string(),
            approvals: 1,
            required_approvals: 1,
            status: "ready".to_string()
        }
    ];

    // Apply filters
    let filtered: Vec<_> = requests
        .iter()
        .filter(|r| args.request_type == "all" || r.request_type == args.request_type)
        .filter(|r| args.layer.as_ref().is_none_or(|l| &r.layer == l))
        .filter(|r| {
            args.requestor
                .as_ref()
                .is_none_or(|req| &r.requestor == req)
        })
        .collect();

    if args.json {
        let output = json!({
            "total": filtered.len(),
            "requests": filtered.iter().map(|r| json!({
                "id": r.id,
                "type": r.request_type,
                "title": r.title,
                "requestor": r.requestor,
                "layer": r.layer,
                "created_at": r.created_at,
                "approvals": r.approvals,
                "required_approvals": r.required_approvals,
                "status": r.status,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Pending Requests ({})", filtered.len()));
        println!();

        if filtered.is_empty() {
            println!("  ✓ No pending requests matching your filters");
            println!();
        } else {
            for req in &filtered {
                let status_icon = match req.status.as_str() {
                    "ready" => "✓",
                    "pending" => "○",
                    _ => "?"
                };

                println!(
                    "  {} [{}] {} ({})",
                    status_icon, req.id, req.title, req.request_type
                );
                println!(
                    "      Requestor: {}  |  Layer: {}  |  Approvals: {}/{}",
                    req.requestor, req.layer, req.approvals, req.required_approvals
                );
                println!("      Created: {}", req.created_at);
                println!();
            }

            output::hint(
                "Use 'aeterna govern approve <id>' or 'aeterna govern reject <id>' to act"
            );
        }
    }

    Ok(())
}

async fn run_approve(args: GovernApproveArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _ctx = resolver.resolve()?;

    // Simulated request lookup
    let request = PendingRequest {
        id: args.request_id.clone(),
        request_type: "policy".to_string(),
        title: "Add security-baseline policy".to_string(),
        requestor: "alice".to_string(),
        layer: "org".to_string(),
        created_at: "2024-01-15T10:30:00Z".to_string(),
        approvals: 1,
        required_approvals: 2,
        status: "pending".to_string()
    };

    if args.json {
        let output = json!({
            "success": true,
            "request_id": args.request_id,
            "action": "approved",
            "comment": args.comment,
            "new_approval_count": request.approvals + 1,
            "required_approvals": request.required_approvals,
            "fully_approved": request.approvals + 1 >= request.required_approvals,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Approve Request");
        println!();

        println!("  Request ID: {}", request.id);
        println!("  Type:       {}", request.request_type);
        println!("  Title:      {}", request.title);
        println!("  Requestor:  {}", request.requestor);
        println!("  Layer:      {}", request.layer);
        println!();

        if !args.yes {
            // In real implementation, would prompt for confirmation
            println!("  ℹ Would prompt for confirmation (use --yes to skip)");
        }

        println!("  ✓ Request approved");
        if let Some(comment) = &args.comment {
            println!("    Comment: {comment}");
        }
        println!();

        let new_count = request.approvals + 1;
        if new_count >= request.required_approvals {
            println!(
                "  ⚡ Request is now fully approved ({}/{})",
                new_count, request.required_approvals
            );
            println!("    The change will be applied automatically.");
        } else {
            println!(
                "  ○ Approval recorded ({}/{})",
                new_count, request.required_approvals
            );
            println!(
                "    Waiting for {} more approval(s).",
                request.required_approvals - new_count
            );
        }
        println!();
    }

    Ok(())
}

async fn run_reject(args: GovernRejectArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _ctx = resolver.resolve()?;

    if args.reason.is_empty() {
        ux_error::UxError::new("Rejection reason is required")
            .why("Requestors need feedback to understand why their request was rejected")
            .fix("Provide a reason using the --reason flag")
            .suggest(format!(
                "aeterna govern reject {} --reason \"Need security review first\"",
                args.request_id
            ))
            .display();
        std::process::exit(1);
    }

    // Simulated request lookup
    let request = PendingRequest {
        id: args.request_id.clone(),
        request_type: "policy".to_string(),
        title: "Add security-baseline policy".to_string(),
        requestor: "alice".to_string(),
        layer: "org".to_string(),
        created_at: "2024-01-15T10:30:00Z".to_string(),
        approvals: 1,
        required_approvals: 2,
        status: "pending".to_string()
    };

    if args.json {
        let output = json!({
            "success": true,
            "request_id": args.request_id,
            "action": "rejected",
            "reason": args.reason,
            "requestor_notified": true,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Reject Request");
        println!();

        println!("  Request ID: {}", request.id);
        println!("  Type:       {}", request.request_type);
        println!("  Title:      {}", request.title);
        println!("  Requestor:  {}", request.requestor);
        println!();

        if !args.yes {
            // In real implementation, would prompt for confirmation
            println!("  ℹ Would prompt for confirmation (use --yes to skip)");
        }

        println!("  ✗ Request rejected");
        println!("    Reason: {}", args.reason);
        println!();
        println!("  ℹ Requestor '{}' has been notified", request.requestor);
        println!();
    }

    Ok(())
}

async fn run_configure(args: GovernConfigureArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _ctx = resolver.resolve()?;

    if args.list_templates {
        let templates = [
            (
                "standard",
                "Balanced governance with quorum-based approvals (2 approvers, 72h timeout)",
                "quorum",
                2u32,
                72u32,
                false
            ),
            (
                "strict",
                "Maximum control with unanimous approvals (3+ approvers, 24h timeout, no \
                 auto-approve)",
                "unanimous",
                3,
                24,
                false
            ),
            (
                "permissive",
                "Minimal friction with single approvals (1 approver, auto-approve low-risk)",
                "single",
                1,
                168,
                true
            )
        ];

        if args.json {
            let output: Vec<_> = templates
                .iter()
                .map(|(name, desc, mode, approvers, timeout, auto)| {
                    serde_json::json!({
                        "name": name,
                        "description": desc,
                        "settings": {
                            "approval_mode": mode,
                            "min_approvers": approvers,
                            "timeout_hours": timeout,
                            "auto_approve_low_risk": auto,
                        }
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Available Governance Templates");
            println!();

            for (name, desc, mode, approvers, timeout, auto) in templates {
                println!("  \x1b[1m{name}\x1b[0m - {desc}");
                println!("    Approval Mode:  {mode}");
                println!("    Min Approvers:  {approvers}");
                println!("    Timeout:        {timeout} hours");
                println!(
                    "    Auto-approve:   {}",
                    if auto { "yes (low-risk)" } else { "no" }
                );
                println!();
            }

            output::hint("Use --template <name> to apply a template");
        }
        return Ok(());
    }

    let mut config = GovernanceConfig {
        approval_mode: "quorum".to_string(),
        min_approvers: 2,
        timeout_hours: 72,
        auto_approve_enabled: true,
        escalation_contact: Some("security-team@acme.com".to_string())
    };

    if let Some(ref cli_template) = args.template {
        match cli_template {
            GovernanceTemplate::Standard => {
                config.approval_mode = "quorum".to_string();
                config.min_approvers = 2;
                config.timeout_hours = 72;
                config.auto_approve_enabled = false;
            }
            GovernanceTemplate::Strict => {
                config.approval_mode = "unanimous".to_string();
                config.min_approvers = 3;
                config.timeout_hours = 24;
                config.auto_approve_enabled = false;
            }
            GovernanceTemplate::Permissive => {
                config.approval_mode = "single".to_string();
                config.min_approvers = 1;
                config.timeout_hours = 168;
                config.auto_approve_enabled = true;
            }
        }
    }

    let has_changes = args.approval_mode.is_some()
        || args.min_approvers.is_some()
        || args.timeout_hours.is_some()
        || args.auto_approve.is_some()
        || args.escalation_contact.is_some()
        || args.template.is_some();

    if args.show || !has_changes {
        // Just show current config
        if args.json {
            let output = json!({
                "approval_mode": config.approval_mode,
                "min_approvers": config.min_approvers,
                "timeout_hours": config.timeout_hours,
                "auto_approve_enabled": config.auto_approve_enabled,
                "escalation_contact": config.escalation_contact,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Governance Configuration");
            println!();

            println!("  Approval Mode:       {}", config.approval_mode);
            println!("  Min Approvers:       {}", config.min_approvers);
            println!("  Timeout:             {} hours", config.timeout_hours);
            println!(
                "  Auto-approve:        {}",
                if config.auto_approve_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  Escalation Contact:  {}",
                config.escalation_contact.as_deref().unwrap_or("(not set)")
            );
            println!();
            output::hint("Use --approval-mode, --min-approvers, etc. to change settings");
        }
        return Ok(());
    }

    // Apply changes
    let mut changes: Vec<String> = Vec::new();

    if let Some(mode) = args.approval_mode {
        let mode_str = match mode {
            ApprovalMode::Single => "single",
            ApprovalMode::Quorum => "quorum",
            ApprovalMode::Unanimous => "unanimous"
        };
        changes.push(format!(
            "approval_mode: {} → {}",
            config.approval_mode, mode_str
        ));
        config.approval_mode = mode_str.to_string();
    }

    if let Some(min) = args.min_approvers {
        changes.push(format!("min_approvers: {} → {}", config.min_approvers, min));
        config.min_approvers = min;
    }

    if let Some(timeout) = args.timeout_hours {
        changes.push(format!(
            "timeout_hours: {} → {}",
            config.timeout_hours, timeout
        ));
        config.timeout_hours = timeout;
    }

    if let Some(auto) = args.auto_approve {
        changes.push(format!(
            "auto_approve: {} → {}",
            config.auto_approve_enabled, auto
        ));
        config.auto_approve_enabled = auto;
    }

    if let Some(contact) = args.escalation_contact {
        changes.push(format!(
            "escalation_contact: {} → {}",
            config.escalation_contact.as_deref().unwrap_or("(none)"),
            contact
        ));
        config.escalation_contact = Some(contact);
    }

    if args.json {
        let output = json!({
            "success": true,
            "changes": changes,
            "new_config": {
                "approval_mode": config.approval_mode,
                "min_approvers": config.min_approvers,
                "timeout_hours": config.timeout_hours,
                "auto_approve_enabled": config.auto_approve_enabled,
                "escalation_contact": config.escalation_contact,
            },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Update Governance Configuration");
        println!();

        output::subheader("Changes Applied");
        for change in &changes {
            println!("  ✓ {change}");
        }
        println!();

        println!("  Configuration updated successfully.");
        println!();
    }

    Ok(())
}

async fn run_roles(args: GovernRolesArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _ctx = resolver.resolve()?;

    match args.action.as_str() {
        "list" => {
            // Simulated role assignments
            let roles = vec![
                RoleAssignment {
                    principal: "alice".to_string(),
                    principal_type: "user".to_string(),
                    role: "admin".to_string(),
                    scope: "company:acme".to_string()
                },
                RoleAssignment {
                    principal: "bob".to_string(),
                    principal_type: "user".to_string(),
                    role: "architect".to_string(),
                    scope: "org:platform".to_string()
                },
                RoleAssignment {
                    principal: "charlie".to_string(),
                    principal_type: "user".to_string(),
                    role: "techlead".to_string(),
                    scope: "team:api".to_string()
                },
                RoleAssignment {
                    principal: "agent_codex".to_string(),
                    principal_type: "agent".to_string(),
                    role: "developer".to_string(),
                    scope: "project:payments".to_string()
                },
            ];

            if args.json {
                let output = json!({
                    "roles": roles.iter().map(|r| json!({
                        "principal": r.principal,
                        "principal_type": r.principal_type,
                        "role": r.role,
                        "scope": r.scope,
                    })).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header("Role Assignments");
                println!();

                println!("  {:<20} {:<10} {:<12} Scope", "Principal", "Type", "Role");
                println!("  {}", "-".repeat(60));

                for role in &roles {
                    println!(
                        "  {:<20} {:<10} {:<12} {}",
                        role.principal, role.principal_type, role.role, role.scope
                    );
                }
                println!();

                output::hint(
                    "Use 'aeterna govern roles assign --principal <id> --role <role> --scope \
                     <scope>'"
                );
            }
        }
        "assign" => {
            let principal = args
                .principal
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--principal is required for assign action"))?;
            let role = args
                .role
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--role is required for assign action"))?;
            let scope = args.scope.as_deref().unwrap_or("project");

            if args.json {
                let output = json!({
                    "success": true,
                    "action": "assign",
                    "principal": principal,
                    "role": role,
                    "scope": scope,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header("Assign Role");
                println!();
                println!("  ✓ Assigned role '{role}' to '{principal}' at scope '{scope}'");
                println!();
            }
        }
        "revoke" => {
            let principal = args
                .principal
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--principal is required for revoke action"))?;
            let role = args
                .role
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--role is required for revoke action"))?;

            if args.json {
                let output = json!({
                    "success": true,
                    "action": "revoke",
                    "principal": principal,
                    "role": role,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header("Revoke Role");
                println!();
                println!("  ✓ Revoked role '{role}' from '{principal}'");
                println!();
            }
        }
        _ => {
            ux_error::UxError::new(format!("Unknown roles action: {}", args.action))
                .fix("Use one of: list, assign, revoke")
                .suggest("aeterna govern roles list")
                .display();
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run_audit(args: GovernAuditArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let _ctx = resolver.resolve()?;

    // Simulated audit entries
    let entries = [
        AuditEntry {
            id: "aud_001".to_string(),
            timestamp: "2024-01-15T14:30:00Z".to_string(),
            action: "approve".to_string(),
            actor: "alice".to_string(),
            target_type: "policy".to_string(),
            target_id: "req_abc123".to_string(),
            details: "Approved security-baseline policy".to_string()
        },
        AuditEntry {
            id: "aud_002".to_string(),
            timestamp: "2024-01-15T12:15:00Z".to_string(),
            action: "reject".to_string(),
            actor: "bob".to_string(),
            target_type: "knowledge".to_string(),
            target_id: "req_xyz789".to_string(),
            details: "Rejected: needs more context".to_string()
        },
        AuditEntry {
            id: "aud_003".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            action: "approve".to_string(),
            actor: "system".to_string(),
            target_type: "memory".to_string(),
            target_id: "req_auto001".to_string(),
            details: "Auto-approved low-risk memory feedback".to_string()
        },
        AuditEntry {
            id: "aud_004".to_string(),
            timestamp: "2024-01-14T16:45:00Z".to_string(),
            action: "escalate".to_string(),
            actor: "system".to_string(),
            target_type: "policy".to_string(),
            target_id: "req_esc001".to_string(),
            details: "Escalated due to timeout (72h)".to_string()
        }
    ];

    // Apply filters
    let filtered: Vec<_> = entries
        .iter()
        .filter(|e| args.action == "all" || e.action == args.action)
        .filter(|e| args.actor.as_ref().is_none_or(|a| &e.actor == a))
        .filter(|e| {
            args.target_type
                .as_ref()
                .is_none_or(|t| &e.target_type == t)
        })
        .take(args.limit)
        .collect();

    match args.export {
        ExportFormat::None => {
            if args.json {
                let output = json!({
                    "since": args.since,
                    "total": filtered.len(),
                    "entries": filtered.iter().map(|e| json!({
                        "id": e.id,
                        "timestamp": e.timestamp,
                        "action": e.action,
                        "actor": e.actor,
                        "target_type": e.target_type,
                        "target_id": e.target_id,
                        "details": e.details,
                    })).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header(&format!("Governance Audit Trail (last {})", args.since));
                println!();

                if filtered.is_empty() {
                    println!("  No audit entries matching your filters");
                } else {
                    for entry in &filtered {
                        let icon = match entry.action.as_str() {
                            "approve" => "✓",
                            "reject" => "✗",
                            "escalate" => "↑",
                            "expire" => "⏱",
                            _ => "•"
                        };

                        println!(
                            "  {} [{}] {} by {}",
                            icon,
                            entry.timestamp,
                            entry.action.to_uppercase(),
                            entry.actor
                        );
                        println!(
                            "      {} {} - {}",
                            entry.target_type, entry.target_id, entry.details
                        );
                        println!();
                    }
                }

                output::hint("Use --export json or --export csv to export audit data");
            }
        }
        ExportFormat::Json => {
            let output = json!({
                "exported_at": chrono::Utc::now().to_rfc3339(),
                "since": args.since,
                "entries": filtered.iter().map(|e| json!({
                    "id": e.id,
                    "timestamp": e.timestamp,
                    "action": e.action,
                    "actor": e.actor,
                    "target_type": e.target_type,
                    "target_id": e.target_id,
                    "details": e.details,
                })).collect::<Vec<_>>(),
            });

            if let Some(path) = args.output {
                std::fs::write(&path, serde_json::to_string_pretty(&output)?)?;
                println!("Exported {} entries to {}", filtered.len(), path);
            } else {
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        }
        ExportFormat::Csv => {
            let mut csv = String::from("id,timestamp,action,actor,target_type,target_id,details\n");
            for entry in &filtered {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},\"{}\"\n",
                    entry.id,
                    entry.timestamp,
                    entry.action,
                    entry.actor,
                    entry.target_type,
                    entry.target_id,
                    entry.details.replace('"', "\"\"")
                ));
            }

            if let Some(path) = args.output {
                std::fs::write(&path, &csv)?;
                println!("Exported {} entries to {}", filtered.len(), path);
            } else {
                print!("{csv}");
            }
        }
    }

    Ok(())
}

// Helper types

struct GovernanceStatus {
    approval_mode: String,
    min_approvers: u32,
    timeout_hours: u32,
    auto_approve_enabled: bool,
    pending_requests: u32,
    approved_today: u32,
    rejected_today: u32,
    escalated: u32,
    your_pending_approvals: u32
}

struct PendingRequest {
    id: String,
    request_type: String,
    title: String,
    requestor: String,
    layer: String,
    created_at: String,
    approvals: u32,
    required_approvals: u32,
    status: String
}

struct GovernanceConfig {
    approval_mode: String,
    min_approvers: u32,
    timeout_hours: u32,
    auto_approve_enabled: bool,
    escalation_contact: Option<String>
}

struct RoleAssignment {
    principal: String,
    principal_type: String,
    role: String,
    scope: String
}

struct AuditEntry {
    id: String,
    timestamp: String,
    action: String,
    actor: String,
    target_type: String,
    target_id: String,
    details: String
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_mode_values() {
        let _single = ApprovalMode::Single;
        let _quorum = ApprovalMode::Quorum;
        let _unanimous = ApprovalMode::Unanimous;
    }

    #[test]
    fn test_export_format_values() {
        let _json = ExportFormat::Json;
        let _csv = ExportFormat::Csv;
        let _none = ExportFormat::None;
    }

    #[test]
    fn test_governance_template_values() {
        let _standard = GovernanceTemplate::Standard;
        let _strict = GovernanceTemplate::Strict;
        let _permissive = GovernanceTemplate::Permissive;
    }

    #[test]
    fn test_pending_request_creation() {
        let req = PendingRequest {
            id: "req_123".to_string(),
            request_type: "policy".to_string(),
            title: "Test policy".to_string(),
            requestor: "alice".to_string(),
            layer: "team".to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
            approvals: 1,
            required_approvals: 2,
            status: "pending".to_string()
        };
        assert_eq!(req.id, "req_123");
        assert!(req.approvals < req.required_approvals);
    }

    #[test]
    fn test_pending_request_fully_approved() {
        let req = PendingRequest {
            id: "req_456".to_string(),
            request_type: "knowledge".to_string(),
            title: "Promote ADR".to_string(),
            requestor: "bob".to_string(),
            layer: "org".to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
            approvals: 2,
            required_approvals: 2,
            status: "ready".to_string()
        };
        assert_eq!(req.approvals, req.required_approvals);
        assert_eq!(req.status, "ready");
    }

    #[test]
    fn test_pending_request_types() {
        let policy_req = PendingRequest {
            id: "req_1".to_string(),
            request_type: "policy".to_string(),
            title: "Policy request".to_string(),
            requestor: "alice".to_string(),
            layer: "company".to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
            approvals: 0,
            required_approvals: 1,
            status: "pending".to_string()
        };

        let knowledge_req = PendingRequest {
            id: "req_2".to_string(),
            request_type: "knowledge".to_string(),
            title: "Knowledge request".to_string(),
            requestor: "bob".to_string(),
            layer: "org".to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
            approvals: 0,
            required_approvals: 2,
            status: "pending".to_string()
        };

        let memory_req = PendingRequest {
            id: "req_3".to_string(),
            request_type: "memory".to_string(),
            title: "Memory request".to_string(),
            requestor: "agent_1".to_string(),
            layer: "team".to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
            approvals: 0,
            required_approvals: 1,
            status: "pending".to_string()
        };

        assert_eq!(policy_req.request_type, "policy");
        assert_eq!(knowledge_req.request_type, "knowledge");
        assert_eq!(memory_req.request_type, "memory");
    }

    #[test]
    fn test_role_assignment_creation() {
        let role = RoleAssignment {
            principal: "alice".to_string(),
            principal_type: "user".to_string(),
            role: "admin".to_string(),
            scope: "company:acme".to_string()
        };
        assert_eq!(role.role, "admin");
    }

    #[test]
    fn test_role_assignment_agent_type() {
        let role = RoleAssignment {
            principal: "agent_codex".to_string(),
            principal_type: "agent".to_string(),
            role: "developer".to_string(),
            scope: "project:payments".to_string()
        };
        assert_eq!(role.principal_type, "agent");
        assert_eq!(role.role, "developer");
    }

    #[test]
    fn test_role_assignment_all_roles() {
        let roles = ["admin", "architect", "techlead", "developer"];
        for role_name in roles {
            let role = RoleAssignment {
                principal: "user".to_string(),
                principal_type: "user".to_string(),
                role: role_name.to_string(),
                scope: "company:test".to_string()
            };
            assert_eq!(role.role, role_name);
        }
    }

    #[test]
    fn test_role_assignment_all_scopes() {
        let scopes = [
            "company:acme",
            "org:platform",
            "team:api",
            "project:payments"
        ];
        for scope in scopes {
            let role = RoleAssignment {
                principal: "alice".to_string(),
                principal_type: "user".to_string(),
                role: "developer".to_string(),
                scope: scope.to_string()
            };
            assert_eq!(role.scope, scope);
        }
    }

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry {
            id: "aud_001".to_string(),
            timestamp: "2024-01-15T10:00:00Z".to_string(),
            action: "approve".to_string(),
            actor: "alice".to_string(),
            target_type: "policy".to_string(),
            target_id: "req_123".to_string(),
            details: "Approved".to_string()
        };
        assert_eq!(entry.action, "approve");
    }

    #[test]
    fn test_audit_entry_all_actions() {
        let actions = ["approve", "reject", "escalate", "expire"];
        for action in actions {
            let entry = AuditEntry {
                id: format!("aud_{}", action),
                timestamp: "2024-01-15T10:00:00Z".to_string(),
                action: action.to_string(),
                actor: "system".to_string(),
                target_type: "policy".to_string(),
                target_id: "req_123".to_string(),
                details: format!("Action: {}", action)
            };
            assert_eq!(entry.action, action);
        }
    }

    #[test]
    fn test_audit_entry_target_types() {
        let target_types = ["policy", "knowledge", "memory"];
        for target_type in target_types {
            let entry = AuditEntry {
                id: "aud_001".to_string(),
                timestamp: "2024-01-15T10:00:00Z".to_string(),
                action: "approve".to_string(),
                actor: "alice".to_string(),
                target_type: target_type.to_string(),
                target_id: "req_123".to_string(),
                details: "Approved".to_string()
            };
            assert_eq!(entry.target_type, target_type);
        }
    }

    #[test]
    fn test_governance_status_creation() {
        let status = GovernanceStatus {
            approval_mode: "quorum".to_string(),
            min_approvers: 2,
            timeout_hours: 72,
            auto_approve_enabled: true,
            pending_requests: 3,
            approved_today: 7,
            rejected_today: 1,
            escalated: 0,
            your_pending_approvals: 2
        };
        assert_eq!(status.approval_mode, "quorum");
        assert_eq!(status.min_approvers, 2);
        assert!(status.auto_approve_enabled);
    }

    #[test]
    fn test_governance_status_strict_mode() {
        let status = GovernanceStatus {
            approval_mode: "unanimous".to_string(),
            min_approvers: 3,
            timeout_hours: 24,
            auto_approve_enabled: false,
            pending_requests: 5,
            approved_today: 2,
            rejected_today: 3,
            escalated: 1,
            your_pending_approvals: 4
        };
        assert_eq!(status.approval_mode, "unanimous");
        assert!(!status.auto_approve_enabled);
        assert!(status.escalated > 0);
    }

    #[test]
    fn test_governance_config_creation() {
        let config = GovernanceConfig {
            approval_mode: "quorum".to_string(),
            min_approvers: 2,
            timeout_hours: 72,
            auto_approve_enabled: true,
            escalation_contact: Some("security-team@acme.com".to_string())
        };
        assert_eq!(config.approval_mode, "quorum");
        assert!(config.escalation_contact.is_some());
    }

    #[test]
    fn test_governance_config_no_escalation() {
        let config = GovernanceConfig {
            approval_mode: "single".to_string(),
            min_approvers: 1,
            timeout_hours: 168,
            auto_approve_enabled: true,
            escalation_contact: None
        };
        assert!(config.escalation_contact.is_none());
    }

    #[test]
    fn test_govern_status_args_defaults() {
        let args = GovernStatusArgs {
            json: false,
            verbose: false
        };
        assert!(!args.json);
        assert!(!args.verbose);
    }

    #[test]
    fn test_govern_pending_args_defaults() {
        let args = GovernPendingArgs {
            request_type: "all".to_string(),
            layer: None,
            requestor: None,
            mine: false,
            json: false
        };
        assert_eq!(args.request_type, "all");
        assert!(args.layer.is_none());
    }

    #[test]
    fn test_govern_pending_args_with_filters() {
        let args = GovernPendingArgs {
            request_type: "policy".to_string(),
            layer: Some("org".to_string()),
            requestor: Some("alice".to_string()),
            mine: true,
            json: true
        };
        assert_eq!(args.request_type, "policy");
        assert_eq!(args.layer.as_deref(), Some("org"));
        assert!(args.mine);
    }

    #[test]
    fn test_govern_approve_args() {
        let args = GovernApproveArgs {
            request_id: "req_123".to_string(),
            comment: Some("LGTM".to_string()),
            yes: true,
            json: false
        };
        assert_eq!(args.request_id, "req_123");
        assert!(args.comment.is_some());
        assert!(args.yes);
    }

    #[test]
    fn test_govern_reject_args() {
        let args = GovernRejectArgs {
            request_id: "req_456".to_string(),
            reason: "Security concerns".to_string(),
            yes: false,
            json: true
        };
        assert_eq!(args.request_id, "req_456");
        assert!(!args.reason.is_empty());
    }

    #[test]
    fn test_govern_configure_args_show() {
        let args = GovernConfigureArgs {
            show: true,
            template: None,
            list_templates: false,
            approval_mode: None,
            min_approvers: None,
            timeout_hours: None,
            auto_approve: None,
            escalation_contact: None,
            json: false
        };
        assert!(args.show);
        assert!(args.template.is_none());
    }

    #[test]
    fn test_govern_configure_args_with_template() {
        let args = GovernConfigureArgs {
            show: false,
            template: Some(GovernanceTemplate::Strict),
            list_templates: false,
            approval_mode: None,
            min_approvers: None,
            timeout_hours: None,
            auto_approve: None,
            escalation_contact: None,
            json: false
        };
        assert!(args.template.is_some());
    }

    #[test]
    fn test_govern_configure_args_with_overrides() {
        let args = GovernConfigureArgs {
            show: false,
            template: None,
            list_templates: false,
            approval_mode: Some(ApprovalMode::Quorum),
            min_approvers: Some(3),
            timeout_hours: Some(48),
            auto_approve: Some(false),
            escalation_contact: Some("ops@example.com".to_string()),
            json: false
        };
        assert!(args.approval_mode.is_some());
        assert_eq!(args.min_approvers, Some(3));
    }

    #[test]
    fn test_govern_roles_args_list() {
        let args = GovernRolesArgs {
            action: "list".to_string(),
            principal: None,
            role: None,
            scope: None,
            json: false
        };
        assert_eq!(args.action, "list");
    }

    #[test]
    fn test_govern_roles_args_assign() {
        let args = GovernRolesArgs {
            action: "assign".to_string(),
            principal: Some("alice".to_string()),
            role: Some("admin".to_string()),
            scope: Some("company:acme".to_string()),
            json: false
        };
        assert_eq!(args.action, "assign");
        assert!(args.principal.is_some());
        assert!(args.role.is_some());
    }

    #[test]
    fn test_govern_roles_args_revoke() {
        let args = GovernRolesArgs {
            action: "revoke".to_string(),
            principal: Some("bob".to_string()),
            role: Some("developer".to_string()),
            scope: None,
            json: true
        };
        assert_eq!(args.action, "revoke");
        assert!(args.json);
    }

    #[test]
    fn test_govern_audit_args_defaults() {
        let args = GovernAuditArgs {
            action: "all".to_string(),
            since: "7d".to_string(),
            actor: None,
            target_type: None,
            limit: 50,
            export: ExportFormat::None,
            output: None,
            json: false
        };
        assert_eq!(args.action, "all");
        assert_eq!(args.since, "7d");
        assert_eq!(args.limit, 50);
    }

    #[test]
    fn test_govern_audit_args_with_filters() {
        let args = GovernAuditArgs {
            action: "approve".to_string(),
            since: "24h".to_string(),
            actor: Some("alice".to_string()),
            target_type: Some("policy".to_string()),
            limit: 100,
            export: ExportFormat::Json,
            output: Some("audit.json".to_string()),
            json: false
        };
        assert_eq!(args.action, "approve");
        assert!(args.actor.is_some());
    }

    #[test]
    fn test_govern_audit_args_csv_export() {
        let args = GovernAuditArgs {
            action: "all".to_string(),
            since: "30d".to_string(),
            actor: None,
            target_type: None,
            limit: 1000,
            export: ExportFormat::Csv,
            output: Some("audit.csv".to_string()),
            json: false
        };
        matches!(args.export, ExportFormat::Csv);
        assert!(args.output.is_some());
    }

    #[test]
    fn test_filter_pending_requests_by_type() {
        let requests = vec![
            PendingRequest {
                id: "req_1".to_string(),
                request_type: "policy".to_string(),
                title: "Policy 1".to_string(),
                requestor: "alice".to_string(),
                layer: "org".to_string(),
                created_at: "2024-01-15T10:00:00Z".to_string(),
                approvals: 0,
                required_approvals: 2,
                status: "pending".to_string()
            },
            PendingRequest {
                id: "req_2".to_string(),
                request_type: "knowledge".to_string(),
                title: "Knowledge 1".to_string(),
                requestor: "bob".to_string(),
                layer: "company".to_string(),
                created_at: "2024-01-15T08:00:00Z".to_string(),
                approvals: 0,
                required_approvals: 2,
                status: "pending".to_string()
            },
        ];

        let filtered: Vec<_> = requests
            .iter()
            .filter(|r| r.request_type == "policy")
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "req_1");
    }

    #[test]
    fn test_filter_pending_requests_by_layer() {
        let requests = vec![
            PendingRequest {
                id: "req_1".to_string(),
                request_type: "policy".to_string(),
                title: "Policy 1".to_string(),
                requestor: "alice".to_string(),
                layer: "org".to_string(),
                created_at: "2024-01-15T10:00:00Z".to_string(),
                approvals: 0,
                required_approvals: 2,
                status: "pending".to_string()
            },
            PendingRequest {
                id: "req_2".to_string(),
                request_type: "policy".to_string(),
                title: "Policy 2".to_string(),
                requestor: "bob".to_string(),
                layer: "company".to_string(),
                created_at: "2024-01-15T08:00:00Z".to_string(),
                approvals: 0,
                required_approvals: 2,
                status: "pending".to_string()
            },
        ];

        let layer_filter = Some("company".to_string());
        let filtered: Vec<_> = requests
            .iter()
            .filter(|r| layer_filter.as_ref().map_or(true, |l| &r.layer == l))
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "req_2");
    }

    #[test]
    fn test_filter_audit_entries_by_action() {
        let entries = vec![
            AuditEntry {
                id: "aud_1".to_string(),
                timestamp: "2024-01-15T14:30:00Z".to_string(),
                action: "approve".to_string(),
                actor: "alice".to_string(),
                target_type: "policy".to_string(),
                target_id: "req_1".to_string(),
                details: "Approved".to_string()
            },
            AuditEntry {
                id: "aud_2".to_string(),
                timestamp: "2024-01-15T12:15:00Z".to_string(),
                action: "reject".to_string(),
                actor: "bob".to_string(),
                target_type: "knowledge".to_string(),
                target_id: "req_2".to_string(),
                details: "Rejected".to_string()
            },
        ];

        let action_filter = "approve";
        let filtered: Vec<_> = entries
            .iter()
            .filter(|e| e.action == action_filter)
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "aud_1");
    }

    #[test]
    fn test_filter_audit_entries_by_actor() {
        let entries = vec![
            AuditEntry {
                id: "aud_1".to_string(),
                timestamp: "2024-01-15T14:30:00Z".to_string(),
                action: "approve".to_string(),
                actor: "alice".to_string(),
                target_type: "policy".to_string(),
                target_id: "req_1".to_string(),
                details: "Approved".to_string()
            },
            AuditEntry {
                id: "aud_2".to_string(),
                timestamp: "2024-01-15T12:15:00Z".to_string(),
                action: "approve".to_string(),
                actor: "bob".to_string(),
                target_type: "knowledge".to_string(),
                target_id: "req_2".to_string(),
                details: "Approved".to_string()
            },
        ];

        let actor_filter = Some("bob".to_string());
        let filtered: Vec<_> = entries
            .iter()
            .filter(|e| actor_filter.as_ref().map_or(true, |a| &e.actor == a))
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].actor, "bob");
    }

    #[test]
    fn test_approval_count_logic() {
        let mut req = PendingRequest {
            id: "req_123".to_string(),
            request_type: "policy".to_string(),
            title: "Test policy".to_string(),
            requestor: "alice".to_string(),
            layer: "team".to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
            approvals: 1,
            required_approvals: 2,
            status: "pending".to_string()
        };

        assert!(req.approvals < req.required_approvals);

        req.approvals += 1;
        assert!(req.approvals >= req.required_approvals);
    }
}
