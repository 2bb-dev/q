use anyhow::Result;

use super::{open_queue, save_queue};

pub fn run(id: &str, pinned: bool) -> Result<()> {
    let (mut queue, _lock, path) = open_queue()?;
    let resolved = queue.resolve(id)?;
    queue.set_pinned(resolved, pinned)?;
    save_queue(&path, &queue)?;
    println!("{} {resolved}", if pinned { "pinned" } else { "unpinned" });
    Ok(())
}
