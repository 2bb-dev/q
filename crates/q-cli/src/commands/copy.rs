use std::io::{self, Write};

use anyhow::{anyhow, Result};
use q_platform::clipboard::{Clipboard, SystemClipboard};

use super::{source, with_workspace};

pub fn run(
    id: Option<String>,
    next: bool,
    stdout: bool,
    tab: Option<String>,
    workspace: Option<&str>,
) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    let prompt = with_workspace(workspace, |workspace| {
        if let Some(id) = id {
            let resolved = workspace.resolve_prompt(&id)?;
            workspace
                .get_prompt(resolved)
                .cloned()
                .ok_or_else(|| anyhow!("prompt missing after resolve"))
        } else {
            let tab_id = workspace.resolve_context_tab(tab.as_deref())?;
            workspace
                .tab(tab_id)
                .and_then(|tab| tab.queue().peek_next())
                .cloned()
                .ok_or_else(|| anyhow!("queue is empty"))
        }
    })?;
    let text = source::read(prompt.source())?;

    if stdout {
        let mut stdout = io::stdout().lock();
        stdout.write_all(text.as_bytes())?;
        stdout.flush()?;
        return Ok(());
    }
    let mut cb = SystemClipboard::new()?;
    cb.set_text(&text)?;
    eprintln!("copied {} ({} chars)", prompt.id, text.chars().count());
    Ok(())
}
