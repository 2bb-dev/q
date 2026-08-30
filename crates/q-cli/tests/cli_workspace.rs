use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn list_shows_the_default_personal_workspace_as_active() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("* Personal"));
}

#[test]
fn create_switches_to_the_new_workspace() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("* team"))
        .stdout(predicates::str::contains("  Personal"));
}

#[test]
fn create_rejects_duplicate_names_case_insensitively() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "create", "TEAM"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn switch_changes_which_workspace_commands_act_on() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "personal prompt", "--tab", "1"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    q(&dir)
        .args(["add", "team prompt", "--tab", "1"])
        .assert()
        .success();

    q(&dir)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("team prompt"))
        .stdout(predicates::str::contains("personal prompt").not());

    q(&dir)
        .args(["workspace", "switch", "personal"])
        .assert()
        .success();
    q(&dir)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("personal prompt"))
        .stdout(predicates::str::contains("team prompt").not());
}

#[test]
fn workspace_flag_overrides_the_active_workspace_for_one_command() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    q(&dir)
        .args(["add", "targeted", "--tab", "1", "--workspace", "Personal"])
        .assert()
        .success();

    q(&dir)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(queue empty)"));
    q(&dir)
        .args(["list", "--tab", "1", "--workspace", "personal"])
        .assert()
        .success()
        .stdout(predicates::str::contains("targeted"));
}

#[test]
fn workspace_flag_rejects_unknown_names_listing_available() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["list", "--tab", "1", "--workspace", "nope"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("workspace not found: nope"))
        .stderr(predicates::str::contains("Personal"));
}

#[test]
fn rename_keeps_prompts_and_updates_the_list() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "kept", "--tab", "1"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "rename", "personal", "mine"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("* mine"));
    q(&dir)
        .args(["list", "--tab", "1", "--workspace", "mine"])
        .assert()
        .success()
        .stdout(predicates::str::contains("kept"));
}

#[test]
fn delete_refuses_the_last_workspace() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "delete", "Personal", "--yes"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "cannot delete the last workspace",
        ));
}

#[test]
fn deleting_the_active_workspace_repoints_to_a_remaining_one() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "delete", "team", "--yes"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("* Personal"));
}

#[test]
fn delete_without_yes_asks_for_confirmation_and_aborts_on_no() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    q(&dir)
        .args(["workspace", "delete", "team"])
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("aborted"));
    q(&dir)
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("team"));
}

#[test]
fn list_json_includes_ids_names_and_active_flag() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["workspace", "create", "team"])
        .assert()
        .success();
    let output = q(&dir)
        .args(["workspace", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let entries = parsed.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|entry| entry["name"] == "team" && entry["active"] == true));
    assert!(entries
        .iter()
        .any(|entry| entry["name"] == "Personal" && entry["active"] == false));
    assert!(entries.iter().all(|entry| entry["id"].is_string()));
}

#[test]
fn add_records_the_cached_github_login_and_lists_it_in_json() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    std::fs::write(dir.path().join("github_user"), "octocat\n").unwrap();

    q(&dir)
        .args(["add", "attributed", "--tab", "1"])
        .assert()
        .success();
    let output = q(&dir)
        .args(["list", "--tab", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(parsed[0]["created_by"], "octocat");
}

#[test]
fn add_without_identity_emits_no_author_fields() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "anonymous", "--tab", "1"])
        .assert()
        .success();
    let output = q(&dir)
        .args(["list", "--tab", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(parsed[0].get("created_by").is_none());
    assert!(parsed[0].get("updated_by").is_none());
}

#[test]
fn team_workspaces_reject_external_markdown_references() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "seed", "--tab", "1"])
        .assert()
        .success();
    // Mark the workspace as a team workspace (a git repository).
    let root = dir.path().join("workspaces");
    let workspace_dir = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.join("workspace.json").exists())
        .unwrap();
    std::fs::create_dir_all(workspace_dir.join(".git")).unwrap();

    let markdown = dir.path().join("note.md");
    std::fs::write(&markdown, "# note").unwrap();
    q(&dir)
        .args(["add", markdown.to_str().unwrap(), "--tab", "1"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("inline prompts only"));

    // The same path is accepted as literal text.
    q(&dir)
        .args(["add", markdown.to_str().unwrap(), "--tab", "1", "--text"])
        .assert()
        .success();
}
