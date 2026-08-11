//! Advisory file locking for safe concurrent access to the queue file.

use std::fs::OpenOptions;
use std::path::Path;

/// An owned advisory lock file.
///
/// Guards borrow this value, so every acquisition is released without leaking
/// memory when the guard is dropped.
pub struct FileLock {
    lock: fd_lock::RwLock<std::fs::File>,
}

impl FileLock {
    /// Open the lock file at `path`. Creates the file if missing.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        Ok(Self {
            lock: fd_lock::RwLock::new(file),
        })
    }

    /// Acquire an exclusive advisory lock, blocking until it is available.
    pub fn write(&mut self) -> std::io::Result<fd_lock::RwLockWriteGuard<'_, std::fs::File>> {
        self.lock.write()
    }
}

#[cfg(test)]
#[path = "../tests/unit/lock.rs"]
mod tests;
