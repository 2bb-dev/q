mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "q", version, about = "Terminal prompt queue")]
struct Cli {
    /// Act on the named workspace instead of the active one.
    #[arg(long, global = true, value_name = "NAME")]
    workspace: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Add a new prompt to the queue.
    Add {
        /// Prompt text or a .md/.markdown file reference. If omitted, read text from stdin.
        text: Option<String>,
        /// Treat a Markdown-looking positional argument as literal text.
        #[arg(long = "text")]
        literal: bool,
        /// Add as pinned.
        #[arg(long)]
        pin: bool,
        /// Target tab name.
        #[arg(long)]
        tab: String,
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
    /// Search every prompt ever added, including ones already copied away.
    History {
        /// Only show entries containing this text.
        search: Option<String>,
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
        /// Forget the entire prompt history.
        #[arg(long, conflicts_with_all = ["search", "forget"])]
        clear: bool,
        /// Forget every remembered prompt matching this text.
        #[arg(long, conflicts_with = "search", value_name = "TEXT")]
        forget: Option<String>,
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
    /// Remove a prompt without copying it.
    Remove { id: String },
    /// Pin a prompt.
    Pin { id: String },
    /// Unpin a prompt.
    Unpin { id: String },
    /// Launch the TUI.
    Tui,
    /// Manage workspaces.
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// List workspaces. The active one is marked with *.
    List {
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Create a new workspace and switch to it.
    Create { name: String },
    /// Rename a workspace.
    Rename { name: String, new_name: String },
    /// Delete a workspace and all its prompts.
    Delete {
        name: String,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
    /// Switch the active workspace.
    Switch { name: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ws = cli.workspace.as_deref();
    match cli.command {
        Some(Command::Add {
            text,
            literal,
            pin,
            tab,
        }) => commands::add::run(text, literal, pin, &tab, ws),
        Some(Command::List { json, tab }) => commands::list::run(json, tab, ws),
        Some(Command::History {
            search,
            json,
            clear,
            forget,
        }) => commands::history::run(json, search, clear, forget, ws),
        Some(Command::Copy {
            id,
            next,
            stdout,
            tab,
        }) => commands::copy::run(id, next, stdout, tab, ws),
        Some(Command::Pop {
            id,
            next,
            stdout,
            tab,
        }) => commands::pop::run(id, next, stdout, tab, ws),
        Some(Command::Remove { id }) => commands::remove::run(&id, ws),
        Some(Command::Pin { id }) => commands::pin::run(&id, true, ws),
        Some(Command::Unpin { id }) => commands::pin::run(&id, false, ws),
        Some(Command::Tui) | None => commands::tui::run(ws),
        Some(Command::Workspace { action }) => match action {
            WorkspaceAction::List { json } => commands::workspace::list(json),
            WorkspaceAction::Create { name } => commands::workspace::create(&name),
            WorkspaceAction::Rename { name, new_name } => {
                commands::workspace::rename(&name, &new_name)
            }
            WorkspaceAction::Delete { name, yes } => commands::workspace::delete(&name, yes),
            WorkspaceAction::Switch { name } => commands::workspace::switch(&name),
        },
    }
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
