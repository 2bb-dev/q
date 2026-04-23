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

    // Add three prompts (one pinned) — mix of arg and stdin.
    q(&dir).args(["add", "first prompt"]).assert().success();
    q(&dir)
        .args(["add"])
        .write_stdin("second prompt\n")
        .assert()
        .success();
    q(&dir)
        .args(["add", "sticky note", "--pin"])
        .assert()
        .success();

    // List shows all three and the pinned marker.
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first prompt"))
        .stdout(predicates::str::contains("second prompt"))
        .stdout(predicates::str::contains("sticky note"))
        .stdout(predicates::str::contains("[P]"));

    // copy --next returns the first unpinned (first prompt) and does NOT remove it.
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("first prompt"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("first prompt"));

    // pop --next removes the first unpinned and returns its text.
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("first prompt"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("first prompt").not())
        .stdout(predicates::str::contains("second prompt"))
        .stdout(predicates::str::contains("sticky note"));

    // Pin the remaining unpinned prompt, then unpin the original pinned.
    let second_id = short_id_of(&dir, "second prompt");
    q(&dir).args(["pin", &second_id]).assert().success();
    let sticky_id = short_id_of(&dir, "sticky note");
    q(&dir).args(["unpin", &sticky_id]).assert().success();

    // After pin/unpin, JSON list preserves two entries with toggled pinned flags.
    let out = q(&dir).args(["list", "--json"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().expect("list --json must be an array");
    assert_eq!(arr.len(), 2);
    let by_text: std::collections::HashMap<&str, bool> = arr
        .iter()
        .map(|v| (v["text"].as_str().unwrap(), v["pinned"].as_bool().unwrap()))
        .collect();
    assert!(by_text["second prompt"], "second should now be pinned");
    assert!(!by_text["sticky note"], "sticky should now be unpinned");

    // copy --next skips the now-pinned second and returns the only unpinned (sticky note).
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("sticky note"));

    // pop by full id removes even pinned entries.
    q(&dir)
        .args(["pop", &second_id, "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("second prompt"));

    // pop the remaining unpinned, then verify the queue drains to empty.
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("sticky note"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("(queue empty)"));
}
