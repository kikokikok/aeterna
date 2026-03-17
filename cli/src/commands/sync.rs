//! Sync command - Memory-Knowledge synchronization
//!
//! Syncs memory and knowledge systems to ensure consistency:
//! - Promotes mature memories to knowledge (if configured)
//! - Validates memory-knowledge pointers
//! - Refreshes caches and indices

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use context::ContextResolver;

use crate::output;

#[derive(Args)]
pub struct SyncArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Perform dry run without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Force sync even if no changes detected
    #[arg(long, short)]
    pub force: bool,

    /// Sync direction: all, memory-to-knowledge, knowledge-to-memory
    #[arg(long, default_value = "all")]
    pub direction: String,

    /// Show verbose sync details
    #[arg(long, short)]
    pub verbose: bool
}

pub async fn run(args: SyncArgs) -> Result<()> {
    let resolver = ContextResolver::new();
    let ctx = resolver.resolve()?;

    if args.json {
        return run_json(args, &ctx).await;
    }

    output::header("Memory-Knowledge Sync");
    println!();

    // Phase 1: Analyze current state
    output::subheader("Analyzing sync state...");
    println!();

    let tenant = &ctx.tenant_id.value;
    let project = ctx
        .project_id
        .as_ref()
        .map_or("(none)", |p| p.value.as_str());

    println!("  {} {}", "Tenant:".dimmed(), tenant.cyan());
    println!("  {} {}", "Project:".dimmed(), project.cyan());
    println!(
        "  {} {}",
        "Direction:".dimmed(),
        args.direction.to_uppercase().cyan()
    );
    println!();

    // Sync requires a live backend connection for state analysis and execution.
    // We cannot accurately report "Already in sync" or "no planned changes"
    // without querying the real backend — always surface the not-connected state.
    eprintln!();
    eprintln!("{} {}", "error:".red().bold(), "Cannot connect to Aeterna server".white().bold());
    eprintln!("       {}", "The memory/knowledge backend is not running or unreachable".dimmed());
    eprintln!();
    eprintln!("{}", "How to fix:".yellow().bold());
    eprintln!("  1. Start the Aeterna server");
    eprintln!("  2. Check your network connection");
    eprintln!("  3. Verify server URL in .aeterna/context.toml");
    eprintln!();
    Err(server_not_connected_error())
}

async fn run_json(args: SyncArgs, ctx: &context::ResolvedContext) -> Result<()> {
    // All sync operations (including dry-run) require a live backend connection.
    // Without a real connection we cannot report accurate planned changes or sync state.
    let err_output = serde_json::json!({
        "success": false,
        "error": "server_not_connected",
        "context": {
            "tenant_id": ctx.tenant_id.value,
            "project_id": ctx.project_id.as_ref().map(|p| &p.value),
        },
        "direction": args.direction,
        "dry_run": args.dry_run,
        "message": "Aeterna server not connected. Set AETERNA_SERVER_URL and ensure the server is running."
    });
    println!("{}", serde_json::to_string_pretty(&err_output)?);
    Err(server_not_connected_error())
}

struct SyncState {
    memories_pending: u32,
    stale_pointers: u32,
    cache_expired: u32
}

impl SyncState {
    fn is_synced(&self) -> bool {
        self.memories_pending == 0 && self.stale_pointers == 0 && self.cache_expired == 0
    }
}

#[derive(Debug)]
struct SyncResults {
    memories_promoted: u32,
    pointers_refreshed: u32,
    cache_updated: u32,
    errors: u32
}

fn analyze_sync_state(_args: &SyncArgs) -> SyncState {
    // Server not connected: returns zeros as a structural sentinel.
    // Callers MUST NOT use is_synced() on this result to gate real sync decisions.
    SyncState {
        memories_pending: 0,
        stale_pointers: 0,
        cache_expired: 0
    }
}

fn server_not_connected_error() -> anyhow::Error {
    anyhow::anyhow!(
        "Aeterna server not connected. Set AETERNA_SERVER_URL and ensure the server is running.\n         Use --dry-run to preview sync changes without a server connection."
    )
}

fn execute_sync(_args: &SyncArgs, _state: &SyncState) -> anyhow::Result<SyncResults> {
    // Non-dry-run sync requires a live backend connection.
    // Return an error instead of silently reporting zero changes.
    Err(server_not_connected_error())
}

fn print_planned_changes(state: &SyncState) {
    if state.memories_pending > 0 {
        println!(
            "  {} Promote {} memories to knowledge",
            "→".cyan(),
            state.memories_pending
        );
    }
    if state.stale_pointers > 0 {
        println!(
            "  {} Refresh {} stale pointers",
            "→".cyan(),
            state.stale_pointers
        );
    }
    if state.cache_expired > 0 {
        println!(
            "  {} Update {} cache entries",
            "→".cyan(),
            state.cache_expired
        );
    }
    if state.is_synced() {
        println!("  {} No changes needed", "→".cyan());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_is_synced() {
        let state = SyncState {
            memories_pending: 0,
            stale_pointers: 0,
            cache_expired: 0
        };
        assert!(state.is_synced());

        let state = SyncState {
            memories_pending: 1,
            stale_pointers: 0,
            cache_expired: 0
        };
        assert!(!state.is_synced());
    }

    #[test]
    fn test_analyze_sync_state_returns_sentinel() {
        let args = SyncArgs {
            json: false,
            dry_run: false,
            force: false,
            direction: "all".to_string(),
            verbose: false
        };
        // analyze_sync_state returns a zero-valued sentinel (server not connected).
        // The caller must NOT interpret this as "already in sync" —
        // it only means no real data was fetched from the backend.
        let state = analyze_sync_state(&args);
        assert_eq!(state.memories_pending, 0);
        assert_eq!(state.stale_pointers, 0);
        assert_eq!(state.cache_expired, 0);
    }

    #[test]
    fn test_execute_sync_returns_error_when_not_connected() {
        let args = SyncArgs {
            json: false,
            dry_run: false,
            force: false,
            direction: "all".to_string(),
            verbose: false
        };
        let state = SyncState {
            memories_pending: 5,
            stale_pointers: 3,
            cache_expired: 2
        };
        let result = execute_sync(&args, &state);
        assert!(result.is_err(), "execute_sync must fail when server not connected");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not connected"), "error message should mention not connected");
    }
}
