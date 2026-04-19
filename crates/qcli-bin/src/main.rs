mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "q", version, about = "Terminal prompt queue")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a new prompt to the queue.
    Add {
        /// Prompt text. If omitted, read from stdin.
        text: Option<String>,
        /// Add as pinned.
        #[arg(long)]
        pin: bool,
    },
    /// List all prompts.
    List {
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Copy a prompt to the clipboard.
    Copy {
        id: Option<String>,
        #[arg(long, conflicts_with = "id")]
        next: bool,
        #[arg(long)]
        stdout: bool,
    },
    /// Pop a prompt (copy + remove). Pinned prompts are never popped when using --next.
    Pop {
        id: Option<String>,
        #[arg(long, conflicts_with = "id")]
        next: bool,
        #[arg(long)]
        stdout: bool,
    },
    /// Pin a prompt.
    Pin { id: String },
    /// Unpin a prompt.
    Unpin { id: String },
    /// Launch the TUI.
    Tui,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Add { text, pin } => commands::add::run(text, pin),
        Command::List { json } => commands::list::run(json),
        Command::Copy { id, next, stdout } => commands::copy::run(id, next, stdout),
        Command::Pop { id, next, stdout } => commands::pop::run(id, next, stdout),
        Command::Pin { id } => commands::pin::run(&id, true),
        Command::Unpin { id } => commands::pin::run(&id, false),
        Command::Tui => commands::tui::run(),
    }
}
