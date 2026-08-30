use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn add_requires_tab_even_when_workspace_has_only_one_tab() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "hello world"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--tab"));
}

#[test]
fn add_with_arg_creates_prompt_and_list_shows_it() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "hello world"])
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("hello world"));
}

#[test]
fn add_from_stdin_when_no_arg() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1"])
        .write_stdin("from stdin\n")
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("from stdin"));
}

#[test]
fn add_pin_flag_marks_prompt_pinned() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "pinned one", "--pin"])
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("[P]").and(predicates::str::contains("pinned one")));
}

#[test]
fn add_empty_text_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("empty"));
}
