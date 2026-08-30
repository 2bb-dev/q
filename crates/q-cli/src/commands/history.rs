use anyhow::Result;
use chrono::{DateTime, Utc};
use q_core::{search::Query, HistoryEntry};
use serde::Serialize;

use super::{source, with_workspace, with_workspace_mut};

#[derive(Serialize)]
struct HistoryOutput<'a> {
    text: Option<String>,
    source: source::SourceOutput<'a>,
    available: bool,
    created_at: DateTime<Utc>,
}

impl<'a> HistoryOutput<'a> {
    fn from_entry(entry: &'a HistoryEntry) -> Self {
        let resolved = source::resolve(entry.source());
        Self {
            text: resolved.text,
            source: resolved.source,
            available: resolved.available,
            created_at: entry.created_at(),
        }
    }
}

pub fn run(
    json: bool,
    search: Option<String>,
    clear: bool,
    forget: Option<String>,
    workspace: Option<&str>,
) -> Result<()> {
    if clear {
        let forgotten = with_workspace_mut(workspace, |workspace| Ok(workspace.clear_history()))?;
        println!("forgot {}", prompt_count(forgotten));
        return Ok(());
    }
    if let Some(term) = forget {
        let query = Query::new(&term);
        let forgotten = with_workspace_mut(workspace, |workspace| {
            Ok(source::forget_matching(workspace, &query))
        })?;
        println!("forgot {}", prompt_count(forgotten));
        return Ok(());
    }

    with_workspace(workspace, |workspace| {
        let query = Query::new(&search.unwrap_or_default());
        let entries: Vec<_> = workspace
            .history()
            .iter()
            .filter(|entry| query.is_match(&source::searchable_text(entry.source())))
            .collect();
        if json {
            let outputs: Vec<_> = entries.into_iter().map(HistoryOutput::from_entry).collect();
            println!("{}", serde_json::to_string_pretty(&outputs)?);
            return Ok(());
        }
        if entries.is_empty() {
            println!("(no matching prompts)");
            return Ok(());
        }
        for entry in entries {
            let display = match source::read(entry.source()) {
                Ok(text) => condense(&text),
                Err(error) => error.to_string(),
            };
            println!(
                "{} {}",
                entry.created_at().format("%Y-%m-%d %H:%M"),
                display
            );
        }
        Ok(())
    })
}

fn prompt_count(count: usize) -> String {
    match count {
        1 => "1 prompt".to_string(),
        other => format!("{other} prompts"),
    }
}

/// Single line, trimmed to 80 chars, for history display.
fn condense(text: &str) -> String {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.chars().count() <= 80 {
        return condensed;
    }
    let mut clipped: String = condensed.chars().take(77).collect();
    clipped.push_str("...");
    clipped
}

#[cfg(test)]
#[path = "../../tests/unit/commands/history.rs"]
mod tests;
