use anyhow::Result;
use chrono::{DateTime, Utc};
use q_core::{Prompt, PromptId};
use serde::Serialize;

use super::{source, with_workspace};

#[derive(Serialize)]
struct PromptOutput<'a> {
    id: PromptId,
    text: Option<String>,
    source: source::SourceOutput<'a>,
    available: bool,
    pinned: bool,
    created_at: DateTime<Utc>,
}

impl<'a> PromptOutput<'a> {
    fn from_prompt(prompt: &'a Prompt) -> Self {
        let resolved = source::resolve(prompt.source());
        Self {
            id: prompt.id,
            text: resolved.text,
            source: resolved.source,
            available: resolved.available,
            pinned: prompt.pinned(),
            created_at: prompt.created_at,
        }
    }
}

pub fn run(json: bool, tab: Option<String>, workspace: Option<&str>) -> Result<()> {
    with_workspace(workspace, |workspace| {
        let tab_id = workspace.resolve_context_tab(tab.as_deref())?;
        let queue = workspace
            .tab(tab_id)
            .ok_or_else(|| anyhow::anyhow!("tab missing after resolve"))?
            .queue();
        if json {
            let items: Vec<_> = queue.iter().map(PromptOutput::from_prompt).collect();
            println!("{}", serde_json::to_string_pretty(&items)?);
            return Ok(());
        }
        if queue.is_empty() {
            println!("(queue empty)");
            return Ok(());
        }
        for prompt in queue.iter() {
            let marker = if prompt.pinned() { "[P]" } else { "   " };
            let preview = match source::read(prompt.source()) {
                Ok(text) => preview(&text),
                Err(error) => error.to_string(),
            };
            println!("{marker} {} {preview}", prompt.id);
        }
        Ok(())
    })
}

fn preview(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.chars().count() <= 80 {
        first_line.to_string()
    } else {
        let mut preview: String = first_line.chars().take(77).collect();
        preview.push_str("...");
        preview
    }
}
