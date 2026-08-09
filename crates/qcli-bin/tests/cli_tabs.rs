use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use qcli_core::{storage, Prompt, Workspace};
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut command = Command::cargo_bin("q").unwrap();
    command.env("QCLI_APP_DIR", dir.path());
    command
}

fn workspace_with_two_tabs(dir: &TempDir) -> (qcli_core::TabId, qcli_core::TabId) {
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace.rename_tab(first, "backend").unwrap();
    let second = workspace.create_tab("website").unwrap();
    storage::save(&dir.path().join("queue.json"), &workspace).unwrap();
    (first, second)
}

#[test]
fn contextual_commands_require_tab_when_multiple_exist() {
    let dir = TempDir::new().unwrap();
    workspace_with_two_tabs(&dir);

    for args in [
        vec!["add", "prompt"],
        vec!["list"],
        vec!["copy", "--next", "--stdout"],
        vec!["pop", "--next", "--stdout"],
    ] {
        q(&dir)
            .args(args)
            .assert()
            .failure()
            .stderr(predicates::str::contains("specify --tab"))
            .stderr(predicates::str::contains("backend"))
            .stderr(predicates::str::contains("website"));
    }
}

#[test]
fn add_list_copy_and_pop_target_named_tab_case_insensitively() {
    let dir = TempDir::new().unwrap();
    workspace_with_two_tabs(&dir);

    q(&dir)
        .args(["add", "api task", "--tab", "BACKEND"])
        .assert()
        .success();
    q(&dir)
        .args(["add", "landing task", "--tab", "website"])
        .assert()
        .success();

    q(&dir)
        .args(["list", "--tab", "backend"])
        .assert()
        .success()
        .stdout(predicates::str::contains("api task"))
        .stdout(predicates::str::contains("landing task").not());
    q(&dir)
        .args(["copy", "--next", "--stdout", "--tab", "website"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("landing task"));
    q(&dir)
        .args(["pop", "--next", "--stdout", "--tab", "website"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("landing task"));
    q(&dir)
        .args(["list", "--tab", "website"])
        .assert()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn unknown_tab_error_lists_available_names() {
    let dir = TempDir::new().unwrap();
    workspace_with_two_tabs(&dir);

    q(&dir)
        .args(["list", "--tab", "missing"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("tab not found: missing"))
        .stderr(predicates::str::contains("backend"))
        .stderr(predicates::str::contains("website"));
}

#[test]
fn id_based_commands_find_prompts_globally_without_tab() {
    let dir = TempDir::new().unwrap();
    let (_, website) = workspace_with_two_tabs(&dir);
    let path = dir.path().join("queue.json");
    let mut workspace = storage::load(&path).unwrap();
    let prompt = Prompt::new("global target").unwrap();
    let id = prompt.id;
    workspace.add_prompt(website, prompt).unwrap();
    storage::save(&path, &workspace).unwrap();

    q(&dir).args(["pin", &id.to_string()]).assert().success();
    q(&dir)
        .args(["copy", &id.to_string(), "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("global target"));
    q(&dir)
        .args(["pop", &id.to_string(), "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("global target"));
}
