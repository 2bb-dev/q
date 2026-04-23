use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

fn short_id_of(dir: &TempDir, text_marker: &str) -> String {
    let out = q(dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    stdout
        .lines()
        .find(|line| line.contains(text_marker))
        .unwrap()
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .unwrap()
        .to_string()
}

#[test]
fn pin_moves_prompt_to_top() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "first"]).assert().success();
    q(&dir).args(["add", "second"]).assert().success();
    let id = short_id_of(&dir, "second");

    q(&dir).args(["pin", &id]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let second_idx = stdout.lines().position(|l| l.contains("second")).unwrap();
    let first_idx = stdout.lines().position(|l| l.contains("first")).unwrap();
    assert!(second_idx < first_idx, "pinned 'second' should come first");
}

#[test]
fn unpin_moves_prompt_to_unpinned_section() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "alpha", "--pin"]).assert().success();
    q(&dir).args(["add", "beta"]).assert().success();
    let id = short_id_of(&dir, "alpha");

    q(&dir).args(["unpin", &id]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let alpha_line = stdout.lines().find(|l| l.contains("alpha")).unwrap();
    assert!(
        !alpha_line.contains("[P]"),
        "alpha should no longer be pinned"
    );
}

#[test]
fn pin_unknown_id_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["pin", "deadbeef"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));
}

#[test]
fn unpin_unknown_id_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["unpin", "deadbeef"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));
}

#[test]
fn pin_short_id_rejected() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "x"]).assert().success();
    q(&dir)
        .args(["pin", "abc"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("too short"));
}

#[test]
fn pin_already_pinned_is_idempotent() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "stuck", "--pin"]).assert().success();
    let id = short_id_of(&dir, "stuck");
    q(&dir).args(["pin", &id]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let line = stdout.lines().find(|l| l.contains("stuck")).unwrap();
    assert!(line.contains("[P]"), "still pinned after second pin");
}
