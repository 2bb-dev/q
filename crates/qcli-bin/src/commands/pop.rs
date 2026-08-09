use anyhow::{anyhow, Result};
use qcli_platform::clipboard::{Clipboard, SystemClipboard};

use super::with_workspace_mut;

pub fn run(id: Option<String>, next: bool, stdout: bool, tab: Option<String>) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    with_workspace_mut(|workspace| {
        let popped = if let Some(id) = id {
            let resolved = workspace.resolve_prompt(&id)?;
            workspace.remove_prompt(resolved)?
        } else {
            let tab_id = workspace.resolve_context_tab(tab.as_deref())?;
            workspace
                .pop_next_unpinned(tab_id)?
                .ok_or_else(|| anyhow!("no unpinned prompts to pop"))?
        };

        if stdout {
            print!("{}", popped.text);
        } else {
            let mut cb = SystemClipboard::new()?;
            cb.set_text(&popped.text)?;
            eprintln!(
                "popped {} ({} chars)",
                popped.id,
                popped.text.chars().count()
            );
        }
        Ok(())
    })
}
