use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use mk_core::hints::{HintPreset, OperationHints};

#[derive(Subcommand)]
pub enum HintsCommand {
    #[command(about = "List available presets")]
    List(ListArgs),

    #[command(about = "Explain what a preset does")]
    Explain(ExplainArgs),

    #[command(about = "Parse a hints string and show resolved hints")]
    Parse(ParseArgs),
}

#[derive(Args)]
pub struct ListArgs {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Args)]
pub struct ExplainArgs {
    #[arg(help = "Preset name (minimal, fast, standard, full, offline, agent)")]
    pub preset: String,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Args)]
pub struct ParseArgs {
    #[arg(help = "Hints string to parse (e.g., 'fast,no-llm,verbose')")]
    pub hints: String,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

pub fn run(cmd: HintsCommand) -> Result<()> {
    match cmd {
        HintsCommand::List(args) => list(args),
        HintsCommand::Explain(args) => explain(args),
        HintsCommand::Parse(args) => parse(args),
    }
}

fn list(args: ListArgs) -> Result<()> {
    let presets = [
        (
            "minimal",
            "No LLM, no reasoning - fastest, cheapest. For CI/CD, batch jobs.",
        ),
        (
            "fast",
            "LLM enabled, no reasoning - quick responses. For interactive use.",
        ),
        (
            "standard",
            "Full features enabled - balanced. Default for humans.",
        ),
        (
            "full",
            "Everything on including auto-promote - deep analysis, debugging.",
        ),
        ("offline", "No LLM, no external calls - disconnected work."),
        (
            "agent",
            "Optimized for AI agents - full reasoning, no verbose.",
        ),
    ];

    if args.json {
        let output: Vec<_> = presets
            .iter()
            .map(|(name, desc)| serde_json::json!({"name": name, "description": desc}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{}", "Available Presets".bold().underline());
    println!();

    for (name, desc) in presets {
        println!("  {} - {}", name.cyan().bold(), desc);
    }

    println!();
    println!(
        "{}",
        "Use 'aeterna hints explain <preset>' for detailed settings.".dimmed()
    );

    Ok(())
}

fn explain(args: ExplainArgs) -> Result<()> {
    let preset: HintPreset = args
        .preset
        .parse()
        .map_err(|_| anyhow::anyhow!("Unknown preset: {}", args.preset))?;

    let hints = OperationHints::from_preset(preset);

    if args.json {
        let output = serde_json::json!({
            "preset": args.preset,
            "reasoning": hints.reasoning,
            "multi_hop": hints.multi_hop,
            "summarization": hints.summarization,
            "caching": hints.caching,
            "governance": hints.governance,
            "audit": hints.audit,
            "llm": hints.llm,
            "auto_promote": hints.auto_promote,
            "drift_check": hints.drift_check,
            "graph": hints.graph,
            "cca": hints.cca,
            "a2a": hints.a2a,
            "verbose": hints.verbose
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{} {}", "Preset:".bold(), args.preset.cyan().bold());
    println!();

    print_hint(
        "reasoning",
        hints.reasoning,
        "Enable reflective reasoning (MemR3)",
    );
    print_hint("multi_hop", hints.multi_hop, "Enable multi-hop retrieval");
    print_hint(
        "summarization",
        hints.summarization,
        "Enable memory summarization",
    );
    print_hint("caching", hints.caching, "Enable query result caching");
    print_hint("governance", hints.governance, "Enable policy enforcement");
    print_hint("audit", hints.audit, "Enable audit logging");
    print_hint("llm", hints.llm, "Enable LLM calls");
    print_hint(
        "auto_promote",
        hints.auto_promote,
        "Auto-promote high-reward memories",
    );
    print_hint(
        "drift_check",
        hints.drift_check,
        "Check for knowledge drift",
    );
    print_hint("graph", hints.graph, "Enable graph queries");
    print_hint("cca", hints.cca, "Enable CCA agents");
    print_hint("a2a", hints.a2a, "Enable A2A protocol");
    print_hint("verbose", hints.verbose, "Enable verbose output");

    Ok(())
}

fn parse(args: ParseArgs) -> Result<()> {
    let hints = OperationHints::parse_hint_string(&args.hints);

    if args.json {
        let output = serde_json::json!({
            "input": args.hints,
            "preset": format!("{:?}", hints.preset),
            "reasoning": hints.reasoning,
            "multi_hop": hints.multi_hop,
            "summarization": hints.summarization,
            "caching": hints.caching,
            "governance": hints.governance,
            "audit": hints.audit,
            "llm": hints.llm,
            "auto_promote": hints.auto_promote,
            "drift_check": hints.drift_check,
            "graph": hints.graph,
            "cca": hints.cca,
            "a2a": hints.a2a,
            "verbose": hints.verbose
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{} {}", "Parsed:".bold(), args.hints.cyan());
    println!();

    print_hint("reasoning", hints.reasoning, "");
    print_hint("multi_hop", hints.multi_hop, "");
    print_hint("llm", hints.llm, "");
    print_hint("caching", hints.caching, "");
    print_hint("governance", hints.governance, "");
    print_hint("verbose", hints.verbose, "");

    Ok(())
}

fn print_hint(name: &str, enabled: bool, desc: &str) {
    let status = if enabled { "on".green() } else { "off".red() };

    if desc.is_empty() {
        println!("  {:<14} {}", format!("{name}:"), status);
    } else {
        println!(
            "  {:<14} {} - {}",
            format!("{name}:"),
            status,
            desc.dimmed()
        );
    }
}
