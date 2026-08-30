use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use q_core::{storage, Prompt, Workspace};
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut command = Command::cargo_bin("q").unwrap();
    command.env("QCLI_APP_DIR", dir.path());
    command
}

fn workspace_with_two_tabs(dir: &TempDir) -> (q_core::TabId, q_core::TabId) {
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace.rename_tab(first, "backend").unwrap();
    let second = workspace.create_tab("website").unwrap();
    let ws_dir = storage::init_dir(&dir.path().join("workspaces"), "Personal").unwrap();
    storage::save_dir(&ws_dir, &workspace).unwrap();
    (first, second)
}

fn workspace_dir(dir: &TempDir) -> std::path::PathBuf {
    let root = dir.path().join("workspaces");
    std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.join("workspace.json").exists())
        .unwrap()
}

#[test]
fn contextual_commands_other_than_add_require_tab_when_multiple_exist() {
    let dir = TempDir::new().unwrap();
    workspace_with_two_tabs(&dir);

    for args in [
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
    let path = workspace_dir(&dir);
    let mut workspace = storage::load_dir(&path).unwrap();
    let prompt = Prompt::new("global target").unwrap();
    let id = prompt.id;
    workspace.add_prompt(website, prompt).unwrap();
    storage::save_dir(&path, &workspace).unwrap();

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
