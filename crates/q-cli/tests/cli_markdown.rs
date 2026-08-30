use std::path::{Component, Path};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use serde_json::Value;
use tempfile::TempDir;

fn q(app_dir: &Path, cwd: &Path) -> Command {
    let mut command = Command::cargo_bin("q").unwrap();
    command
        .env("QCLI_APP_DIR", app_dir)
        .env("PWD", cwd)
        .current_dir(cwd);
    command
}

fn list_json(app_dir: &Path, cwd: &Path) -> Value {
    let output = q(app_dir, cwd).args(["list", "--json"]).output().unwrap();
    assert!(
        output.status.success(),
        "list --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn history_json(app_dir: &Path, cwd: &Path, search: Option<&str>) -> Value {
    let mut command = q(app_dir, cwd);
    command.arg("history");
    if let Some(search) = search {
        command.arg(search);
    }
    let output = command.arg("--json").output().unwrap();
    assert!(
        output.status.success(),
        "history --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn first_id(items: &Value) -> &str {
    items[0]["id"].as_str().expect("list item must have an id")
}

#[test]
fn markdown_extensions_are_case_insensitive_and_paths_are_absolute_but_noncanonical() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let cwd = root.path().join("project");
    std::fs::create_dir_all(cwd.join("notes")).unwrap();
    std::fs::write(cwd.join("prompt.MD"), "first live prompt").unwrap();
    std::fs::write(cwd.join("second.mArKdOwN"), "second live prompt").unwrap();

    let noncanonical = Path::new("notes/../prompt.MD");
    q(&app_dir, &cwd)
        .args(["add", "--tab", "1", noncanonical.to_str().unwrap()])
        .assert()
        .success();
    q(&app_dir, &cwd)
        .args(["add", "--tab", "1", "second.mArKdOwN"])
        .assert()
        .success();

    let listed = list_json(&app_dir, &cwd);
    let items = listed.as_array().unwrap();
    assert_eq!(items.len(), 2);

    let expected_noncanonical = cwd.join(noncanonical);
    assert!(expected_noncanonical.is_absolute());
    assert!(expected_noncanonical
        .components()
        .any(|component| component == Component::ParentDir));
    let first = items
        .iter()
        .find(|item| item["text"] == "first live prompt")
        .unwrap();
    assert_eq!(first["source"]["type"], "markdown_file");
    assert_eq!(
        first["source"]["path"],
        expected_noncanonical.to_str().unwrap()
    );
    assert_eq!(first["available"], true);

    let second = items
        .iter()
        .find(|item| item["text"] == "second live prompt")
        .unwrap();
    assert_eq!(second["source"]["type"], "markdown_file");
    assert_eq!(
        second["source"]["path"],
        cwd.join("second.mArKdOwN").to_str().unwrap()
    );
}

#[test]
fn text_flag_forces_a_markdown_looking_argument_to_remain_inline() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");

    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "--text", "missing file.MD"])
        .assert()
        .success();

    let listed = list_json(&app_dir, root.path());
    assert_eq!(listed[0]["text"], "missing file.MD");
    assert_eq!(listed[0]["source"]["type"], "inline");
    assert_eq!(listed[0]["available"], true);
}

#[test]
fn stdin_is_always_inline_even_when_it_looks_like_a_markdown_path() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    std::fs::write(root.path().join("existing.md"), "file contents").unwrap();

    q(&app_dir, root.path())
        .args(["add", "--tab", "1"])
        .write_stdin("existing.md\n")
        .assert()
        .success();

    let listed = list_json(&app_dir, root.path());
    assert_eq!(listed[0]["text"], "existing.md\n");
    assert_eq!(listed[0]["source"]["type"], "inline");
}

#[test]
fn markdown_add_rejects_missing_paths_directories_and_invalid_utf8() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    std::fs::create_dir(root.path().join("directory.md")).unwrap();
    std::fs::write(root.path().join("invalid.md"), [0xff, 0xfe, 0xfd]).unwrap();

    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "missing.md"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "directory.md"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("regular file"));
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "invalid.md"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("UTF-8").or(predicates::str::contains("utf-8")));

    assert_eq!(
        list_json(&app_dir, root.path()).as_array().unwrap().len(),
        0
    );
}

#[test]
fn empty_markdown_is_valid_and_filenames_with_spaces_support_pin_and_tab() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("empty prompt.markdown");
    std::fs::write(&path, "").unwrap();

    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "--pin", "empty prompt.markdown"])
        .assert()
        .success();

    let listed = list_json(&app_dir, root.path());
    assert_eq!(listed[0]["text"], "");
    assert_eq!(listed[0]["available"], true);
    assert_eq!(listed[0]["pinned"], true);
    assert_eq!(listed[0]["source"]["type"], "markdown_file");
    assert_eq!(listed[0]["source"]["path"], path.to_str().unwrap());
}

#[test]
fn copy_reads_the_files_current_contents_exactly() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("live.md");
    std::fs::write(&path, "old contents").unwrap();
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "live.md"])
        .assert()
        .success();
    let listed = list_json(&app_dir, root.path());
    let id = first_id(&listed).to_owned();

    let current = "\u{feff}  current\r\ncontents\r\n";
    std::fs::write(&path, current.as_bytes()).unwrap();
    let output = q(&app_dir, root.path())
        .args(["copy", &id, "--stdout"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "copy failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, current.as_bytes());
    assert_eq!(
        list_json(&app_dir, root.path()).as_array().unwrap().len(),
        1
    );
}

#[test]
fn pop_reads_current_contents_then_removes_only_the_reference() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("pop.md");
    std::fs::write(&path, "before").unwrap();
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "pop.md"])
        .assert()
        .success();
    let listed = list_json(&app_dir, root.path());
    let id = first_id(&listed).to_owned();

    let current = "after\nwith trailing newline\n";
    std::fs::write(&path, current).unwrap();
    let output = q(&app_dir, root.path())
        .args(["pop", &id, "--stdout"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, current.as_bytes());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), current);
    assert_eq!(
        list_json(&app_dir, root.path()).as_array().unwrap().len(),
        0
    );
}

#[test]
fn broken_copy_and_pop_fail_and_preserve_the_reference() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("broken.md");
    std::fs::write(&path, "queued contents").unwrap();
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "broken.md"])
        .assert()
        .success();
    let listed = list_json(&app_dir, root.path());
    let id = first_id(&listed).to_owned();
    std::fs::remove_file(&path).unwrap();

    q(&app_dir, root.path())
        .args(["copy", &id, "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("moved or deleted"));
    q(&app_dir, root.path())
        .args(["pop", &id, "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("moved or deleted"));

    let broken = list_json(&app_dir, root.path());
    assert_eq!(broken.as_array().unwrap().len(), 1);
    assert_eq!(broken[0]["id"], id);
    assert_eq!(broken[0]["source"]["type"], "markdown_file");
    assert_eq!(broken[0]["source"]["path"], path.to_str().unwrap());
    assert_eq!(broken[0]["available"], false);
    assert!(broken[0]["text"].is_null());
}

#[test]
fn history_searches_the_live_contents_and_path_after_pop() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    let path = root.path().join("roadmap-file-name.md");
    std::fs::write(&path, "original contents").unwrap();
    q(&app_dir, root.path())
        .args(["add", "--tab", "1", "roadmap-file-name.md"])
        .assert()
        .success();
    let listed = list_json(&app_dir, root.path());
    let id = first_id(&listed).to_owned();
    q(&app_dir, root.path())
        .args(["pop", &id, "--stdout"])
        .assert()
        .success();

    std::fs::write(&path, "newly edited searchable phrase").unwrap();
    let by_contents = history_json(&app_dir, root.path(), Some("edited searchable"));
    assert_eq!(by_contents.as_array().unwrap().len(), 1);
    assert_eq!(by_contents[0]["text"], "newly edited searchable phrase");
    assert_eq!(by_contents[0]["source"]["type"], "markdown_file");
    assert_eq!(by_contents[0]["available"], true);

    let by_path = history_json(&app_dir, root.path(), Some("roadmap-file-name"));
    assert_eq!(by_path.as_array().unwrap().len(), 1);
    assert_eq!(by_path[0]["source"]["path"], path.to_str().unwrap());

    std::fs::remove_file(path).unwrap();
    let broken = history_json(&app_dir, root.path(), Some("roadmap-file-name"));
    assert_eq!(broken.as_array().unwrap().len(), 1);
    assert_eq!(broken[0]["available"], false);
    assert!(broken[0]["text"].is_null());
}

#[test]
fn duplicate_markdown_references_are_allowed() {
    let root = TempDir::new().unwrap();
    let app_dir = root.path().join("app");
    std::fs::write(root.path().join("duplicate.md"), "same live source").unwrap();

    for _ in 0..2 {
        q(&app_dir, root.path())
            .args(["add", "--tab", "1", "duplicate.md"])
            .assert()
            .success();
    }

    let listed = list_json(&app_dir, root.path());
    let items = listed.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_ne!(items[0]["id"], items[1]["id"]);
    assert_eq!(items[0]["source"], items[1]["source"]);
}
