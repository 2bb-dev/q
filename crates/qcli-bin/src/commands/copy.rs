use anyhow::{anyhow, Result};
use qcli_platform::clipboard::{Clipboard, SystemClipboard};

use super::open_queue;

pub fn run(id: Option<String>, next: bool, stdout: bool) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    let (queue, _lock, _path) = open_queue()?;
    let prompt = if let Some(id) = id {
        let resolved = queue.resolve(&id)?;
        queue
            .get(resolved)
            .cloned()
            .ok_or_else(|| anyhow!("prompt missing after resolve"))?
    } else {
        queue
            .peek_next()
            .cloned()
            .ok_or_else(|| anyhow!("queue is empty"))?
    };

    if stdout {
        print!("{}", prompt.text);
        return Ok(());
    }
    let mut cb = SystemClipboard::new()?;
    cb.set_text(&prompt.text)?;
    eprintln!(
        "copied {} ({} chars)",
        prompt.id,
        prompt.text.chars().count()
    );
    Ok(())
}
