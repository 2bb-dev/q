//! Workspace persistence.
//!
//! The current format is a directory with one file per prompt, designed so
//! concurrent changes touch disjoint files:
//!
//! ```text
//! workspace.json            # id, name, schema version
//! tabs/<tab-id>.json        # name, activity_at
//! prompts/<prompt-id>.json  # tab, source, created_at, pinned_at
//! history.json              # per-user local prompt history (never synced)
//! ```
//!
//! The legacy single-file `queue.json` (schemas 1-4) is still readable via
//! [`load_legacy_file`] so it can be migrated into a workspace directory.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::workspace::{HistoryEntry, Tab};
use crate::{Prompt, Queue, TabId, Workspace};

/// Schema of the directory-based workspace format.
pub const DIR_SCHEMA_VERSION: u32 = 1;

const WORKSPACE_FILE: &str = "workspace.json";
const HISTORY_FILE: &str = "history.json";
const TABS_DIR: &str = "tabs";
const PROMPTS_DIR: &str = "prompts";

#[derive(Debug, Serialize, Deserialize)]
struct WorkspaceMetaFile {
    schema: u32,
    id: Uuid,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TabFile {
    id: TabId,
    name: String,
    activity_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    created_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PromptFile {
    tab: TabId,
    #[serde(flatten)]
    prompt: Prompt,
}

#[derive(Debug, Serialize)]
struct PromptFileRef<'a> {
    tab: TabId,
    #[serde(flatten)]
    prompt: &'a Prompt,
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryFile {
    schema: u32,
    entries: Vec<HistoryEntry>,
}

/// Identity of a workspace directory, read from its `workspace.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMeta {
    pub id: Uuid,
    pub name: String,
}

/// Reads a workspace directory's metadata.
pub fn read_meta(dir: &Path) -> Result<WorkspaceMeta> {
    let meta: WorkspaceMetaFile = read_json(&dir.join(WORKSPACE_FILE))?;
    if meta.schema != DIR_SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema(meta.schema));
    }
    Ok(WorkspaceMeta {
        id: meta.id,
        name: meta.name,
    })
}

/// Renames a workspace, preserving its id.
pub fn rename_dir(dir: &Path, new_name: &str) -> Result<()> {
    let meta = read_meta(dir)?;
    write_json(
        &dir.join(WORKSPACE_FILE),
        &WorkspaceMetaFile {
            schema: DIR_SCHEMA_VERSION,
            id: meta.id,
            name: new_name.to_string(),
        },
    )
}

/// Lists the workspace directories under `root`, sorted by name then id.
pub fn list_dirs(root: &Path) -> Result<Vec<(PathBuf, WorkspaceMeta)>> {
    let mut workspaces = Vec::new();
    if !root.exists() {
        return Ok(workspaces);
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() && path.join(WORKSPACE_FILE).exists() {
            let meta = read_meta(&path)?;
            workspaces.push((path, meta));
        }
    }
    workspaces.sort_by(|left, right| {
        left.1
            .name
            .to_lowercase()
            .cmp(&right.1.name.to_lowercase())
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    Ok(workspaces)
}

/// Creates a new empty workspace directory named `name` under `parent`.
/// The directory is named after the workspace's fresh id and returned.
pub fn init_dir(parent: &Path, name: &str) -> Result<PathBuf> {
    let id = Uuid::new_v4();
    let dir = parent.join(id.to_string());
    fs::create_dir_all(&dir)?;
    let meta = WorkspaceMetaFile {
        schema: DIR_SCHEMA_VERSION,
        id,
        name: name.to_string(),
    };
    write_json(&dir.join(WORKSPACE_FILE), &meta)?;
    Ok(dir)
}

/// Loads a workspace from its directory. A workspace with no tab files yet
/// loads as a fresh workspace with the initial tab.
pub fn load_dir(dir: &Path) -> Result<Workspace> {
    let meta: WorkspaceMetaFile = read_json(&dir.join(WORKSPACE_FILE))?;
    if meta.schema != DIR_SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema(meta.schema));
    }

    let mut tab_files: Vec<TabFile> = read_dir_json(&dir.join(TABS_DIR))?;
    tab_files.sort_by_key(|tab| tab.id);
    let prompt_files: Vec<PromptFile> = read_dir_json(&dir.join(PROMPTS_DIR))?;

    let mut queues: std::collections::HashMap<TabId, Queue> = std::collections::HashMap::new();
    for file in prompt_files {
        if !tab_files.iter().any(|tab| tab.id == file.tab) {
            return Err(CoreError::TabNotFound(format!(
                "prompt {} references unknown tab {}",
                file.prompt.id, file.tab.0
            )));
        }
        queues.entry(file.tab).or_default().add(file.prompt);
    }

    let tabs: Vec<Tab> = tab_files
        .into_iter()
        .map(|tab| {
            let queue = queues.remove(&tab.id).unwrap_or_default();
            Tab::from_parts(tab.id, tab.name, tab.activity_at, tab.created_by, queue)
        })
        .collect();

    let history_path = dir.join(HISTORY_FILE);
    let history = if history_path.exists() {
        let file: HistoryFile = read_json(&history_path)?;
        if file.schema != DIR_SCHEMA_VERSION {
            return Err(CoreError::UnsupportedSchema(file.schema));
        }
        file.entries
    } else {
        Vec::new()
    };

    let mut workspace = Workspace::from_parts(tabs, history);
    workspace.validate_and_normalize()?;
    Ok(workspace)
}

/// Saves a workspace into its directory: one file per tab and prompt, files
/// for removed tabs and prompts deleted, unchanged files left untouched.
/// Every write is atomic (tmp + rename).
pub fn save_dir(dir: &Path, workspace: &Workspace) -> Result<()> {
    if !dir.join(WORKSPACE_FILE).exists() {
        return Err(CoreError::Invalid(format!(
            "not a workspace directory (missing {WORKSPACE_FILE}): {}",
            dir.display()
        )));
    }
    let tabs_dir = dir.join(TABS_DIR);
    let prompts_dir = dir.join(PROMPTS_DIR);
    fs::create_dir_all(&tabs_dir)?;
    fs::create_dir_all(&prompts_dir)?;

    let mut keep_tabs = HashSet::new();
    let mut keep_prompts = HashSet::new();
    for tab in workspace.tabs() {
        let file_name = format!("{}.json", tab.id().0);
        write_json_if_changed(
            &tabs_dir.join(&file_name),
            &TabFile {
                id: tab.id(),
                name: tab.name().to_string(),
                activity_at: tab.activity_at(),
                created_by: tab.created_by().map(str::to_string),
            },
        )?;
        keep_tabs.insert(file_name);
        for prompt in tab.queue().iter() {
            let file_name = format!("{}.json", prompt.id.0);
            write_json_if_changed(
                &prompts_dir.join(&file_name),
                &PromptFileRef {
                    tab: tab.id(),
                    prompt,
                },
            )?;
            keep_prompts.insert(file_name);
        }
    }
    remove_stale_json(&tabs_dir, &keep_tabs)?;
    remove_stale_json(&prompts_dir, &keep_prompts)?;

    write_json_if_changed(
        &dir.join(HISTORY_FILE),
        &HistoryFile {
            schema: DIR_SCHEMA_VERSION,
            entries: workspace.history().to_vec(),
        },
    )?;
    Ok(())
}

/// Snapshot of every persisted file's identity in a workspace directory,
/// used to detect changes made by other processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirFingerprint(Vec<(PathBuf, SystemTime, u64)>);

pub fn fingerprint_dir(dir: &Path) -> Result<Option<DirFingerprint>> {
    if !dir.exists() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    for name in [WORKSPACE_FILE, HISTORY_FILE] {
        stat_into(&dir.join(name), &mut entries)?;
    }
    for sub in [TABS_DIR, PROMPTS_DIR] {
        let sub_dir = dir.join(sub);
        if !sub_dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&sub_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                stat_into(&path, &mut entries)?;
            }
        }
    }
    entries.sort();
    Ok(Some(DirFingerprint(entries)))
}

fn stat_into(path: &Path, entries: &mut Vec<(PathBuf, SystemTime, u64)>) -> Result<()> {
    match fs::metadata(path) {
        Ok(metadata) => {
            entries.push((path.to_path_buf(), metadata.modified()?, metadata.len()));
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn read_dir_json<T: serde::de::DeserializeOwned>(dir: &Path) -> Result<Vec<T>> {
    let mut values = Vec::new();
    if !dir.exists() {
        return Ok(values);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            values.push(read_json(&path)?);
        }
    }
    Ok(values)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut serialized = serde_json::to_string_pretty(value)?;
    serialized.push('\n');
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &serialized)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn write_json_if_changed<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut serialized = serde_json::to_string_pretty(value)?;
    serialized.push('\n');
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == serialized {
            return Ok(());
        }
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &serialized)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn remove_stale_json(dir: &Path, keep: &HashSet<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !keep.contains(file_name) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

// --- Legacy single-file format (schemas 1-4), read-only for migration ---

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

/// Loads a legacy single-file `queue.json` (schemas 1-4). Missing or empty
/// files load as a fresh workspace. Never writes the source file.
pub fn load_legacy_file(path: &Path) -> Result<Workspace> {
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
            migrate_queue_pinned(&mut parsed.queue);
            let queue: Queue = serde_json::from_value(parsed.queue)?;
            Workspace::from_legacy_queue(queue, Utc::now())
        }
        2..=4 => {
            let mut parsed: WorkspaceFileLegacy = serde_json::from_str(&data)?;
            if header.schema < 4 {
                migrate_workspace_to_typed_sources(&mut parsed.workspace);
            }
            migrate_workspace_pinned(&mut parsed.workspace);
            let mut workspace: Workspace = serde_json::from_value(parsed.workspace)?;
            if header.schema == 2 {
                workspace.seed_history_from_prompts();
            }
            workspace
        }
        schema => return Err(CoreError::UnsupportedSchema(schema)),
    };
    workspace.validate_and_normalize()?;
    Ok(workspace)
}

/// Replaces schema 1-3 `text` members with schema 4 tagged inline sources.
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

/// Replaces the legacy `pinned` boolean with a `pinned_at` timestamp; a
/// pinned prompt reuses its `created_at` as the pin time.
fn migrate_workspace_pinned(workspace: &mut Value) {
    if let Some(tabs) = workspace.get_mut("tabs").and_then(Value::as_array_mut) {
        for tab in tabs {
            if let Some(queue) = tab.get_mut("queue") {
                migrate_queue_pinned(queue);
            }
        }
    }
}

fn migrate_queue_pinned(queue: &mut Value) {
    if let Some(prompts) = queue.get_mut("prompts").and_then(Value::as_array_mut) {
        for prompt in prompts {
            let Some(object) = prompt.as_object_mut() else {
                continue;
            };
            let pinned = object.remove("pinned");
            if pinned.and_then(|v| v.as_bool()) == Some(true) {
                let pinned_at = object
                    .get("created_at")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!(Utc::now()));
                object.insert("pinned_at".to_string(), pinned_at);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/storage.rs"]
mod tests;
