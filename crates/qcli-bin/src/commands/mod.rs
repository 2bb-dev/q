pub mod add;
pub mod copy;
pub mod list;
pub mod pin;
pub mod pop;

use anyhow::Result;
use qcli_core::{storage, Queue};
use qcli_platform::lock::FileLock;
use qcli_platform::paths;
use std::path::PathBuf;

/// Lock the queue file, load the queue, and return the bundle.
pub(crate) fn open_queue() -> Result<(Queue, FileLock, PathBuf)> {
    let path = paths::queue_path()?;
    let lock = FileLock::acquire(&path.with_extension("lock"))?;
    let queue = storage::load(&path)?;
    Ok((queue, lock, path))
}

pub(crate) fn save_queue(path: &std::path::Path, queue: &Queue) -> Result<()> {
    storage::save(path, queue)?;
    Ok(())
}
