pub mod add;
pub mod copy;
pub mod history;
pub mod list;
pub mod pin;
pub mod pop;
pub mod remove;
mod source;
pub mod tui;
pub mod workspace;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use q_core::storage::WorkspaceMeta;
use q_core::Workspace;
use q_platform::lock::FileLock;
use q_platform::paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct AppState {
    active_workspace: Option<String>,
}

pub(crate) fn read_state() -> Result<AppState> {
    let path = paths::state_path()?;
    if !path.exists() {
        return Ok(AppState::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(&path)?)?)
}

pub(crate) fn write_state(state: &AppState) -> Result<()> {
    let path = paths::state_path()?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub(crate) fn set_active_workspace(dir: &Path) -> Result<()> {
    let mut state = read_state()?;
    state.active_workspace = dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    write_state(&state)
}

/// Ensures at least one workspace exists, migrating a legacy `queue.json`
/// into a "Personal" workspace (or creating a fresh one) on first use, and
/// returns every workspace directory.
pub(crate) fn ensure_workspaces() -> Result<Vec<(PathBuf, WorkspaceMeta)>> {
    let root = paths::workspaces_dir()?;
    let mut lock = FileLock::open(&root.join(".lock"))?;
    let _guard = lock.write()?;
    let workspaces = q_core::storage::list_dirs(&root)?;
    if !workspaces.is_empty() {
        return Ok(workspaces);
    }
    let legacy_path = paths::queue_path()?;
    let workspace = q_core::storage::load_legacy_file(&legacy_path)?;
    let dir = q_core::storage::init_dir(&root, "Personal")?;
    q_core::storage::save_dir(&dir, &workspace)?;
    if legacy_path.exists() {
        fs::rename(&legacy_path, legacy_path.with_extension("json.migrated"))?;
    }
    Ok(q_core::storage::list_dirs(&root)?)
}

/// Resolves the workspace directory a command should act on: the explicit
/// `--workspace` override when given, otherwise the active workspace from
/// `state.json`, otherwise the first workspace.
pub(crate) fn resolve_workspace_dir(override_name: Option<&str>) -> Result<PathBuf> {
    let workspaces = ensure_workspaces()?;
    if let Some(name) = override_name {
        return match find_by_name(&workspaces, name) {
            Some((dir, _)) => Ok(dir.clone()),
            None => bail!(
                "workspace not found: {} (available: {})",
                name.trim(),
                available_names(&workspaces)
            ),
        };
    }
    let state = read_state()?;
    if let Some(active) = state.active_workspace {
        if let Some((dir, _)) = workspaces
            .iter()
            .find(|(dir, _)| dir.file_name().and_then(|n| n.to_str()) == Some(active.as_str()))
        {
            return Ok(dir.clone());
        }
    }
    Ok(workspaces[0].0.clone())
}

pub(crate) fn find_by_name<'a>(
    workspaces: &'a [(PathBuf, WorkspaceMeta)],
    name: &str,
) -> Option<&'a (PathBuf, WorkspaceMeta)> {
    let normalized = name.trim().to_lowercase();
    workspaces
        .iter()
        .find(|(_, meta)| meta.name.to_lowercase() == normalized)
}

pub(crate) fn available_names(workspaces: &[(PathBuf, WorkspaceMeta)]) -> String {
    workspaces
        .iter()
        .map(|(_, meta)| meta.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn with_workspace<T>(
    override_name: Option<&str>,
    action: impl FnOnce(&Workspace) -> Result<T>,
) -> Result<T> {
    let dir = resolve_workspace_dir(override_name)?;
    let mut lock = FileLock::open(&dir.join(".lock"))?;
    let _guard = lock.write()?;
    let workspace = q_core::storage::load_dir(&dir)?;
    action(&workspace)
}

pub(crate) fn with_workspace_mut<T>(
    override_name: Option<&str>,
    action: impl FnOnce(&mut Workspace) -> Result<T>,
) -> Result<T> {
    let dir = resolve_workspace_dir(override_name)?;
    let mut lock = FileLock::open(&dir.join(".lock"))?;
    let _guard = lock.write()?;
    let mut workspace = q_core::storage::load_dir(&dir)?;
    let result = action(&mut workspace)?;
    q_core::storage::save_dir(&dir, &workspace)?;
    Ok(result)
}
