use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn pop_next_stdout_prints_and_removes_newest_unpinned() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "first"]).assert().success();
    q(&dir).args(["add", "second"]).assert().success();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("second"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("first"))
        .stdout(predicates::str::contains("second").not());
}

#[test]
fn pop_skips_pinned_prompts() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "pinned", "--pin"]).assert().success();
    q(&dir).args(["add", "floating"]).assert().success();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .stdout(predicates::str::starts_with("floating"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("pinned"))
        .stdout(predicates::str::contains("floating").not());
}

#[test]
fn pop_by_id_removes_that_prompt_even_if_pinned() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "target", "--pin"]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let id = stdout
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .unwrap()
        .to_string();

    q(&dir).args(["pop", &id, "--stdout"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn pop_next_on_empty_queue_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no unpinned"));
}
