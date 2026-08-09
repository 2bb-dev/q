use anyhow::Result;

use super::with_workspace;

pub fn run(json: bool, tab: Option<String>) -> Result<()> {
    with_workspace(|workspace| {
        let tab_id = workspace.resolve_context_tab(tab.as_deref())?;
        let queue = workspace
            .tab(tab_id)
            .ok_or_else(|| anyhow::anyhow!("tab missing after resolve"))?
            .queue();
        if json {
            let arr: Vec<_> = queue.iter().collect();
            println!("{}", serde_json::to_string_pretty(&arr)?);
            return Ok(());
        }
        if queue.is_empty() {
            println!("(queue empty)");
            return Ok(());
        }
        for prompt in queue.iter() {
            let marker = if prompt.pinned { "[P]" } else { "   " };
            println!("{marker} {} {}", prompt.id, prompt.preview());
        }
        Ok(())
    })
}
