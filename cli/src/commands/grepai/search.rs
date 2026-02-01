//! # GrepAI Search Command

use clap::Args;
use serde_json::json;

#[derive(Args)]
pub struct SearchArgs {
    /// Natural language search query
    pub query: String,

    /// Maximum number of results
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Minimum relevance score threshold (0.0-1.0)
    #[arg(short, long, default_value = "0.7")]
    pub threshold: f32,

    /// File path pattern filter (glob)
    #[arg(long)]
    pub file_pattern: Option<String>,

    /// Language filter (rust, python, go, etc.)
    #[arg(long)]
    pub language: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show only file paths (no content)
    #[arg(long)]
    pub files_only: bool,
}

pub async fn handle(args: SearchArgs) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut cmd = Command::new("grepai");
    cmd.arg("search")
        .arg(&args.query)
        .arg("--limit")
        .arg(args.limit.to_string())
        .arg("--threshold")
        .arg(args.threshold.to_string());

    if let Some(pattern) = &args.file_pattern {
        cmd.arg("--file-pattern").arg(pattern);
    }
    if let Some(lang) = &args.language {
        cmd.arg("--language").arg(lang);
    }
    if args.json {
        cmd.arg("--json");
    }

    if !args.json {
        println!("Searching: \"{}\"", args.query);
        if let Some(pattern) = &args.file_pattern {
            println!("File pattern: {}", pattern);
        }
        if let Some(lang) = &args.language {
            println!("Language: {}", lang);
        }
        println!();
    }

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        if args.json {
            let results: serde_json::Value = serde_json::from_str(&stdout)
                .unwrap_or_else(|_| json!({"results": []}));
            println!("{}", serde_json::to_string_pretty(&results)?);
        } else if args.files_only {
            if let Ok(results) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(chunks) = results["results"].as_array() {
                    let mut files = std::collections::HashSet::new();
                    for chunk in chunks {
                        if let Some(file) = chunk["file"].as_str() {
                            files.insert(file);
                        }
                    }
                    for file in files {
                        println!("{}", file);
                    }
                }
            }
        } else {
            print!("{}", stdout);
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Search failed: {}", stderr).into())
    }
}
