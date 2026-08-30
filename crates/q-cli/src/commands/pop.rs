use std::io::{self, Write};

use anyhow::{anyhow, Result};
use q_platform::clipboard::{Clipboard, SystemClipboard};

use super::{source, with_workspace_mut};

pub fn run(id: Option<String>, next: bool, stdout: bool, tab: Option<String>) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    with_workspace_mut(|workspace| {
        let prompt = if let Some(id) = id {
            let resolved = workspace.resolve_prompt(&id)?;
            workspace
                .get_prompt(resolved)
                .cloned()
                .ok_or_else(|| anyhow!("prompt missing after resolve"))?
        } else {
            let tab_id = workspace.resolve_context_tab(tab.as_deref())?;
            workspace
                .tab(tab_id)
                .and_then(|tab| tab.queue().iter().find(|prompt| !prompt.pinned()))
                .cloned()
                .ok_or_else(|| anyhow!("no unpinned prompts to pop"))?
        };
        let text = source::read(prompt.source())?;

        if stdout {
            let mut stdout = io::stdout().lock();
            stdout.write_all(text.as_bytes())?;
            stdout.flush()?;
        } else {
            let mut cb = SystemClipboard::new()?;
            cb.set_text(&text)?;
            eprintln!("popped {} ({} chars)", prompt.id, text.chars().count());
        }

        workspace.remove_prompt(prompt.id)?;
        Ok(())
    })
}
