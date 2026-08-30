//! Team-workspace sync through the CLI, using a local bare repo as the
//! origin (libgit2 file transport, no network or token).

use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

fn workspace_dir(dir: &TempDir) -> PathBuf {
    std::fs::read_dir(dir.path().join("workspaces"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.join("workspace.json").exists())
        .unwrap()
}

fn bare_origin() -> TempDir {
    let remote = TempDir::new().unwrap();
    let mut options = git2::RepositoryInitOptions::new();
    options.bare(true);
    options.initial_head("refs/heads/main");
    git2::Repository::init_opts(remote.path(), &options).unwrap();
    remote
}

#[test]
fn cli_commands_pull_before_and_push_after_on_team_workspaces() {
    let origin = bare_origin();
    let origin_url = origin.path().to_str().unwrap().to_string();

    // Machine A: create a workspace, make it a team workspace, push it.
    let a = TempDir::new().unwrap();
    q(&a)
        .args(["add", "from-a", "--tab", "1"])
        .assert()
        .success();
    let a_ws = workspace_dir(&a);
    q_platform::git::init_team_repo(&a_ws, "alice").unwrap();
    q_platform::git::add_origin_and_push(&a_ws, &origin_url, "").unwrap();

    // Machine B: connect by cloning the repo into the workspaces dir.
    let b = TempDir::new().unwrap();
    let b_ws = b.path().join("workspaces").join("cloned");
    std::fs::create_dir_all(b.path().join("workspaces")).unwrap();
    q_platform::git::clone_repo(&origin_url, &b_ws, "").unwrap();

    // B sees A's prompt (pull before the command).
    q(&b)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("from-a"));

    // B adds a prompt; the mutation is pushed after the command.
    q(&b)
        .args(["add", "from-b", "--tab", "1"])
        .assert()
        .success();

    // A sees both prompts after its next command pulls.
    q(&a)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("from-a"))
        .stdout(predicates::str::contains("from-b"));
}

#[test]
fn team_workspace_keeps_working_when_the_origin_is_unreachable() {
    let origin = bare_origin();
    let origin_url = origin.path().to_str().unwrap().to_string();

    let a = TempDir::new().unwrap();
    q(&a).args(["add", "seed", "--tab", "1"]).assert().success();
    let a_ws = workspace_dir(&a);
    q_platform::git::init_team_repo(&a_ws, "alice").unwrap();
    q_platform::git::add_origin_and_push(&a_ws, &origin_url, "").unwrap();

    // The origin disappears (offline).
    drop(origin);

    q(&a)
        .args(["add", "offline prompt", "--tab", "1"])
        .assert()
        .success()
        .stderr(predicates::str::contains("warning"));
    q(&a)
        .args(["list", "--tab", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains("offline prompt"));
}
