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
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::NamedTempFile;

    #[test]
    fn second_acquire_blocks_until_first_released() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let mut first = FileLock::open(&path).expect("open first lock");
        let first_guard = first.write().expect("first acquire");

        let path2 = path.clone();
        let handle = thread::spawn(move || {
            let mut second = FileLock::open(&path2).expect("open second lock");
            let start = Instant::now();
            let _second_guard = second.write().expect("second acquire");
            start.elapsed()
        });

        thread::sleep(Duration::from_millis(150));
        drop(first_guard);

        let elapsed = handle.join().unwrap();
        assert!(
            elapsed >= Duration::from_millis(100),
            "second acquire should have waited, elapsed = {elapsed:?}"
        );
    }

    #[test]
    fn lock_can_be_reopened_repeatedly() {
        let tmp = NamedTempFile::new().unwrap();
        for _ in 0..100 {
            let mut lock = FileLock::open(tmp.path()).expect("open lock");
            let _guard = lock.write().expect("acquire lock");
        }
    }
}
