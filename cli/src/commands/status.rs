use anyhow::Result;
use clap::Args;
use colored::Colorize;
use context::ContextResolver;

#[derive(Args)]
pub struct StatusArgs {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,

    #[arg(long, help = "Show verbose details")]
    pub verbose: bool
}

pub fn run(args: StatusArgs) -> Result<()> {
    let resolver = ContextResolver::new();
    let ctx = resolver.resolve()?;

    if args.json {
        let output = serde_json::json!({
            "tenant_id": {
                "value": ctx.tenant_id.value,
                "source": ctx.tenant_id.source.to_string()
            },
            "user_id": {
                "value": ctx.user_id.value,
                "source": ctx.user_id.source.to_string()
            },
            "org_id": ctx.org_id.as_ref().map(|o| serde_json::json!({
                "value": o.value,
                "source": o.source.to_string()
            })),
            "team_id": ctx.team_id.as_ref().map(|t| serde_json::json!({
                "value": t.value,
                "source": t.source.to_string()
            })),
            "project_id": ctx.project_id.as_ref().map(|p| serde_json::json!({
                "value": p.value,
                "source": p.source.to_string()
            })),
            "agent_id": ctx.agent_id.as_ref().map(|a| serde_json::json!({
                "value": a.value,
                "source": a.source.to_string()
            })),
            "hints": {
                "preset": format!("{:?}", ctx.hints.value.preset),
                "source": ctx.hints.source.to_string()
            },
            "context_root": ctx.context_root.as_ref().map(|p| p.display().to_string()),
            "git_root": ctx.git_root.as_ref().map(|p| p.display().to_string())
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{}", "Aeterna Status".bold().underline());
    println!();

    println!("{}", "Context:".bold());
    print_value(
        "tenant_id",
        &ctx.tenant_id.value,
        &ctx.tenant_id.source.to_string(),
        args.verbose
    );
    print_value(
        "user_id",
        &ctx.user_id.value,
        &ctx.user_id.source.to_string(),
        args.verbose
    );

    if let Some(org) = &ctx.org_id {
        print_value("org_id", &org.value, &org.source.to_string(), args.verbose);
    }

    if let Some(team) = &ctx.team_id {
        print_value(
            "team_id",
            &team.value,
            &team.source.to_string(),
            args.verbose
        );
    }

    if let Some(project) = &ctx.project_id {
        print_value(
            "project_id",
            &project.value,
            &project.source.to_string(),
            args.verbose
        );
    }

    if let Some(agent) = &ctx.agent_id {
        print_value(
            "agent_id",
            &agent.value,
            &agent.source.to_string(),
            args.verbose
        );
    }

    println!();
    println!("{}", "Hints:".bold());
    print_value(
        "preset",
        &format!("{:?}", ctx.hints.value.preset),
        &ctx.hints.source.to_string(),
        args.verbose
    );

    if args.verbose {
        println!("  reasoning:   {}", bool_str(ctx.hints.value.reasoning));
        println!("  multi_hop:   {}", bool_str(ctx.hints.value.multi_hop));
        println!("  llm:         {}", bool_str(ctx.hints.value.llm));
        println!("  caching:     {}", bool_str(ctx.hints.value.caching));
        println!("  governance:  {}", bool_str(ctx.hints.value.governance));
        println!("  audit:       {}", bool_str(ctx.hints.value.audit));
        println!("  graph:       {}", bool_str(ctx.hints.value.graph));
        println!("  cca:         {}", bool_str(ctx.hints.value.cca));
        println!("  a2a:         {}", bool_str(ctx.hints.value.a2a));
        println!("  verbose:     {}", bool_str(ctx.hints.value.verbose));
    }

    println!();
    println!("{}", "Paths:".bold());
    if let Some(root) = &ctx.context_root {
        println!("  context_root: {}", root.display().to_string().dimmed());
    } else {
        println!("  context_root: {}", "(none)".dimmed());
    }
    if let Some(root) = &ctx.git_root {
        println!("  git_root:     {}", root.display().to_string().dimmed());
    } else {
        println!("  git_root:     {}", "(none)".dimmed());
    }

    Ok(())
}

fn print_value(name: &str, value: &str, source: &str, verbose: bool) {
    if verbose {
        println!(
            "  {:<12} {} {}",
            format!("{name}:"),
            value.cyan(),
            format!("({source})").dimmed()
        );
    } else {
        println!("  {:<12} {}", format!("{name}:"), value.cyan());
    }
}

fn bool_str(b: bool) -> colored::ColoredString {
    if b { "on".green() } else { "off".red() }
}
