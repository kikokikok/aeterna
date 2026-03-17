use clap::{Args, Subcommand};
use context::ContextResolver;
use mk_core::hints::{HintPreset, OperationHints};
use mk_core::types::{TenantContext, TenantId, UserId};
use serde_json::json;
use std::collections::HashMap;

use crate::backend;
use crate::output;
use crate::ux_error;

use std::str::FromStr;

#[derive(Subcommand)]
pub enum MemoryCommand {
    #[command(about = "Search memories across layers")]
    Search(MemorySearchArgs),

    #[command(about = "Add a new memory")]
    Add(MemoryAddArgs),

    #[command(about = "Delete a memory by ID")]
    Delete(MemoryDeleteArgs),

    #[command(about = "List memories in a layer")]
    List(MemoryListArgs),

    #[command(about = "Show memory details by ID")]
    Show(MemoryShowArgs),

    #[command(about = "Provide feedback on a memory")]
    Feedback(MemoryFeedbackArgs),

    #[command(about = "Promote a memory to a broader layer")]
    Promote(MemoryPromoteArgs),
}

#[derive(Args)]
pub struct MemorySearchArgs {
    /// Search query
    pub query: String,

    /// Maximum number of results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Minimum similarity threshold (0.0-1.0)
    #[arg(short, long, default_value = "0.0")]
    pub threshold: f32,

    /// Filter by layer (agent, user, session, project, team, org, company)
    #[arg(long)]
    pub layer: Option<String>,

    /// Hints preset (minimal, fast, standard, full, offline, agent)
    #[arg(long)]
    pub preset: Option<String>,

    /// Enable reasoning (overrides preset)
    #[arg(long)]
    pub reasoning: Option<bool>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show verbose output (what hints would do)
    #[arg(short, long)]
    pub verbose: bool,

    /// Dry run - don't actually search, just show what would happen
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct MemoryAddArgs {
    /// Memory content to store
    pub content: String,

    /// Layer to store in (agent, user, session, project, team, org, company)
    #[arg(short, long, default_value = "project")]
    pub layer: String,

    /// Tags for the memory (comma-separated)
    #[arg(short, long)]
    pub tags: Option<String>,

    /// Additional metadata as JSON
    #[arg(short, long)]
    pub metadata: Option<String>,

    /// Hints preset
    #[arg(long)]
    pub preset: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run - don't actually store, just show what would happen
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct MemoryDeleteArgs {
    /// Memory ID to delete
    pub memory_id: String,

    /// Layer the memory is in
    #[arg(short, long)]
    pub layer: String,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct MemoryListArgs {
    /// Layer to list (agent, user, session, project, team, org, company)
    #[arg(short, long, default_value = "project")]
    pub layer: String,

    /// Maximum number of results
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct MemoryShowArgs {
    /// Memory ID to show
    pub memory_id: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct MemoryFeedbackArgs {
    /// Memory ID to provide feedback for
    pub memory_id: String,

    /// Layer the memory is in
    #[arg(short, long)]
    pub layer: String,

    /// Feedback type (helpful, irrelevant, outdated, inaccurate, duplicate)
    #[arg(short = 't', long)]
    pub feedback_type: String,

    /// Score (-1.0 to 1.0)
    #[arg(short, long)]
    pub score: f32,

    /// Optional reasoning for the feedback
    #[arg(short, long)]
    pub reasoning: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct MemoryPromoteArgs {
    /// Memory ID to promote
    pub memory_id: String,

    /// Current layer the memory is in
    #[arg(short, long)]
    pub from_layer: String,

    /// Target layer to promote to (must be broader than current)
    #[arg(short, long)]
    pub to_layer: String,

    /// Reason for promotion
    #[arg(short, long)]
    pub reason: Option<String>,

    /// Skip governance approval (requires admin role)
    #[arg(long)]
    pub skip_approval: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run - don't actually promote, just show what would happen
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(cmd: MemoryCommand) -> anyhow::Result<()> {
    match cmd {
        MemoryCommand::Search(args) => run_search(args).await,
        MemoryCommand::Add(args) => run_add(args).await,
        MemoryCommand::Delete(args) => run_delete(args).await,
        MemoryCommand::List(args) => run_list(args).await,
        MemoryCommand::Show(args) => run_show(args).await,
        MemoryCommand::Feedback(args) => run_feedback(args).await,
        MemoryCommand::Promote(args) => run_promote(args).await,
    }
}

async fn run_search(args: MemorySearchArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    // Determine hints
    let base_hints = if let Some(preset_str) = &args.preset {
        let preset: HintPreset = preset_str.parse().map_err(|_| {
            let err = ux_error::invalid_preset(preset_str);
            err.display();
            anyhow::anyhow!("Invalid preset")
        })?;
        OperationHints::from_preset(preset)
    } else {
        resolved.to_hints()
    };

    // Apply overrides
    let hints = if let Some(reasoning) = args.reasoning {
        base_hints.with_reasoning(reasoning)
    } else {
        base_hints
    };

    if args.dry_run || args.verbose {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "memory_search",
                "query": args.query,
                "limit": args.limit,
                "threshold": args.threshold,
                "layer": args.layer,
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                    "projectId": resolved.project_id.as_ref().map(|v| &v.value),
                },
                "hints": {
                    "preset": format!("{}", hints.preset),
                    "reasoning": hints.reasoning,
                    "multiHop": hints.multi_hop,
                    "summarization": hints.summarization,
                    "caching": hints.caching,
                    "llm": hints.llm,
                    "graph": hints.graph,
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Memory Search (Dry Run)");
            println!();
            println!("  Query:     {}", args.query);
            println!("  Limit:     {}", args.limit);
            println!("  Threshold: {}", args.threshold);
            if let Some(layer) = &args.layer {
                println!("  Layer:     {layer}");
            }
            println!();
            output::header("Context");
            println!("  tenant_id:  {}", resolved.tenant_id.value);
            println!("  user_id:    {}", resolved.user_id.value);
            if let Some(project) = &resolved.project_id {
                println!("  project_id: {}", project.value);
            }
            println!();
            output::header("Active Hints");
            println!("  preset:       {}", hints.preset);
            println!(
                "  reasoning:    {} {}",
                if hints.reasoning { "on" } else { "off" },
                hint_effect(hints.reasoning, "will use MemR³ reflective reasoning")
            );
            println!(
                "  multi_hop:    {} {}",
                if hints.multi_hop { "on" } else { "off" },
                hint_effect(hints.multi_hop, "will follow graph relationships")
            );
            println!(
                "  summarization: {} {}",
                if hints.summarization { "on" } else { "off" },
                hint_effect(hints.summarization, "will summarize results")
            );
            println!(
                "  caching:      {} {}",
                if hints.caching { "on" } else { "off" },
                hint_effect(hints.caching, "will cache query results")
            );
            println!(
                "  llm:          {} {}",
                if hints.llm { "on" } else { "off" },
                hint_effect(hints.llm, "will use LLM for embeddings")
            );
            println!(
                "  graph:        {} {}",
                if hints.graph { "on" } else { "off" },
                hint_effect(hints.graph, "will query knowledge graph")
            );
            println!();

            if args.dry_run {
                output::info("Dry run mode - no actual search performed.");
                output::info("Remove --dry-run to execute the search.");
            }
        }
        return Ok(());
    }

    // TODO: Actually perform the search when connected to backend
    // Try direct backend connection
    match try_search_direct(&resolved, &args, hints.reasoning).await {
        Ok(()) => Ok(()),
        Err(e) => {
            output::warn(&format!("Backend connection failed: {}", e));
            let err = ux_error::server_not_connected();
            err.display();
            output::info("Run with --dry-run to see what would happen.");
            Ok(())
        }
    }
}

async fn try_search_direct(
    resolved: &context::ResolvedContext,
    args: &MemorySearchArgs,
    enable_reasoning: bool,
) -> anyhow::Result<()> {
    let manager = backend::create_memory_manager(enable_reasoning)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = TenantContext::new(
        TenantId::from_str(&resolved.tenant_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
        UserId::from_str(&resolved.user_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
    );

    let filters: HashMap<String, serde_json::Value> = if let Some(layer_str) = &args.layer {
        let mut f = HashMap::new();
        f.insert("layer".to_string(), json!(layer_str));
        f
    } else {
        HashMap::new()
    };

    let (results, reasoning_trace) = if enable_reasoning {
        manager
            .search_text_with_reasoning(ctx, &args.query, args.limit, args.threshold, filters, None)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
    } else {
        (
            manager
                .search(ctx, &args.query, args.limit, args.threshold, filters)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            None,
        )
    };

    if args.json {
        let entries: Vec<serde_json::Value> = results
            .iter()
            .map(|entry| {
                json!({
                    "id": entry.id,
                    "content": entry.content,
                    "layer": format!("{:?}", entry.layer),
                    "score": entry.metadata.get("score"),
                    "importanceScore": entry.importance_score,
                    "createdAt": entry.created_at,
                })
            })
            .collect();
        let output = json!({
            "operation": "memory_search",
            "query": args.query,
            "reasoningEnabled": enable_reasoning,
            "reasoningTrace": reasoning_trace,
            "resultCount": results.len(),
            "results": entries,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Memory Search Results");
        println!();
        if results.is_empty() {
            output::info("No matching memories found.");
        } else {
            if let Some(trace) = &reasoning_trace {
                output::header("Reasoning Trace");
                println!("  Strategy:      {}", trace.strategy);
                println!(
                    "  Refined query: {}",
                    trace.refined_query.as_deref().unwrap_or("<unchanged>")
                );
                println!("  Trace:         {}", trace.thought_process);
                println!();
            }
            println!("  Found {} result(s):", results.len());
            println!();
            for (i, entry) in results.iter().enumerate() {
                let score = entry
                    .metadata
                    .get("score")
                    .and_then(|v| v.as_f64())
                    .map(|s| format!("{:.4}", s))
                    .unwrap_or_else(|| "N/A".to_string());
                let content_preview = if entry.content.len() > 80 {
                    format!("{}...", &entry.content[..77])
                } else {
                    entry.content.clone()
                };
                println!("  {}. [{}] (score: {})", i + 1, entry.id, score);
                println!("     Layer: {:?}", entry.layer);
                println!("     {}", content_preview);
                println!();
            }
        }
    }

    Ok(())
}

async fn run_add(args: MemoryAddArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    // Validate layer
    let layer = args.layer.to_lowercase();
    let valid_layers = [
        "agent", "user", "session", "project", "team", "org", "company",
    ];
    if !valid_layers.contains(&layer.as_str()) {
        let err = ux_error::invalid_layer(&layer, &valid_layers);
        err.display();
        return Err(anyhow::anyhow!("Invalid layer"));
    }

    // Parse tags
    let tags: Vec<String> = args
        .tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse metadata
    let metadata: serde_json::Value = if let Some(meta_str) = &args.metadata {
        serde_json::from_str(meta_str).map_err(|e| {
            let err = ux_error::invalid_metadata_json(&e.to_string());
            err.display();
            anyhow::anyhow!("Invalid metadata JSON")
        })?
    } else {
        json!({})
    };

    // Determine hints
    let hints = if let Some(preset_str) = &args.preset {
        let preset: HintPreset = preset_str.parse().map_err(|_| {
            let err = ux_error::invalid_preset(preset_str);
            err.display();
            anyhow::anyhow!("Invalid preset")
        })?;
        OperationHints::from_preset(preset)
    } else {
        resolved.to_hints()
    };

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "memory_add",
                "content": args.content,
                "layer": layer,
                "tags": tags,
                "metadata": metadata,
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                    "projectId": resolved.project_id.as_ref().map(|v| &v.value),
                },
                "hints": {
                    "preset": format!("{}", hints.preset),
                    "governance": hints.governance,
                    "audit": hints.audit,
                    "llm": hints.llm,
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Memory Add (Dry Run)");
            println!();
            println!("  Content: {}", truncate(&args.content, 60));
            println!("  Layer:   {layer}");
            if !tags.is_empty() {
                println!("  Tags:    {}", tags.join(", "));
            }
            if metadata != json!({}) {
                println!("  Metadata: {metadata}");
            }
            println!();
            output::header("Context");
            println!("  tenant_id:  {}", resolved.tenant_id.value);
            println!("  user_id:    {}", resolved.user_id.value);
            if let Some(project) = &resolved.project_id {
                println!("  project_id: {}", project.value);
            }
            println!();
            output::header("Active Hints");
            println!(
                "  governance: {} {}",
                if hints.governance { "on" } else { "off" },
                hint_effect(hints.governance, "will check policies before storing")
            );
            println!(
                "  audit:      {} {}",
                if hints.audit { "on" } else { "off" },
                hint_effect(hints.audit, "will log to audit trail")
            );
            println!(
                "  llm:        {} {}",
                if hints.llm { "on" } else { "off" },
                hint_effect(hints.llm, "will generate embeddings")
            );
            println!();
            output::info("Dry run mode - memory not stored.");
            output::info("Remove --dry-run to store the memory.");
        }
        return Ok(());
    }

    // TODO: Actually store the memory when connected to backend
    // Try direct backend connection
    match try_add_direct(&resolved, &args.content, &layer).await {
        Ok(id) => {
            if args.json {
                let output = json!({
                    "operation": "memory_add",
                    "status": "success",
                    "memoryId": id,
                    "layer": layer,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header("Memory Added");
                println!();
                println!("  Memory ID: {}", id);
                println!("  Layer:     {}", layer);
                println!("  Content:   {}", truncate(&args.content, 60));
                println!();
                output::info("Memory stored successfully.");
            }
            Ok(())
        }
        Err(e) => {
            output::warn(&format!("Backend connection failed: {}", e));
            let err = ux_error::server_not_connected();
            err.display();
            output::info("Run with --dry-run to see what would happen.");
            Ok(())
        }
    }
}

async fn try_add_direct(
    resolved: &context::ResolvedContext,
    content: &str,
    layer_str: &str,
) -> anyhow::Result<String> {
    let manager = backend::create_memory_manager(false)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = TenantContext::new(
        TenantId::from_str(&resolved.tenant_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
        UserId::from_str(&resolved.user_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
    );

    let layer = backend::parse_layer(layer_str).map_err(|e| anyhow::anyhow!("{}", e))?;

    let id = manager
        .add(ctx, content, layer)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(id)
}

async fn run_delete(args: MemoryDeleteArgs) -> anyhow::Result<()> {
    // Validate layer
    let layer = args.layer.to_lowercase();
    let valid_layers = [
        "agent", "user", "session", "project", "team", "org", "company",
    ];
    if !valid_layers.contains(&layer.as_str()) {
        let err = ux_error::invalid_layer(&layer, &valid_layers);
        err.display();
        return Err(anyhow::anyhow!("Invalid layer"));
    }

    if !args.yes {
        output::warn(&format!(
            "This will delete memory '{}' from layer '{}'.",
            args.memory_id, layer
        ));
        output::info("Use --yes to skip this confirmation.");
        return Ok(());
    }

    if args.json {
        match try_delete_direct(&args.memory_id, &layer).await {
            Ok(()) => {
                let output = json!({
                    "operation": "memory_delete",
                    "memoryId": args.memory_id,
                    "layer": layer,
                    "status": "deleted",
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            Err(e) => {
                let output = json!({
                    "operation": "memory_delete",
                    "memoryId": args.memory_id,
                    "layer": layer,
                    "status": "error",
                    "message": format!("{}", e)
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        }
    } else {
        match try_delete_direct(&args.memory_id, &layer).await {
            Ok(()) => {
                output::header("Memory Deleted");
                println!();
                println!("  Memory ID: {}", args.memory_id);
                println!("  Layer:     {}", layer);
                println!();
                output::info("Memory deleted successfully.");
            }
            Err(e) => {
                output::warn(&format!("Backend connection failed: {}", e));
                let err = ux_error::server_not_connected();
                err.display();
            }
        }
    }

    Ok(())
}

async fn try_delete_direct(memory_id: &str, layer_str: &str) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;
    let manager = backend::create_memory_manager(false)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = TenantContext::new(
        TenantId::from_str(&resolved.tenant_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
        UserId::from_str(&resolved.user_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
    );

    let layer = backend::parse_layer(layer_str).map_err(|e| anyhow::anyhow!("{}", e))?;

    manager
        .delete_from_layer(ctx, layer, memory_id)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

async fn run_list(args: MemoryListArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    // Validate layer
    let layer = args.layer.to_lowercase();
    let valid_layers = [
        "agent", "user", "session", "project", "team", "org", "company",
    ];
    if !valid_layers.contains(&layer.as_str()) {
        let err = ux_error::invalid_layer(&layer, &valid_layers);
        err.display();
        return Err(anyhow::anyhow!("Invalid layer"));
    }

    if args.json {
        match try_list_direct(&resolved, &layer, args.limit).await {
            Ok(entries) => {
                let items: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "content": e.content,
                            "layer": format!("{:?}", e.layer),
                            "createdAt": e.created_at,
                            "updatedAt": e.updated_at,
                        })
                    })
                    .collect();
                let output = json!({
                    "operation": "memory_list",
                    "layer": layer,
                    "count": entries.len(),
                    "entries": items,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            Err(e) => {
                let output = json!({
                    "operation": "memory_list",
                    "layer": layer,
                    "status": "error",
                    "message": format!("{}", e)
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        }
    } else {
        output::header(&format!("Memories in '{}' layer", layer));
        println!();
        match try_list_direct(&resolved, &layer, args.limit).await {
            Ok(entries) => {
                if entries.is_empty() {
                    output::info("No memories found in this layer.");
                } else {
                    println!(
                        "  Found {} memor{}:",
                        entries.len(),
                        if entries.len() == 1 { "y" } else { "ies" }
                    );
                    println!();
                    for (i, entry) in entries.iter().enumerate() {
                        let content_preview = if entry.content.len() > 70 {
                            format!("{}...", &entry.content[..67])
                        } else {
                            entry.content.clone()
                        };
                        println!("  {}. [{}]", i + 1, entry.id);
                        println!("     {}", content_preview);
                        println!();
                    }
                }
            }
            Err(e) => {
                output::warn(&format!("Backend connection failed: {}", e));
                let err = ux_error::server_not_connected();
                err.display();
            }
        }
    }

    Ok(())
}

async fn try_list_direct(
    resolved: &context::ResolvedContext,
    layer_str: &str,
    limit: usize,
) -> anyhow::Result<Vec<mk_core::types::MemoryEntry>> {
    let manager = backend::create_memory_manager(false)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = TenantContext::new(
        TenantId::from_str(&resolved.tenant_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
        UserId::from_str(&resolved.user_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
    );

    let layer = backend::parse_layer(layer_str).map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut entries = manager
        .list_all_from_layer(ctx, layer)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    entries.truncate(limit);
    Ok(entries)
}

async fn run_show(args: MemoryShowArgs) -> anyhow::Result<()> {
    match try_show_direct(&args.memory_id).await {
        Ok(Some(entry)) => {
            if args.json {
                let output = json!({
                    "operation": "memory_show",
                    "memoryId": entry.id,
                    "content": entry.content,
                    "layer": format!("{:?}", entry.layer),
                    "createdAt": entry.created_at,
                    "updatedAt": entry.updated_at,
                    "metadata": entry.metadata,
                    "importanceScore": entry.importance_score,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header(&format!("Memory: {}", entry.id));
                println!();
                println!("  Layer:      {:?}", entry.layer);
                println!("  Created:    {}", entry.created_at);
                println!("  Updated:    {}", entry.updated_at);
                if let Some(score) = entry.importance_score {
                    println!("  Importance: {:.4}", score);
                }
                println!();
                println!("  Content:");
                println!("  {}", entry.content);
                println!();
                if !entry.metadata.is_empty() {
                    output::header("Metadata");
                    for (key, value) in &entry.metadata {
                        if key != "score" && key != "tenant_id" && key != "user_id" {
                            println!("  {}: {}", key, value);
                        }
                    }
                }
            }
        }
        Ok(None) => {
            if args.json {
                let output = json!({
                    "operation": "memory_show",
                    "memoryId": args.memory_id,
                    "status": "not_found",
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header(&format!("Memory: {}", args.memory_id));
                println!();
                output::warn("Memory not found.");
            }
        }
        Err(e) => {
            if args.json {
                let output = json!({
                    "operation": "memory_show",
                    "memoryId": args.memory_id,
                    "status": "error",
                    "message": format!("{}", e)
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                output::header(&format!("Memory: {}", args.memory_id));
                println!();
                output::warn(&format!("Backend connection failed: {}", e));
                let err = ux_error::server_not_connected();
                err.display();
            }
        }
    }
    Ok(())
}

async fn try_show_direct(memory_id: &str) -> anyhow::Result<Option<mk_core::types::MemoryEntry>> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;
    let manager = backend::create_memory_manager(false)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = TenantContext::new(
        TenantId::from_str(&resolved.tenant_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
        UserId::from_str(&resolved.user_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
    );

    // Try all layers to find the memory
    let layers = [
        mk_core::types::MemoryLayer::Agent,
        mk_core::types::MemoryLayer::User,
        mk_core::types::MemoryLayer::Session,
        mk_core::types::MemoryLayer::Project,
        mk_core::types::MemoryLayer::Team,
        mk_core::types::MemoryLayer::Org,
        mk_core::types::MemoryLayer::Company,
    ];

    for layer in layers {
        if let Ok(Some(entry)) = manager.get_from_layer(ctx.clone(), layer, memory_id).await {
            return Ok(Some(entry));
        }
    }

    Ok(None)
}

async fn run_feedback(args: MemoryFeedbackArgs) -> anyhow::Result<()> {
    // Validate layer
    let layer = args.layer.to_lowercase();
    let valid_layers = [
        "agent", "user", "session", "project", "team", "org", "company",
    ];
    if !valid_layers.contains(&layer.as_str()) {
        let err = ux_error::invalid_layer(&layer, &valid_layers);
        err.display();
        return Err(anyhow::anyhow!("Invalid layer"));
    }

    // Validate feedback type
    let feedback_type = args.feedback_type.to_lowercase();
    let valid_types = [
        "helpful",
        "irrelevant",
        "outdated",
        "inaccurate",
        "duplicate",
    ];
    if !valid_types.contains(&feedback_type.as_str()) {
        let err = ux_error::invalid_feedback_type(&feedback_type);
        err.display();
        return Err(anyhow::anyhow!("Invalid feedback type"));
    }

    // Validate score
    if args.score < -1.0 || args.score > 1.0 {
        let err = ux_error::invalid_score(args.score);
        err.display();
        return Err(anyhow::anyhow!("Invalid score"));
    }

    if args.json {
        match try_feedback_direct(
            &args.memory_id,
            &layer,
            &feedback_type,
            args.score,
            args.reasoning.clone(),
        )
        .await
        {
            Ok(()) => {
                let output = json!({
                    "operation": "memory_feedback",
                    "memoryId": args.memory_id,
                    "layer": layer,
                    "feedbackType": feedback_type,
                    "score": args.score,
                    "reasoning": args.reasoning,
                    "status": "recorded",
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            Err(e) => {
                let output = json!({
                    "operation": "memory_feedback",
                    "memoryId": args.memory_id,
                    "layer": layer,
                    "feedbackType": feedback_type,
                    "score": args.score,
                    "reasoning": args.reasoning,
                    "status": "error",
                    "message": format!("{}", e)
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        }
    } else {
        output::header("Memory Feedback");
        println!();
        println!("  Memory ID: {}", args.memory_id);
        println!("  Layer:     {layer}");
        println!("  Type:      {feedback_type}");
        println!("  Score:     {}", args.score);
        if let Some(reasoning) = &args.reasoning {
            println!("  Reasoning: {reasoning}");
        }
        println!();
        match try_feedback_direct(
            &args.memory_id,
            &layer,
            &feedback_type,
            args.score,
            args.reasoning.clone(),
        )
        .await
        {
            Ok(()) => {
                output::info("Feedback recorded successfully.");
            }
            Err(e) => {
                output::warn(&format!("Backend connection failed: {}", e));
                let err = ux_error::server_not_connected();
                err.display();
            }
        }
    }

    Ok(())
}

async fn try_feedback_direct(
    memory_id: &str,
    layer_str: &str,
    feedback_type: &str,
    score: f32,
    reasoning: Option<String>,
) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;
    let manager = backend::create_memory_manager(false)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = TenantContext::new(
        TenantId::from_str(&resolved.tenant_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
        UserId::from_str(&resolved.user_id.value).map_err(|e| anyhow::anyhow!("{}", e))?,
    );

    let layer = backend::parse_layer(layer_str).map_err(|e| anyhow::anyhow!("{}", e))?;

    let reward_type = match feedback_type {
        "helpful" => mk_core::types::RewardType::Helpful,
        "irrelevant" => mk_core::types::RewardType::Irrelevant,
        "outdated" => mk_core::types::RewardType::Outdated,
        "inaccurate" => mk_core::types::RewardType::Inaccurate,
        "duplicate" => mk_core::types::RewardType::Duplicate,
        _ => anyhow::bail!("Unsupported feedback type: {}", feedback_type),
    };

    let reward = mk_core::types::RewardSignal {
        reward_type,
        score,
        reasoning,
        agent_id: None,
        timestamp: chrono::Utc::now().timestamp(),
    };

    manager
        .record_reward(ctx, layer, memory_id, reward)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

fn hint_effect(enabled: bool, effect: &str) -> String {
    if enabled {
        format!("({effect})")
    } else {
        String::new()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

fn layer_order(l: &str) -> usize {
    match l {
        "agent" => 0,
        "user" => 1,
        "session" => 2,
        "project" => 3,
        "team" => 4,
        "org" => 5,
        "company" => 6,
        _ => 0,
    }
}

async fn run_promote(args: MemoryPromoteArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let from_layer = args.from_layer.to_lowercase();
    let to_layer = args.to_layer.to_lowercase();
    let valid_layers = [
        "agent", "user", "session", "project", "team", "org", "company",
    ];

    if !valid_layers.contains(&from_layer.as_str()) {
        let err = ux_error::invalid_layer(&from_layer, &valid_layers);
        err.display();
        return Err(anyhow::anyhow!("Invalid source layer"));
    }

    if !valid_layers.contains(&to_layer.as_str()) {
        let err = ux_error::invalid_layer(&to_layer, &valid_layers);
        err.display();
        return Err(anyhow::anyhow!("Invalid target layer"));
    }

    if layer_order(&to_layer) <= layer_order(&from_layer) {
        let err = ux_error::promotion_direction_invalid(&from_layer, &to_layer);
        err.display();
        return Err(anyhow::anyhow!("Cannot promote to same or narrower layer"));
    }

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "memory_promote",
                "memoryId": args.memory_id,
                "fromLayer": from_layer,
                "toLayer": to_layer,
                "reason": args.reason,
                "skipApproval": args.skip_approval,
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                    "projectId": resolved.project_id.as_ref().map(|v| &v.value),
                },
                "governanceRequired": !args.skip_approval,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Memory Promote (Dry Run)");
            println!();
            println!("  Memory ID:   {}", args.memory_id);
            println!("  From Layer:  {from_layer}");
            println!("  To Layer:    {to_layer}");
            if let Some(reason) = &args.reason {
                println!("  Reason:      {reason}");
            }
            println!();
            output::header("Context");
            println!("  tenant_id:  {}", resolved.tenant_id.value);
            println!("  user_id:    {}", resolved.user_id.value);
            if let Some(project) = &resolved.project_id {
                println!("  project_id: {}", project.value);
            }
            println!();
            output::header("Governance");
            if args.skip_approval {
                output::warn("Skipping approval - requires admin role");
            } else {
                println!("  Promotion will require approval based on governance config");
                println!("  Use --skip-approval to bypass (admin only)");
            }
            println!();
            output::info("Dry run mode - no promotion performed.");
            output::info("Remove --dry-run to execute the promotion.");
        }
        return Ok(());
    }

    if !args.yes && !args.dry_run {
        output::warn(&format!(
            "This will promote memory '{}' from '{}' to '{}' layer.",
            args.memory_id, from_layer, to_layer
        ));
        if !args.skip_approval {
            output::info("This action may require governance approval.");
        }
        output::info("Use --yes to skip this confirmation.");
        return Ok(());
    }

    if args.json {
        let output = json!({
            "operation": "memory_promote",
            "memoryId": args.memory_id,
            "fromLayer": from_layer,
            "toLayer": to_layer,
            "reason": args.reason,
            "status": "not_implemented",
            "message": "Promote requires governance service which is not available in local mode"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Memory Promote");
        println!();
        println!("  Memory ID:   {}", args.memory_id);
        println!("  From Layer:  {from_layer}");
        println!("  To Layer:    {to_layer}");
        if let Some(reason) = &args.reason {
            println!("  Reason:      {reason}");
        }
        println!();
        output::warn(
            "Promote requires the governance service, which is not available in local mode.",
        );
        output::info("Use --dry-run to see what would happen.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_effect_enabled() {
        let result = hint_effect(true, "will do something");
        assert_eq!(result, "(will do something)");
    }

    #[test]
    fn test_hint_effect_disabled() {
        let result = hint_effect(false, "will do something");
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_short_string() {
        let result = truncate("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        let result = truncate("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate("hello world this is a long string", 10);
        assert_eq!(result, "hello w...");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn test_truncate_with_unicode() {
        let result = truncate("héllo", 10);
        assert_eq!(result, "héllo");
    }

    #[test]
    fn test_layer_order_all_layers() {
        assert_eq!(layer_order("agent"), 0);
        assert_eq!(layer_order("user"), 1);
        assert_eq!(layer_order("session"), 2);
        assert_eq!(layer_order("project"), 3);
        assert_eq!(layer_order("team"), 4);
        assert_eq!(layer_order("org"), 5);
        assert_eq!(layer_order("company"), 6);
    }

    #[test]
    fn test_layer_order_unknown() {
        assert_eq!(layer_order("unknown"), 0);
        assert_eq!(layer_order(""), 0);
    }

    #[test]
    fn test_layer_hierarchy_promotion_valid() {
        assert!(layer_order("team") > layer_order("project"));
        assert!(layer_order("org") > layer_order("team"));
        assert!(layer_order("company") > layer_order("org"));
    }

    #[test]
    fn test_layer_hierarchy_promotion_invalid() {
        assert!(layer_order("agent") < layer_order("user"));
        assert!(layer_order("project") < layer_order("team"));
    }
}
