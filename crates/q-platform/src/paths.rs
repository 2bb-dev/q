//! Application directory and file-path resolution.
//!
//! `QCLI_APP_DIR` env var overrides the resolved directory (used by tests).

use std::path::PathBuf;

/// Returns the base directory for q-cli data (queue, config).
///
/// - If `QCLI_APP_DIR` is set, that path is used (created if missing).
/// - macOS: `~/Library/Application Support/q-cli`
/// - Linux: `$XDG_DATA_HOME/q-cli` or `~/.local/share/q-cli`
pub fn app_dir() -> std::io::Result<PathBuf> {
    if let Some(override_dir) = std::env::var_os("QCLI_APP_DIR") {
        let dir = PathBuf::from(override_dir);
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }
    let proj = directories::ProjectDirs::from("dev", "2bb", "q-cli").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory",
        )
    })?;
    let dir = proj.data_dir().to_path_buf();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Path to the legacy single-file queue JSON, kept only as a migration
/// source for the workspace directory format.
pub fn queue_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join("queue.json"))
}

/// Path to the persisted app state JSON (active workspace pointer).
pub fn state_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join("state.json"))
}

/// Directory containing one subdirectory per workspace.
pub fn workspaces_dir() -> std::io::Result<PathBuf> {
    let dir = app_dir()?.join("workspaces");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Path to the persisted provider config JSON.
pub fn config_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join("config.json"))
}

#[cfg(test)]
#[path = "../tests/unit/paths.rs"]
mod tests;
