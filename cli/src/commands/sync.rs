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

use crate::offline::{OfflineCliClient, OfflineConfig};
use crate::output;

async fn get_live_client() -> Option<crate::client::AeternaClient> {
    crate::backend::connect()
        .await
        .ok()
        .map(|(client, _)| client)
}

async fn get_offline_client() -> Result<OfflineCliClient> {
    let resolved = crate::profile::load_resolved(None, None)?;
    OfflineCliClient::new(resolved.server_url, OfflineConfig::default())
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

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
    pub verbose: bool,
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

    let Some(_client) = get_live_client().await else {
        let offline = get_offline_client().await?;
        offline.display_cache_age_warning().await;
        let stats = offline.queue_stats().await;

        if !args.dry_run {
            let payload = serde_json::json!({
                "direction": args.direction,
                "force": args.force,
                "verbose": args.verbose,
            });
            let op_id = offline
                .queue_operation("sync", "control-plane", "memory-knowledge", payload)
                .await?;
            output::warn("Server not reachable - queued sync request for later processing");
            println!("  Queue operation: {op_id}");
        } else {
            output::warn("Server not reachable - showing offline sync status only");
        }

        println!(
            "  Offline queue: {}/{} pending",
            stats.pending, stats.max_size
        );
        println!();
        output::hint(
            "Reconnect later and run 'aeterna sync' again to use the live backend when available",
        );
        return Ok(());
    };

    let profile_name = crate::profile::load_resolved(None, None)
        .map(|r| r.profile_name)
        .unwrap_or_else(|_| "default".to_string());
    Err(crate::backend::unsupported("sync", &profile_name))
}

async fn run_json(args: SyncArgs, ctx: &context::ResolvedContext) -> Result<()> {
    if get_live_client().await.is_none() {
        let offline = get_offline_client().await?;
        let stats = offline.queue_stats().await;
        let queued_op = if args.dry_run {
            None
        } else {
            Some(
                offline
                    .queue_operation(
                        "sync",
                        "control-plane",
                        "memory-knowledge",
                        serde_json::json!({
                            "direction": args.direction,
                            "force": args.force,
                            "verbose": args.verbose,
                        }),
                    )
                    .await?,
            )
        };
        let err_output = serde_json::json!({
            "success": true,
            "mode": "offline",
            "context": {
                "tenant_id": ctx.tenant_id.value,
                "project_id": ctx.project_id.as_ref().map(|p| &p.value),
            },
            "direction": args.direction,
            "dry_run": args.dry_run,
            "queued_operation_id": queued_op,
            "queue": {
                "pending": stats.pending,
                "max_size": stats.max_size,
            },
            "message": if args.dry_run {
                "Server not connected; offline sync dry-run completed without queueing changes"
            } else {
                "Server not connected; sync request queued for later processing"
            }
        });
        println!("{}", serde_json::to_string_pretty(&err_output)?);
        return Ok(());
    }

    let profile_name = crate::profile::load_resolved(None, None)
        .map(|r| r.profile_name)
        .unwrap_or_else(|_| "default".to_string());
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "success": false,
            "error": "unsupported",
            "operation": "sync",
            "profile": profile_name,
            "message": "Backend API for 'sync' is not yet available"
        }))?
    );
    Err(anyhow::anyhow!("Backend API for 'sync' not yet available"))
}

struct SyncState {
    memories_pending: u32,
    stale_pointers: u32,
    cache_expired: u32,
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
    errors: u32,
}

fn analyze_sync_state(_args: &SyncArgs) -> SyncState {
    // Server not connected: returns zeros as a structural sentinel.
    // Callers MUST NOT use is_synced() on this result to gate real sync decisions.
    SyncState {
        memories_pending: 0,
        stale_pointers: 0,
        cache_expired: 0,
    }
}

fn server_not_connected_error() -> anyhow::Error {
    anyhow::anyhow!(
        "Aeterna server not connected. Set AETERNA_SERVER_URL or configure an active profile, then ensure the server is running.\n         Use --dry-run to preview sync changes without a server connection."
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
            cache_expired: 0,
        };
        assert!(state.is_synced());

        let state = SyncState {
            memories_pending: 1,
            stale_pointers: 0,
            cache_expired: 0,
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
            verbose: false,
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
            verbose: false,
        };
        let state = SyncState {
            memories_pending: 5,
            stale_pointers: 3,
            cache_expired: 2,
        };
        let result = execute_sync(&args, &state);
        assert!(
            result.is_err(),
            "execute_sync must fail when server not connected"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not connected"),
            "error message should mention not connected"
        );
    }
}
