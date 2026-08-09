//! End-to-end CLI workflow: exercises the full user journey across subcommands
//! (add, list, pin, unpin, copy, pop) sharing a single persisted queue file.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

fn short_id_of(dir: &TempDir, marker: &str) -> String {
    let out = q(dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    stdout
        .lines()
        .find(|line| line.contains(marker))
        .unwrap_or_else(|| panic!("marker `{marker}` not found in list output:\n{stdout}"))
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .expect("no 8-char id column")
        .to_string()
}

#[test]
fn full_user_journey_persists_across_commands() {
    let dir = TempDir::new().unwrap();

    // Empty queue: list reports it cleanly, copy/pop refuse.
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(queue empty)"));
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .failure();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .failure();

    // Add two unpinned prompts (arg + stdin), newline-trimmed.
    q(&dir).args(["add", "first prompt"]).assert().success();
    q(&dir)
        .args(["add"])
        .write_stdin("second prompt\n")
        .assert()
        .success();

    // List shows both, no pinned marker yet.
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first prompt"))
        .stdout(predicates::str::contains("second prompt"))
        .stdout(predicates::str::contains("[P]").not());

    // copy --next returns the newest prompt without removing it.
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("second prompt"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("second prompt"));

    // pop --next removes the newest unpinned prompt and returns its text.
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("second prompt"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("second prompt").not())
        .stdout(predicates::str::contains("first prompt"));

    // Add a pinned prompt; list now shows both and the pinned marker.
    q(&dir)
        .args(["add", "sticky note", "--pin"])
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("sticky note"))
        .stdout(predicates::str::contains("first prompt"))
        .stdout(predicates::str::contains("[P]"));

    // pop --next skips the pinned sticky note and returns the remaining unpinned.
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("first prompt"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("sticky note"))
        .stdout(predicates::str::contains("first prompt").not());

    // With only pinned remaining, pop --next reports no unpinned available.
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no unpinned"));

    // Resolve the pinned prompt's id and flip it unpinned.
    let sticky_id = short_id_of(&dir, "sticky note");
    q(&dir).args(["unpin", &sticky_id]).assert().success();

    // JSON list confirms single entry with pinned=false.
    let out = q(&dir).args(["list", "--json"]).output().unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    let arr = parsed.as_array().expect("list --json must be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["text"], "sticky note");
    assert!(
        !arr[0]["pinned"].as_bool().unwrap(),
        "sticky should now be unpinned"
    );

    // Pin it again via the subcommand, then pop it by id (pop by id removes
    // even pinned prompts).
    q(&dir).args(["pin", &sticky_id]).assert().success();
    q(&dir)
        .args(["pop", &sticky_id, "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("sticky note"));

    // Queue drains to empty.
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("(queue empty)"));
}
