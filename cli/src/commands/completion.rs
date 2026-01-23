use std::io;

use anyhow::Result;
use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{Shell, generate};

use super::Cli;

#[derive(Args)]
pub struct CompletionArgs {
    #[arg(help = "Shell to generate completions for")]
    pub shell: ShellChoice,
}

#[derive(Clone, ValueEnum)]
pub enum ShellChoice {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

pub fn run(args: CompletionArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let shell = match args.shell {
        ShellChoice::Bash => Shell::Bash,
        ShellChoice::Zsh => Shell::Zsh,
        ShellChoice::Fish => Shell::Fish,
        ShellChoice::PowerShell => Shell::PowerShell,
    };

    generate(shell, &mut cmd, "aeterna", &mut io::stdout());

    Ok(())
}
