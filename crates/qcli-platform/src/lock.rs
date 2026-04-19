//! Advisory file locking for safe concurrent access to the queue file.
//!
//! We `Box::leak` one `RwLock<File>` per acquire so the write guard can be
//! `'static`. This leaks ~8 bytes per invocation — acceptable for a short-
//! lived CLI process that exits immediately after the lock is released.
//! A long-lived TUI process should acquire the lock once and hold it.

use std::fs::OpenOptions;
use std::path::Path;

pub struct FileLock {
    _guard: fd_lock::RwLockWriteGuard<'static, std::fs::File>,
}

impl FileLock {
    /// Acquire an exclusive advisory lock on `path`. Creates the file if missing.
    /// Blocks until the lock is available.
    pub fn acquire(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let lock: &'static mut fd_lock::RwLock<std::fs::File> =
            Box::leak(Box::new(fd_lock::RwLock::new(file)));
        let guard = lock.write()?;
        Ok(FileLock { _guard: guard })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::NamedTempFile;

    #[test]
    fn second_acquire_blocks_until_first_released() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let first = FileLock::acquire(&path).expect("first acquire");

        let path2 = path.clone();
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let _second = FileLock::acquire(&path2).expect("second acquire");
            start.elapsed()
        });

        thread::sleep(Duration::from_millis(150));
        drop(first);

        let elapsed = handle.join().unwrap();
        assert!(
            elapsed >= Duration::from_millis(100),
            "second acquire should have waited, elapsed = {elapsed:?}"
        );
    }
}
