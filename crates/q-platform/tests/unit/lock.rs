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
