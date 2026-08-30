use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, Result};
use crate::{Queue, Workspace};

pub const SCHEMA_VERSION: u32 = 4;

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

#[derive(Debug, Deserialize)]
struct QueueFileLegacy {
    queue: Value,
}

#[derive(Debug, Deserialize)]
struct WorkspaceFileLegacy {
    workspace: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkspaceFile {
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
            let mut parsed: QueueFileLegacy = serde_json::from_str(&data)?;
            migrate_queue_to_typed_sources(&mut parsed.queue);
            let queue: Queue = serde_json::from_value(parsed.queue)?;
            Workspace::from_legacy_queue(queue, Utc::now())
        }
        2 | 3 => {
            let mut parsed: WorkspaceFileLegacy = serde_json::from_str(&data)?;
            migrate_workspace_to_typed_sources(&mut parsed.workspace);
            let mut workspace: Workspace = serde_json::from_value(parsed.workspace)?;
            if header.schema == 2 {
                workspace.seed_history_from_prompts();
            }
            workspace
        }
        SCHEMA_VERSION => {
            let parsed: WorkspaceFile = serde_json::from_str(&data)?;
            parsed.workspace
        }
        schema => return Err(CoreError::UnsupportedSchema(schema)),
    };
    workspace.validate_and_normalize()?;
    Ok(workspace)
}

/// Replaces schema 1-3 `text` members with schema 4 tagged inline sources.
/// This is an in-memory migration only; `load` never writes the source file.
fn migrate_workspace_to_typed_sources(workspace: &mut Value) {
    if let Some(tabs) = workspace.get_mut("tabs").and_then(Value::as_array_mut) {
        for tab in tabs {
            if let Some(queue) = tab.get_mut("queue") {
                migrate_queue_to_typed_sources(queue);
            }
        }
    }

    if let Some(history) = workspace.get_mut("history").and_then(Value::as_array_mut) {
        for entry in history {
            migrate_text_member(entry);
        }
    }
}

fn migrate_queue_to_typed_sources(queue: &mut Value) {
    if let Some(prompts) = queue.get_mut("prompts").and_then(Value::as_array_mut) {
        for prompt in prompts {
            migrate_text_member(prompt);
        }
    }
}

fn migrate_text_member(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(text) = object.remove("text") else {
        return;
    };
    object.insert(
        "source".to_string(),
        serde_json::json!({ "type": "inline", "text": text }),
    );
}

/// Atomic save: write to `<path>.tmp`, then rename.
pub fn save(path: &Path, workspace: &Workspace) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let file = WorkspaceFile {
        schema: SCHEMA_VERSION,
        workspace: workspace.clone(),
    };
    let serialized = serde_json::to_string_pretty(&file)?;
    fs::write(&tmp, serialized)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/storage.rs"]
mod tests;
