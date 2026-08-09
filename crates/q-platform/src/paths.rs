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
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serializes env-var mutation across tests in this module. Cargo runs
    // unit tests in parallel within a single binary, and QCLI_APP_DIR is
    // process-wide; without this guard concurrent tests could observe each
    // other's value and flake.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn set_app_dir(path: &std::path::Path) {
        std::env::set_var("QCLI_APP_DIR", path);
    }

    fn clear_app_dir() {
        std::env::remove_var("QCLI_APP_DIR");
    }

    #[test]
    fn override_env_is_honored() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        set_app_dir(tmp.path());
        let dir = app_dir().unwrap();
        assert_eq!(dir, tmp.path());
        clear_app_dir();
    }

    #[test]
    fn queue_path_ends_in_queue_json() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        set_app_dir(tmp.path());
        let p = queue_path().unwrap();
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("queue.json"));
        assert_eq!(p.parent(), Some(tmp.path()));
        clear_app_dir();
    }

    #[test]
    fn config_path_ends_in_config_json() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        set_app_dir(tmp.path());
        let p = config_path().unwrap();
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("config.json"));
        assert_eq!(p.parent(), Some(tmp.path()));
        clear_app_dir();
    }

    #[test]
    fn images_dir_is_created_under_app_dir() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        set_app_dir(tmp.path());
        let dir = images_dir().unwrap();
        assert_eq!(dir, tmp.path().join("images"));
        assert!(dir.is_dir(), "images_dir should create the directory");
        clear_app_dir();
    }

    #[test]
    fn app_dir_creates_missing_override_directory() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("nested").join("app");
        assert!(!nested.exists());
        set_app_dir(&nested);
        let dir = app_dir().unwrap();
        assert_eq!(dir, nested);
        assert!(nested.is_dir());
        clear_app_dir();
    }
}
