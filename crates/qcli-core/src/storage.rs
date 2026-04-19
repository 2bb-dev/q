use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::queue::Queue;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct QueueFile {
    schema: u32,
    queue: Queue,
}

pub fn load(path: &Path) -> Result<Queue> {
    if !path.exists() {
        return Ok(Queue::new());
    }
    let data = fs::read_to_string(path)?;
    if data.trim().is_empty() {
        return Ok(Queue::new());
    }
    let parsed: QueueFile = serde_json::from_str(&data)?;
    Ok(parsed.queue)
}

/// Atomic save: write to `<path>.tmp`, then rename.
pub fn save(path: &Path, queue: &Queue) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let file = QueueFile {
        schema: SCHEMA_VERSION,
        queue: queue.clone(),
    };
    let serialized = serde_json::to_string_pretty(&file)?;
    fs::write(&tmp, serialized)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::Prompt;
    use tempfile::TempDir;

    #[test]
    fn load_missing_file_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        assert!(load(&path).unwrap().is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        let mut q = Queue::new();
        q.add(Prompt::new("hello").unwrap());
        q.add(Prompt::new("world").unwrap());
        save(&path, &q).unwrap();
        let loaded = load(&path).unwrap();
        let a: Vec<_> = q.iter().map(|p| p.text.clone()).collect();
        let b: Vec<_> = loaded.iter().map(|p| p.text.clone()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn save_is_atomic_no_tmp_left_behind() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Queue::new()).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn load_empty_file_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        fs::write(&path, "").unwrap();
        assert!(load(&path).unwrap().is_empty());
    }

    #[test]
    fn schema_version_is_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Queue::new()).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 1"));
    }
}
