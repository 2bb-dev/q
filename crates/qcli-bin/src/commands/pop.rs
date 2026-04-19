use anyhow::{anyhow, Result};
use qcli_platform::clipboard::{Clipboard, SystemClipboard};

use super::{open_queue, save_queue};

pub fn run(id: Option<String>, next: bool, stdout: bool) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    let (mut queue, _lock, path) = open_queue()?;

    let popped = if let Some(id) = id {
        let resolved = queue.resolve(&id)?;
        queue.remove(resolved)?
    } else {
        queue
            .pop_next_unpinned()
            .ok_or_else(|| anyhow!("no unpinned prompts to pop"))?
    };

    save_queue(&path, &queue)?;

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
}
