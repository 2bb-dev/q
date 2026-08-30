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
