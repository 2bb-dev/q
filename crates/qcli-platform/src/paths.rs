//! Application directory and file-path resolution.
//!
//! `QCLI_APP_DIR` env var overrides the resolved directory (used by tests).

use std::path::PathBuf;

/// Returns the base directory for q-cli data (queue, config, images).
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

/// Path to the persisted queue JSON.
pub fn queue_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join("queue.json"))
}

/// Path to the persisted provider config JSON.
pub fn config_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join("config.json"))
}

/// Directory for stored image attachments.
pub fn images_dir() -> std::io::Result<PathBuf> {
    let dir = app_dir()?.join("images");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn override_env_is_honored() {
        let tmp = TempDir::new().unwrap();
        std::env::set_var("QCLI_APP_DIR", tmp.path());
        let dir = app_dir().unwrap();
        assert_eq!(dir, tmp.path());
        std::env::remove_var("QCLI_APP_DIR");
    }

    #[test]
    fn queue_path_ends_in_queue_json() {
        let tmp = TempDir::new().unwrap();
        std::env::set_var("QCLI_APP_DIR", tmp.path());
        let p = queue_path().unwrap();
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("queue.json"));
        std::env::remove_var("QCLI_APP_DIR");
    }
}
