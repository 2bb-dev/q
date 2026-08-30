use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn q(app_dir: &Path, cwd: &Path) -> Command {
    let mut command = Command::cargo_bin("q").unwrap();
    command
        .env("QCLI_APP_DIR", app_dir)
        .env("PWD", cwd)
        .current_dir(cwd);
    command
}

fn json(app_dir: &Path, cwd: &Path, args: &[&str]) -> Value {
    let output = q(app_dir, cwd).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn remove_deletes_only_the_queue_record_and_keeps_the_external_file() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("keep source.md");
    let contents = "external file remains authoritative\n";
    std::fs::write(&path, contents).unwrap();
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "keep source.md"])
        .assert()
        .success();
    let listed = json(&app_dir, root.path(), &["list", "--json"]);
    let id = listed[0]["id"].as_str().unwrap().to_owned();

    q(&app_dir, root.path())
        .args(["remove", &id])
        .assert()
        .success();

    assert_eq!(
        json(&app_dir, root.path(), &["list", "--json"])
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);

    let history = json(&app_dir, root.path(), &["history", "--json"]);
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["text"], contents);
    assert_eq!(history[0]["source"]["type"], "markdown_file");
    assert_eq!(history[0]["available"], true);
}

#[test]
fn remove_can_discard_a_broken_reference_without_reading_it() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("gone.md");
    std::fs::write(&path, "temporary").unwrap();
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "gone.md"])
        .assert()
        .success();
    let listed = json(&app_dir, root.path(), &["list", "--json"]);
    let id = listed[0]["id"].as_str().unwrap().to_owned();
    std::fs::remove_file(path).unwrap();

    q(&app_dir, root.path())
        .args(["remove", &id])
        .assert()
        .success();

    assert_eq!(
        json(&app_dir, root.path(), &["list", "--json"])
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let history = json(&app_dir, root.path(), &["history", "--json"]);
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["available"], false);
    assert!(history[0]["text"].is_null());
}

#[test]
fn remove_also_removes_inline_records_without_copying() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "inline prompt"])
        .assert()
        .success();
    let listed = json(&app_dir, root.path(), &["list", "--json"]);
    let id = listed[0]["id"].as_str().unwrap();

    q(&app_dir, root.path())
        .args(["remove", id])
        .assert()
        .success();
    assert_eq!(
        json(&app_dir, root.path(), &["list", "--json"])
            .as_array()
            .unwrap()
            .len(),
        0
    );
}
