pub mod add;
pub mod copy;
pub mod history;
pub mod list;
pub mod pin;
pub mod pop;
pub mod remove;
mod source;
pub mod tui;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use q_core::Workspace;
use q_platform::lock::FileLock;
use q_platform::paths;

/// Resolves the active workspace directory, migrating a legacy `queue.json`
/// into a "Personal" workspace (or creating a fresh one) on first use.
pub(crate) fn active_workspace_dir() -> Result<PathBuf> {
    let root = paths::workspaces_dir()?;
    let mut lock = FileLock::open(&root.join(".lock"))?;
    let _guard = lock.write()?;
    if let Some(dir) = first_workspace_dir(&root)? {
        return Ok(dir);
    }
    let legacy_path = paths::queue_path()?;
    let workspace = q_core::storage::load_legacy_file(&legacy_path)?;
    let dir = q_core::storage::init_dir(&root, "Personal")?;
    q_core::storage::save_dir(&dir, &workspace)?;
    if legacy_path.exists() {
        fs::rename(&legacy_path, legacy_path.with_extension("json.migrated"))?;
    }
    Ok(dir)
}

fn first_workspace_dir(root: &Path) -> Result<Option<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() && path.join("workspace.json").exists() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs.into_iter().next())
}

pub(crate) fn with_workspace<T>(action: impl FnOnce(&Workspace) -> Result<T>) -> Result<T> {
    let dir = active_workspace_dir()?;
    let mut lock = FileLock::open(&dir.join(".lock"))?;
    let _guard = lock.write()?;
    let workspace = q_core::storage::load_dir(&dir)?;
    action(&workspace)
}

pub(crate) fn with_workspace_mut<T>(action: impl FnOnce(&mut Workspace) -> Result<T>) -> Result<T> {
    let dir = active_workspace_dir()?;
    let mut lock = FileLock::open(&dir.join(".lock"))?;
    let _guard = lock.write()?;
    let mut workspace = q_core::storage::load_dir(&dir)?;
    let result = action(&mut workspace)?;
    q_core::storage::save_dir(&dir, &workspace)?;
    Ok(result)
}
