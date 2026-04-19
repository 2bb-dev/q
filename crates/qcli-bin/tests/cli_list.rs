use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn list_on_empty_queue_prints_empty_notice() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn list_shows_id_preview_and_pinned_marker() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "first"]).assert().success();
    q(&dir).args(["add", "second", "--pin"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first"))
        .stdout(predicates::str::contains("second"))
        .stdout(predicates::str::contains("[P]"));
}

#[test]
fn list_json_emits_valid_json_array() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "hello"]).assert().success();
    let output = q(&dir).args(["list", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["text"], "hello");
}
