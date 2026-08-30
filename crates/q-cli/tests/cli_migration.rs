use assert_cmd::Command;
use q_core::{storage, Prompt, Workspace};
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

/// Serializes the workspace the way a schema 4 binary did: a `pinned`
/// boolean instead of `pinned_at`.
fn write_legacy_queue(dir: &TempDir, workspace: &Workspace) {
    let mut workspace_value = serde_json::to_value(workspace).unwrap();
    for tab in workspace_value["tabs"].as_array_mut().unwrap() {
        for prompt in tab["queue"]["prompts"].as_array_mut().unwrap() {
            let object = prompt.as_object_mut().unwrap();
            let pinned = object.remove("pinned_at").is_some();
            object.insert("pinned".to_string(), serde_json::json!(pinned));
        }
    }
    let value = serde_json::json!({
        "schema": 4,
        "workspace": workspace_value,
    });
    std::fs::write(
        dir.path().join("queue.json"),
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn legacy_queue_json_migrates_into_a_personal_workspace_directory() {
    let dir = TempDir::new().unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let pinned = workspace
        .add_prompt(tab, Prompt::new("keep me pinned").unwrap())
        .unwrap();
    workspace.set_prompt_pinned(pinned, true).unwrap();
    workspace
        .add_prompt(tab, Prompt::new("plain prompt").unwrap())
        .unwrap();
    write_legacy_queue(&dir, &workspace);

    q(&dir)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("keep me pinned"))
        .stdout(predicates::str::contains("plain prompt"));

    assert!(!dir.path().join("queue.json").exists());
    assert!(dir.path().join("queue.json.migrated").exists());

    let root = dir.path().join("workspaces");
    let workspace_dir = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.join("workspace.json").exists())
        .expect("migrated workspace directory");
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(workspace_dir.join("workspace.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["name"], "Personal");

    let migrated = storage::load_dir(&workspace_dir).unwrap();
    assert!(migrated.get_prompt(pinned).unwrap().pinned());
}

#[test]
fn first_run_without_legacy_file_creates_a_fresh_personal_workspace() {
    let dir = TempDir::new().unwrap();

    q(&dir)
        .args(["add", "first ever", "--tab", "1"])
        .assert()
        .success();
    q(&dir)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first ever"));

    assert!(!dir.path().join("queue.json").exists());
    assert!(dir.path().join("workspaces").exists());
}

#[test]
fn migration_happens_once_and_later_runs_reuse_the_same_workspace() {
    let dir = TempDir::new().unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("survivor").unwrap())
        .unwrap();
    write_legacy_queue(&dir, &workspace);

    q(&dir)
        .args(["add", "added after migration", "--tab", "1"])
        .assert()
        .success();
    q(&dir)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("survivor"))
        .stdout(predicates::str::contains("added after migration"));

    let root = dir.path().join("workspaces");
    let count = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("workspace.json").exists())
        .count();
    assert_eq!(count, 1);
}
