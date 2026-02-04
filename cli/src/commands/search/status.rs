//! # Code Search Status Command

use clap::Args;

#[derive(Args)]
pub struct StatusArgs {
    /// Project name/path (optional, shows all if not specified)
    #[arg(short, long)]
    pub project: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Watch mode - continuously update status
    #[arg(short, long)]
    pub watch: bool,
}

pub async fn handle(args: StatusArgs) -> anyhow::Result<()> {
    use std::process::Command;

    loop {
        let mut cmd = Command::new("codesearch");
        cmd.arg("status");

        if let Some(project) = &args.project {
            cmd.arg("--project").arg(project);
        }
        if args.json {
            cmd.arg("--json");
        }

        let output = cmd.output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Clear screen in watch mode
            if args.watch && !args.json {
                print!("\x1B[2J\x1B[1;1H"); // ANSI clear screen
                println!("Code Search Status (watching, Ctrl+C to exit)\n");
            }
            
            print!("{}", stdout);
            
            if args.watch {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            } else {
                break;
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Status check failed: {}", stderr));
        }
    }

    Ok(())
}
