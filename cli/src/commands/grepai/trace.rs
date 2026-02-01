//! # GrepAI Trace Command

use clap::{Args, Subcommand};

#[derive(Args)]
pub struct TraceArgs {
    #[command(subcommand)]
    pub command: TraceCommand,
}

#[derive(Subcommand)]
pub enum TraceCommand {
    #[command(about = "Find all functions that call a symbol")]
    Callers(CallersArgs),

    #[command(about = "Find all functions called by a symbol")]
    Callees(CalleesArgs),

    #[command(about = "Build full call graph for a symbol")]
    Graph(GraphArgs),
}

#[derive(Args)]
pub struct CallersArgs {
    /// Symbol name to trace (function, method, class)
    pub symbol: String,

    /// File path where symbol is defined (improves accuracy)
    #[arg(short, long)]
    pub file: Option<String>,

    /// Include indirect callers (recursive)
    #[arg(short, long)]
    pub recursive: bool,

    /// Maximum depth for recursive tracing
    #[arg(long, default_value = "3")]
    pub max_depth: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct CalleesArgs {
    /// Symbol name to trace
    pub symbol: String,

    /// File path where symbol is defined
    #[arg(short, long)]
    pub file: Option<String>,

    /// Include indirect callees (recursive)
    #[arg(short, long)]
    pub recursive: bool,

    /// Maximum depth for recursive tracing
    #[arg(long, default_value = "3")]
    pub max_depth: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GraphArgs {
    /// Symbol name to build graph around
    pub symbol: String,

    /// File path where symbol is defined
    #[arg(short, long)]
    pub file: Option<String>,

    /// Graph depth (1 = direct neighbors, 2 = neighbors of neighbors)
    #[arg(short, long, default_value = "2")]
    pub depth: usize,

    /// Include callers in graph
    #[arg(long, default_value = "true")]
    pub include_callers: bool,

    /// Include callees in graph
    #[arg(long, default_value = "true")]
    pub include_callees: bool,

    /// Output format (json, dot, mermaid)
    #[arg(long, default_value = "json")]
    pub format: String,
}

pub async fn handle(args: TraceArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        TraceCommand::Callers(args) => handle_callers(args).await,
        TraceCommand::Callees(args) => handle_callees(args).await,
        TraceCommand::Graph(args) => handle_graph(args).await,
    }
}

async fn handle_callers(args: CallersArgs) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut cmd = Command::new("grepai");
    cmd.arg("trace").arg("callers").arg(&args.symbol);

    if let Some(file) = &args.file {
        cmd.arg("--file").arg(file);
    }
    if args.recursive {
        cmd.arg("--recursive");
        cmd.arg("--max-depth").arg(args.max_depth.to_string());
    }
    if args.json {
        cmd.arg("--json");
    }

    if !args.json {
        println!("Tracing callers of: {}", args.symbol);
        if let Some(file) = &args.file {
            println!("In file: {}", file);
        }
        if args.recursive {
            println!("Recursive (depth: {})", args.max_depth);
        }
        println!();
    }

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        print!("{}", stdout);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Trace failed: {}", stderr).into())
    }
}

async fn handle_callees(args: CalleesArgs) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut cmd = Command::new("grepai");
    cmd.arg("trace").arg("callees").arg(&args.symbol);

    if let Some(file) = &args.file {
        cmd.arg("--file").arg(file);
    }
    if args.recursive {
        cmd.arg("--recursive");
        cmd.arg("--max-depth").arg(args.max_depth.to_string());
    }
    if args.json {
        cmd.arg("--json");
    }

    if !args.json {
        println!("Tracing callees of: {}", args.symbol);
        if let Some(file) = &args.file {
            println!("In file: {}", file);
        }
        if args.recursive {
            println!("Recursive (depth: {})", args.max_depth);
        }
        println!();
    }

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        print!("{}", stdout);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Trace failed: {}", stderr).into())
    }
}

async fn handle_graph(args: GraphArgs) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut cmd = Command::new("grepai");
    cmd.arg("trace")
        .arg("graph")
        .arg(&args.symbol)
        .arg("--depth")
        .arg(args.depth.to_string())
        .arg("--format")
        .arg(&args.format);

    if let Some(file) = &args.file {
        cmd.arg("--file").arg(file);
    }
    if !args.include_callers {
        cmd.arg("--no-callers");
    }
    if !args.include_callees {
        cmd.arg("--no-callees");
    }

    println!("Building call graph for: {}", args.symbol);
    if let Some(file) = &args.file {
        println!("In file: {}", file);
    }
    println!("Depth: {}, Format: {}", args.depth, args.format);
    println!();

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        print!("{}", stdout);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Graph generation failed: {}", stderr).into())
    }
}
