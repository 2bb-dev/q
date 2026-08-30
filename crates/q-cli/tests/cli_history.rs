use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn history_on_empty_workspace_prints_empty_notice() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["history"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(no matching prompts)"));
}

#[test]
fn history_keeps_prompts_that_were_popped() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "deploy the api"])
        .assert()
        .success();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(queue empty)"));

    q(&dir)
        .args(["history"])
        .assert()
        .success()
        .stdout(predicates::str::contains("deploy the api"));
}

#[test]
fn history_filters_case_insensitively_by_search_term() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "deploy the api"])
        .assert()
        .success();
    q(&dir)
        .args(["add", "--tab", "1", "write the docs"])
        .assert()
        .success();

    q(&dir)
        .args(["history", "API"])
        .assert()
        .success()
        .stdout(predicates::str::contains("deploy the api"))
        .stdout(predicates::str::contains("write the docs").not());
}

#[test]
fn history_search_is_script_and_accent_insensitive() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "улучшить конфиги"])
        .assert()
        .success();
    q(&dir)
        .args(["add", "--tab", "1", "café Müller"])
        .assert()
        .success();

    // Latin query finds Cyrillic text.
    q(&dir)
        .args(["history", "uluchshit"])
        .assert()
        .success()
        .stdout(predicates::str::contains("улучшить конфиги"));

    // Cyrillic query still works.
    q(&dir)
        .args(["history", "КОНФИГИ"])
        .assert()
        .success()
        .stdout(predicates::str::contains("улучшить конфиги"));

    // Unaccented query finds accented text.
    q(&dir)
        .args(["history", "cafe muller"])
        .assert()
        .success()
        .stdout(predicates::str::contains("café Müller"));
}

#[test]
fn history_clear_forgets_everything_but_keeps_the_queue() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "keep me"])
        .assert()
        .success();

    q(&dir)
        .args(["history", "--clear"])
        .assert()
        .success()
        .stdout(predicates::str::contains("forgot 1 prompt"));

    q(&dir)
        .args(["history"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(no matching prompts)"));
    // The queued prompt itself is untouched.
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("keep me"));
}

#[test]
fn history_forget_removes_only_matching_entries() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "secret token abc"])
        .assert()
        .success();
    q(&dir)
        .args(["add", "--tab", "1", "harmless prompt"])
        .assert()
        .success();

    q(&dir)
        .args(["history", "--forget", "secret"])
        .assert()
        .success()
        .stdout(predicates::str::contains("forgot 1 prompt"));

    q(&dir)
        .args(["history"])
        .assert()
        .success()
        .stdout(predicates::str::contains("harmless prompt"))
        .stdout(predicates::str::contains("secret token").not());
}

#[test]
fn history_forget_matches_across_scripts() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "секретный токен"])
        .assert()
        .success();

    q(&dir)
        .args(["history", "--forget", "sekretnyy"])
        .assert()
        .success()
        .stdout(predicates::str::contains("forgot 1 prompt"));

    q(&dir)
        .args(["history"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(no matching prompts)"));
}

#[test]
fn history_clear_conflicts_with_a_search_term() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["history", "term", "--clear"])
        .assert()
        .failure();
}

#[test]
fn history_json_emits_valid_json_array() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "--tab", "1", "hello"])
        .assert()
        .success();

    let output = q(&dir).args(["history", "--json"]).output().unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["text"], "hello");
    assert_eq!(parsed[0]["source"]["type"], "inline");
    assert_eq!(parsed[0]["available"], true);
}
