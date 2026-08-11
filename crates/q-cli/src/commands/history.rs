use anyhow::Result;
use q_core::search::Query;

use super::{with_workspace, with_workspace_mut};

pub fn run(json: bool, search: Option<String>, clear: bool, forget: Option<String>) -> Result<()> {
    if clear {
        let forgotten = with_workspace_mut(|workspace| Ok(workspace.clear_history()))?;
        println!("forgot {}", prompt_count(forgotten));
        return Ok(());
    }
    if let Some(term) = forget {
        let forgotten =
            with_workspace_mut(|workspace| Ok(workspace.forget_history_matching(&term)))?;
        println!("forgot {}", prompt_count(forgotten));
        return Ok(());
    }

    with_workspace(|workspace| {
        let query = Query::new(&search.unwrap_or_default());
        let entries: Vec<_> = workspace
            .history()
            .iter()
            .filter(|entry| query.is_match(&entry.text))
            .collect();
        if json {
            println!("{}", serde_json::to_string_pretty(&entries)?);
            return Ok(());
        }
        if entries.is_empty() {
            println!("(no matching prompts)");
            return Ok(());
        }
        for entry in entries {
            println!(
                "{} {}",
                entry.created_at.format("%Y-%m-%d %H:%M"),
                condense(&entry.text)
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

/// Single line, trimmed to 80 chars, for list display.
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
