use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn copy_next_stdout_prints_newest_prompt_text() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "alpha"]).assert().success();
    q(&dir).args(["add", "beta"]).assert().success();
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("beta"));
}

#[test]
fn copy_by_id_prefix_stdout_prints_that_prompt() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "target"]).assert().success();
    let output = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let id = stdout
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .unwrap()
        .to_string();

    q(&dir)
        .args(["copy", &id, "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("target"));
}

#[test]
fn copy_does_not_remove_the_prompt() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "keep me"]).assert().success();
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("keep me"));
}

#[test]
fn copy_without_id_or_next_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "x"]).assert().success();
    q(&dir)
        .args(["copy"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--next"));
}

#[test]
fn copy_empty_queue_with_next_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("empty"));
}
