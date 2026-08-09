mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "q", version, about = "Terminal prompt queue")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
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
        /// Target tab name. Required when multiple tabs exist.
        #[arg(long)]
        tab: Option<String>,
    },
    /// List all prompts.
    List {
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
        /// Target tab name. Required when multiple tabs exist.
        #[arg(long)]
        tab: Option<String>,
    },
    /// Copy a prompt to the clipboard.
    Copy {
        id: Option<String>,
        #[arg(long, conflicts_with = "id")]
        next: bool,
        #[arg(long)]
        stdout: bool,
        /// Target tab for --next. Required when multiple tabs exist.
        #[arg(long)]
        tab: Option<String>,
    },
    /// Pop a prompt (copy + remove). Pinned prompts are never popped when using --next.
    Pop {
        id: Option<String>,
        #[arg(long, conflicts_with = "id")]
        next: bool,
        #[arg(long)]
        stdout: bool,
        /// Target tab for --next. Required when multiple tabs exist.
        #[arg(long)]
        tab: Option<String>,
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
        Some(Command::Add { text, pin, tab }) => commands::add::run(text, pin, tab),
        Some(Command::List { json, tab }) => commands::list::run(json, tab),
        Some(Command::Copy {
            id,
            next,
            stdout,
            tab,
        }) => commands::copy::run(id, next, stdout, tab),
        Some(Command::Pop {
            id,
            next,
            stdout,
            tab,
        }) => commands::pop::run(id, next, stdout, tab),
        Some(Command::Pin { id }) => commands::pin::run(&id, true),
        Some(Command::Unpin { id }) => commands::pin::run(&id, false),
        Some(Command::Tui) | None => commands::tui::run(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_subcommand_selects_default_tui() {
        let cli = Cli::try_parse_from(["q"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn explicit_tui_remains_available() {
        let cli = Cli::try_parse_from(["q", "tui"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Tui)));
    }
}
