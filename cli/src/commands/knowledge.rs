use clap::{Args, Subcommand};
use context::ContextResolver;
use mk_core::hints::{HintPreset, OperationHints};
use serde_json::json;

use crate::output;
use crate::ux_error;

#[derive(Subcommand)]
pub enum KnowledgeCommand {
    #[command(about = "Search knowledge across layers")]
    Search(KnowledgeSearchArgs),

    #[command(about = "Get a specific knowledge entry by path")]
    Get(KnowledgeGetArgs),

    #[command(about = "List knowledge entries in a layer")]
    List(KnowledgeListArgs),

    #[command(about = "Check knowledge constraints")]
    Check(KnowledgeCheckArgs),

    #[command(about = "Propose new knowledge (ADR, Pattern, Policy, Spec)")]
    Propose(KnowledgeProposeArgs)
}

#[derive(Args)]
pub struct KnowledgeSearchArgs {
    /// Search query
    pub query: String,

    /// Maximum number of results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Filter by layers (comma-separated: company, org, team, project)
    #[arg(long)]
    pub layers: Option<String>,

    /// Hints preset (minimal, fast, standard, full, offline, agent)
    #[arg(long)]
    pub preset: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Dry run - don't actually search, just show what would happen
    #[arg(long)]
    pub dry_run: bool
}

#[derive(Args)]
pub struct KnowledgeGetArgs {
    /// Path to the knowledge entry (e.g., "adrs/adr-001.md")
    pub path: String,

    /// Layer to get from (company, org, team, project)
    #[arg(short, long, default_value = "project")]
    pub layer: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct KnowledgeListArgs {
    /// Layer to list from (company, org, team, project)
    #[arg(short, long, default_value = "project")]
    pub layer: String,

    /// Filter by path prefix
    #[arg(long)]
    pub prefix: Option<String>,

    /// Maximum number of results
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool
}

#[derive(Args)]
pub struct KnowledgeCheckArgs {
    /// Context to check (e.g., a file path, dependency, etc.)
    #[arg(short, long)]
    pub context: Option<String>,

    /// Check against a specific policy
    #[arg(long)]
    pub policy: Option<String>,

    /// Check for a specific dependency
    #[arg(long)]
    pub dependency: Option<String>,

    /// Hints preset
    #[arg(long)]
    pub preset: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run
    #[arg(long)]
    pub dry_run: bool
}

#[derive(Args)]
pub struct KnowledgeProposeArgs {
    /// Natural language description of what you want to propose
    pub description: String,

    /// Knowledge type (adr, pattern, policy, spec) - auto-detected if not
    /// specified
    #[arg(short = 't', long)]
    pub knowledge_type: Option<String>,

    /// Target layer (company, org, team, project) - inferred from description
    /// if not specified
    #[arg(short, long)]
    pub layer: Option<String>,

    /// Custom title (auto-generated from description if not specified)
    #[arg(long)]
    pub title: Option<String>,

    /// Submit directly for approval (skip draft review)
    #[arg(long)]
    pub submit: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run - show what would be proposed without creating draft
    #[arg(long)]
    pub dry_run: bool
}

pub async fn run(cmd: KnowledgeCommand) -> anyhow::Result<()> {
    match cmd {
        KnowledgeCommand::Search(args) => run_search(args).await,
        KnowledgeCommand::Get(args) => run_get(args).await,
        KnowledgeCommand::List(args) => run_list(args).await,
        KnowledgeCommand::Check(args) => run_check(args).await,
        KnowledgeCommand::Propose(args) => run_propose(args).await
    }
}

async fn run_search(args: KnowledgeSearchArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

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

    let layers: Vec<String> = args.layers.map_or_else(
        || {
            vec![
                "project".to_string(),
                "team".to_string(),
                "org".to_string(),
                "company".to_string(),
            ]
        },
        |l| l.split(',').map(|s| s.trim().to_lowercase()).collect()
    );

    let valid_layers = ["company", "org", "team", "project"];
    for layer in &layers {
        if !valid_layers.contains(&layer.as_str()) {
            let err = ux_error::invalid_knowledge_layer(layer);
            err.display();
            return Err(anyhow::anyhow!("Invalid layer"));
        }
    }

    if args.dry_run || args.verbose {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "knowledge_search",
                "query": args.query,
                "limit": args.limit,
                "layers": layers,
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                    "projectId": resolved.project_id.as_ref().map(|v| &v.value),
                },
                "hints": {
                    "preset": format!("{}", base_hints.preset),
                    "reasoning": base_hints.reasoning,
                    "multiHop": base_hints.multi_hop,
                    "llm": base_hints.llm,
                    "graph": base_hints.graph,
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Knowledge Search (Dry Run)");
            println!();
            println!("  Query:  {}", args.query);
            println!("  Limit:  {}", args.limit);
            println!("  Layers: {}", layers.join(", "));
            println!();
            output::header("Context");
            println!("  tenant_id:  {}", resolved.tenant_id.value);
            println!("  user_id:    {}", resolved.user_id.value);
            if let Some(project) = &resolved.project_id {
                println!("  project_id: {}", project.value);
            }
            println!();
            output::header("Active Hints");
            println!("  preset:    {}", base_hints.preset);
            println!(
                "  reasoning: {} {}",
                if base_hints.reasoning { "on" } else { "off" },
                hint_effect(base_hints.reasoning, "will use semantic reasoning")
            );
            println!(
                "  llm:       {} {}",
                if base_hints.llm { "on" } else { "off" },
                hint_effect(base_hints.llm, "will use LLM for semantic search")
            );
            println!(
                "  graph:     {} {}",
                if base_hints.graph { "on" } else { "off" },
                hint_effect(base_hints.graph, "will query knowledge graph")
            );
            println!();
            output::header("Search Scope");
            println!("  Knowledge is searched in precedence order:");
            for (i, layer) in layers.iter().enumerate() {
                let desc = match layer.as_str() {
                    "project" => "Project-specific knowledge (ADRs, patterns)",
                    "team" => "Team standards and conventions",
                    "org" => "Organization-wide policies",
                    "company" => "Company global standards",
                    _ => ""
                };
                println!("    {}. {} - {}", i + 1, layer, desc);
            }
            println!();

            if args.dry_run {
                output::info("Dry run mode - no actual search performed.");
                output::info("Remove --dry-run to execute the search.");
            }
        }
        return Ok(());
    }

    let err = ux_error::server_not_connected();
    err.display();
    output::info("Run with --dry-run to see what would happen.");

    Ok(())
}

async fn run_get(args: KnowledgeGetArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let layer = args.layer.to_lowercase();
    let valid_layers = ["company", "org", "team", "project"];
    if !valid_layers.contains(&layer.as_str()) {
        let err = ux_error::invalid_knowledge_layer(&layer);
        err.display();
        return Err(anyhow::anyhow!("Invalid layer"));
    }

    if args.json {
        let output = json!({
            "operation": "knowledge_get",
            "path": args.path,
            "layer": layer,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
                "projectId": resolved.project_id.as_ref().map(|v| &v.value),
            },
            "status": "not_connected",
            "message": "Knowledge repository not connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Knowledge: {} ({})", args.path, layer));
        println!();
        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_list(args: KnowledgeListArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let layer = args.layer.to_lowercase();
    let valid_layers = ["company", "org", "team", "project"];
    if !valid_layers.contains(&layer.as_str()) {
        let err = ux_error::invalid_knowledge_layer(&layer);
        err.display();
        return Err(anyhow::anyhow!("Invalid layer"));
    }

    if args.json {
        let output = json!({
            "operation": "knowledge_list",
            "layer": layer,
            "prefix": args.prefix,
            "limit": args.limit,
            "context": {
                "tenantId": resolved.tenant_id.value,
                "userId": resolved.user_id.value,
                "projectId": resolved.project_id.as_ref().map(|v| &v.value),
            },
            "status": "not_connected",
            "message": "Knowledge repository not connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header(&format!("Knowledge in '{layer}' layer"));
        if let Some(prefix) = &args.prefix {
            println!("  Prefix: {prefix}");
        }
        println!();
        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

async fn run_check(args: KnowledgeCheckArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

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

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "knowledge_check",
                "context": args.context,
                "policy": args.policy,
                "dependency": args.dependency,
                "tenantContext": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                    "projectId": resolved.project_id.as_ref().map(|v| &v.value),
                },
                "hints": {
                    "preset": format!("{}", base_hints.preset),
                    "governance": base_hints.governance,
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Knowledge Check (Dry Run)");
            println!();
            if let Some(ctx) = &args.context {
                println!("  Context:    {ctx}");
            }
            if let Some(policy) = &args.policy {
                println!("  Policy:     {policy}");
            }
            if let Some(dep) = &args.dependency {
                println!("  Dependency: {dep}");
            }
            println!();
            output::header("What Would Be Checked");
            println!("  1. Constraint rules from inherited policies");
            println!("  2. Dependency blocklists (security, licensing)");
            println!("  3. Pattern requirements (code style, architecture)");
            println!();
            println!(
                "  governance: {} {}",
                if base_hints.governance { "on" } else { "off" },
                hint_effect(base_hints.governance, "will enforce policies")
            );
            println!();
            output::info("Dry run mode - no actual check performed.");
            output::info("Remove --dry-run to execute the constraint check.");
        }
        return Ok(());
    }

    if args.json {
        let output = json!({
            "operation": "knowledge_check",
            "context": args.context,
            "policy": args.policy,
            "dependency": args.dependency,
            "status": "not_connected",
            "message": "Knowledge repository not connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Knowledge Check");
        println!();
        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

fn hint_effect(enabled: bool, effect: &str) -> String {
    if enabled {
        format!("({effect})")
    } else {
        String::new()
    }
}

async fn run_propose(args: KnowledgeProposeArgs) -> anyhow::Result<()> {
    let resolver = ContextResolver::new();
    let resolved = resolver.resolve()?;

    let valid_types = ["adr", "pattern", "policy", "spec"];
    let valid_layers = ["company", "org", "team", "project"];

    if let Some(ref kt) = args.knowledge_type {
        let kt_lower = kt.to_lowercase();
        if !valid_types.contains(&kt_lower.as_str()) {
            let err = ux_error::invalid_knowledge_type(&kt_lower, &valid_types);
            err.display();
            return Err(anyhow::anyhow!("Invalid knowledge type"));
        }
    }

    if let Some(ref layer) = args.layer {
        let layer_lower = layer.to_lowercase();
        if !valid_layers.contains(&layer_lower.as_str()) {
            let err = ux_error::invalid_knowledge_layer(&layer_lower);
            err.display();
            return Err(anyhow::anyhow!("Invalid layer"));
        }
    }

    let detected_type = detect_knowledge_type(&args.description);
    let detected_layer = detect_knowledge_layer(&args.description);
    let knowledge_type = args
        .knowledge_type
        .as_ref()
        .map_or_else(|| detected_type.clone(), |s| s.to_lowercase());
    let layer = args
        .layer
        .as_ref()
        .map_or_else(|| detected_layer.clone(), |s| s.to_lowercase());
    let title = args
        .title
        .clone()
        .unwrap_or_else(|| extract_title(&args.description));

    if args.dry_run {
        if args.json {
            let output = json!({
                "dryRun": true,
                "operation": "knowledge_propose",
                "description": args.description,
                "detectedType": detected_type,
                "detectedLayer": detected_layer,
                "effectiveType": knowledge_type,
                "effectiveLayer": layer,
                "title": title,
                "submit": args.submit,
                "context": {
                    "tenantId": resolved.tenant_id.value,
                    "userId": resolved.user_id.value,
                    "projectId": resolved.project_id.as_ref().map(|v| &v.value),
                },
                "governanceRequired": true,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            output::header("Knowledge Propose (Dry Run)");
            println!();
            println!("  Description: {}", truncate(&args.description, 60));
            println!();
            output::header("Auto-Detection");
            println!(
                "  Type:  {} {}",
                detected_type,
                if args.knowledge_type.is_some() {
                    format!("(overridden to {knowledge_type})")
                } else {
                    "(auto-detected)".to_string()
                }
            );
            println!(
                "  Layer: {} {}",
                detected_layer,
                if args.layer.is_some() {
                    format!("(overridden to {layer})")
                } else {
                    "(auto-detected)".to_string()
                }
            );
            println!("  Title: {title}");
            println!();
            output::header("Context");
            println!("  tenant_id:  {}", resolved.tenant_id.value);
            println!("  user_id:    {}", resolved.user_id.value);
            if let Some(project) = &resolved.project_id {
                println!("  project_id: {}", project.value);
            }
            println!();
            output::header("What Would Happen");
            println!("  1. Create a {knowledge_type} draft in {layer} layer");
            println!("  2. Generate structured content from description");
            if args.submit {
                println!("  3. Submit directly for governance approval");
            } else {
                println!("  3. Save as draft for review before submission");
            }
            println!();
            output::info("Dry run mode - no draft created.");
            output::info("Remove --dry-run to create the proposal.");
        }
        return Ok(());
    }

    if !args.yes && !args.dry_run {
        output::warn(&format!(
            "This will create a {knowledge_type} proposal in {layer} layer:"
        ));
        println!("  Title: {title}");
        println!("  Type:  {knowledge_type}");
        println!("  Layer: {layer}");
        if args.submit {
            output::info("The proposal will be submitted directly for approval.");
        } else {
            output::info("The proposal will be saved as a draft for review.");
        }
        output::info("Use --yes to skip this confirmation.");
        return Ok(());
    }

    if args.json {
        let output = json!({
            "operation": "knowledge_propose",
            "description": args.description,
            "type": knowledge_type,
            "layer": layer,
            "title": title,
            "submit": args.submit,
            "status": "not_connected",
            "message": "Knowledge repository not connected"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        output::header("Knowledge Propose");
        println!();
        println!("  Title: {title}");
        println!("  Type:  {knowledge_type}");
        println!("  Layer: {layer}");
        println!();
        let err = ux_error::server_not_connected();
        err.display();
    }

    Ok(())
}

fn detect_knowledge_type(description: &str) -> String {
    let lower = description.to_lowercase();

    if lower.contains("decision")
        || lower.contains("adr")
        || lower.contains("architecture")
        || lower.contains("chose")
        || lower.contains("decided")
    {
        return "adr".to_string();
    }

    if lower.contains("policy")
        || lower.contains("rule")
        || lower.contains("must")
        || lower.contains("require")
        || lower.contains("enforce")
        || lower.contains("block")
        || lower.contains("forbid")
    {
        return "policy".to_string();
    }

    if lower.contains("pattern")
        || lower.contains("approach")
        || lower.contains("best practice")
        || lower.contains("how to")
        || lower.contains("guideline")
    {
        return "pattern".to_string();
    }

    if lower.contains("spec")
        || lower.contains("specification")
        || lower.contains("requirement")
        || lower.contains("shall")
    {
        return "spec".to_string();
    }

    "adr".to_string()
}

fn detect_knowledge_layer(description: &str) -> String {
    let lower = description.to_lowercase();

    if lower.contains("company")
        || lower.contains("enterprise")
        || lower.contains("global")
        || lower.contains("all teams")
        || lower.contains("organization-wide")
    {
        return "company".to_string();
    }

    if lower.contains("org")
        || lower.contains("department")
        || lower.contains("division")
        || lower.contains("business unit")
    {
        return "org".to_string();
    }

    if lower.contains("team")
        || lower.contains("squad")
        || lower.contains("group")
        || lower.contains("our team")
    {
        return "team".to_string();
    }

    "project".to_string()
}

fn extract_title(description: &str) -> String {
    let first_sentence = description.split('.').next().unwrap_or(description);
    let cleaned = first_sentence.trim();
    if cleaned.len() > 60 {
        format!("{}...", &cleaned[..57])
    } else {
        cleaned.to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_effect_enabled() {
        let result = hint_effect(true, "will use semantic reasoning");
        assert_eq!(result, "(will use semantic reasoning)");
    }

    #[test]
    fn test_hint_effect_disabled() {
        let result = hint_effect(false, "will use semantic reasoning");
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
        let result = truncate("hello world this is a long string", 20);
        assert_eq!(result, "hello world this ...");
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn test_extract_title_short() {
        let result = extract_title("Use PostgreSQL for databases");
        assert_eq!(result, "Use PostgreSQL for databases");
    }

    #[test]
    fn test_extract_title_with_period() {
        let result = extract_title("Use PostgreSQL. It has good performance.");
        assert_eq!(result, "Use PostgreSQL");
    }

    #[test]
    fn test_extract_title_long() {
        let long_text = "This is a very long description that should be truncated to fit within \
                         the title limit";
        let result = extract_title(long_text);
        assert!(result.len() <= 60);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_title_empty() {
        let result = extract_title("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_detect_knowledge_type_adr() {
        assert_eq!(detect_knowledge_type("We decided to use PostgreSQL"), "adr");
        assert_eq!(
            detect_knowledge_type("Architecture decision for API"),
            "adr"
        );
        assert_eq!(detect_knowledge_type("ADR: Database selection"), "adr");
        assert_eq!(detect_knowledge_type("We chose React for frontend"), "adr");
    }

    #[test]
    fn test_detect_knowledge_type_policy() {
        assert_eq!(
            detect_knowledge_type("All services must use HTTPS"),
            "policy"
        );
        assert_eq!(
            detect_knowledge_type("Policy: No direct database access"),
            "policy"
        );
        assert_eq!(
            detect_knowledge_type("Require code review before merge"),
            "policy"
        );
        assert_eq!(
            detect_knowledge_type("Block dependencies with CVEs"),
            "policy"
        );
        assert_eq!(detect_knowledge_type("Enforce 80% test coverage"), "policy");
        assert_eq!(detect_knowledge_type("Forbid global state"), "policy");
    }

    #[test]
    fn test_detect_knowledge_type_pattern() {
        assert_eq!(
            detect_knowledge_type("Pattern: Repository abstraction"),
            "pattern"
        );
        assert_eq!(
            detect_knowledge_type("Best practice for logging"),
            "pattern"
        );
        assert_eq!(
            detect_knowledge_type("Approach for error handling"),
            "pattern"
        );
        assert_eq!(detect_knowledge_type("How to implement caching"), "pattern");
        assert_eq!(
            detect_knowledge_type("Guideline for API versioning"),
            "pattern"
        );
    }

    #[test]
    fn test_detect_knowledge_type_spec() {
        assert_eq!(detect_knowledge_type("Spec for API endpoints"), "spec");
        assert_eq!(
            detect_knowledge_type("The system shall support 1000 users"),
            "spec"
        );
    }

    #[test]
    fn test_detect_knowledge_type_default() {
        assert_eq!(detect_knowledge_type("Random text without keywords"), "adr");
    }

    #[test]
    fn test_detect_knowledge_layer_company() {
        assert_eq!(
            detect_knowledge_layer("Company-wide security policy"),
            "company"
        );
        assert_eq!(
            detect_knowledge_layer("Enterprise standard for APIs"),
            "company"
        );
        assert_eq!(detect_knowledge_layer("Global logging format"), "company");
        assert_eq!(detect_knowledge_layer("Apply to all teams"), "company");
        assert_eq!(
            detect_knowledge_layer("Organization-wide code style"),
            "company"
        );
    }

    #[test]
    fn test_detect_knowledge_layer_org() {
        assert_eq!(detect_knowledge_layer("Org level guidelines"), "org");
        assert_eq!(detect_knowledge_layer("Department standard"), "org");
        assert_eq!(detect_knowledge_layer("Division policy"), "org");
        assert_eq!(detect_knowledge_layer("Business unit convention"), "org");
    }

    #[test]
    fn test_detect_knowledge_layer_team() {
        assert_eq!(detect_knowledge_layer("Team coding convention"), "team");
        assert_eq!(detect_knowledge_layer("Squad best practices"), "team");
        assert_eq!(detect_knowledge_layer("Our team uses Rust"), "team");
        assert_eq!(detect_knowledge_layer("Group decision on testing"), "team");
    }

    #[test]
    fn test_detect_knowledge_layer_default() {
        assert_eq!(detect_knowledge_layer("Random text"), "project");
        assert_eq!(
            detect_knowledge_layer("Use PostgreSQL for this service"),
            "project"
        );
    }
}
