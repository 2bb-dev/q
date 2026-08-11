use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::{Queue, Workspace};

pub const SCHEMA_VERSION: u32 = 3;

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
            let parsed: QueueFileV1 = serde_json::from_str(&data)?;
            Workspace::from_legacy_queue(parsed.queue, Utc::now())
        }
        2 => {
            let parsed: WorkspaceFile = serde_json::from_str(&data)?;
            let mut workspace = parsed.workspace;
            workspace.seed_history_from_prompts();
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
