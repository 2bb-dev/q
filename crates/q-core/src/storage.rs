use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::{Queue, Workspace};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFingerprint {
    modified: SystemTime,
    len: u64,
}

pub fn fingerprint(path: &Path) -> Result<Option<FileFingerprint>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(FileFingerprint {
            modified: metadata.modified()?,
            len: metadata.len(),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Deserialize)]
struct SchemaHeader {
    schema: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueueFileV1 {
    schema: u32,
    queue: Queue,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkspaceFileV2 {
    schema: u32,
    workspace: Workspace,
}

pub fn load(path: &Path) -> Result<Workspace> {
    if !path.exists() {
        return Ok(Workspace::new());
    }
    let data = fs::read_to_string(path)?;
    if data.trim().is_empty() {
        return Ok(Workspace::new());
    }

    let header: SchemaHeader = serde_json::from_str(&data)?;
    let mut workspace = match header.schema {
        1 => {
            let parsed: QueueFileV1 = serde_json::from_str(&data)?;
            Workspace::from_legacy_queue(parsed.queue, Utc::now())
        }
        SCHEMA_VERSION => {
            let parsed: WorkspaceFileV2 = serde_json::from_str(&data)?;
            parsed.workspace
        }
        schema => return Err(CoreError::UnsupportedSchema(schema)),
    };
    workspace.validate_and_normalize()?;
    Ok(workspace)
}

/// Atomic save: write to `<path>.tmp`, then rename.
pub fn save(path: &Path, workspace: &Workspace) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let file = WorkspaceFileV2 {
        schema: SCHEMA_VERSION,
        workspace: workspace.clone(),
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
    fn load_missing_file_returns_initial_workspace() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        let workspace = load(&path).unwrap();
        assert_eq!(workspace.tabs().len(), 1);
        assert_eq!(workspace.tabs()[0].name(), "1");
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        let mut workspace = Workspace::new();
        let first = workspace.first_tab_id();
        workspace
            .add_prompt(first, Prompt::new("hello").unwrap())
            .unwrap();
        let work = workspace.create_tab("work").unwrap();
        workspace
            .add_prompt(work, Prompt::new("world").unwrap())
            .unwrap();

        save(&path, &workspace).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(loaded.tabs().len(), 2);
        assert_eq!(loaded.tab(work).unwrap().queue().len(), 1);
        assert_eq!(loaded.tab(first).unwrap().queue().len(), 1);
    }

    #[test]
    fn schema_one_migrates_without_prompt_loss() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        let mut queue = Queue::new();
        let mut prompt = Prompt::new("legacy").unwrap();
        prompt.pinned = true;
        let id = prompt.id;
        let created_at = prompt.created_at;
        queue.add(prompt);
        let legacy = QueueFileV1 { schema: 1, queue };
        fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let workspace = load(&path).unwrap();
        let migrated = workspace.get_prompt(id).unwrap();

        assert_eq!(workspace.tabs().len(), 1);
        assert_eq!(workspace.tabs()[0].name(), "1");
        assert_eq!(migrated.text, "legacy");
        assert!(migrated.pinned);
        assert_eq!(migrated.created_at, created_at);
        assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 1"));
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        fs::write(&path, r#"{"schema":99}"#).unwrap();
        assert!(matches!(load(&path), Err(CoreError::UnsupportedSchema(99))));
    }

    #[test]
    fn save_is_atomic_no_tmp_left_behind() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Workspace::new()).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn load_empty_file_returns_initial_workspace() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        fs::write(&path, "").unwrap();
        assert_eq!(load(&path).unwrap().tabs().len(), 1);
    }

    #[test]
    fn schema_version_is_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Workspace::new()).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 2"));
    }

    #[test]
    fn fingerprint_tracks_missing_and_saved_workspace() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        assert_eq!(fingerprint(&path).unwrap(), None);

        save(&path, &Workspace::new()).unwrap();
        assert!(fingerprint(&path).unwrap().is_some());
    }
}
