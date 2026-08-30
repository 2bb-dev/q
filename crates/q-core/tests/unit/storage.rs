use super::*;
use crate::{Prompt, PromptSource};
use tempfile::TempDir;
use uuid::Uuid;

// --- Directory format ---

#[test]
fn init_dir_creates_workspace_named_after_its_id() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let meta: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("workspace.json")).unwrap()).unwrap();
    assert_eq!(meta["schema"], DIR_SCHEMA_VERSION);
    assert_eq!(meta["name"], "Personal");
    assert_eq!(
        dir.file_name().and_then(|n| n.to_str()).unwrap(),
        meta["id"].as_str().unwrap()
    );
}

#[test]
fn load_fresh_dir_returns_initial_workspace() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let workspace = load_dir(&dir).unwrap();
    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
}

#[test]
fn save_then_load_roundtrips_tabs_prompts_pins_and_history() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let markdown = root.path().join("not-created.md");
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    let inline = workspace
        .add_prompt(first, Prompt::new("hello").unwrap())
        .unwrap();
    workspace.set_prompt_pinned(inline, true).unwrap();
    let work = workspace.create_tab("work").unwrap();
    workspace
        .add_prompt(work, Prompt::from_external_markdown(&markdown).unwrap())
        .unwrap();
    let popped = workspace
        .add_prompt(first, Prompt::new("popped later").unwrap())
        .unwrap();
    workspace.remove_prompt(popped).unwrap();

    save_dir(&dir, &workspace).unwrap();
    let loaded = load_dir(&dir).unwrap();

    assert_eq!(loaded.tabs().len(), 2);
    assert_eq!(loaded.tab(work).unwrap().queue().len(), 1);
    assert_eq!(loaded.tab(first).unwrap().queue().len(), 1);
    assert!(loaded.get_prompt(inline).unwrap().pinned());
    assert!(loaded.get_prompt(popped).is_none());
    assert!(loaded
        .history()
        .iter()
        .any(|entry| entry.inline_text() == Some("popped later")));
    assert!(loaded
        .history()
        .iter()
        .any(|entry| entry.external_markdown_path() == Some(markdown.as_path())));
}

#[test]
fn save_writes_one_file_per_tab_and_prompt() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace
        .add_prompt(first, Prompt::new("one").unwrap())
        .unwrap();
    workspace
        .add_prompt(first, Prompt::new("two").unwrap())
        .unwrap();
    workspace.create_tab("work").unwrap();

    save_dir(&dir, &workspace).unwrap();

    assert_eq!(count_json(&dir.join("tabs")), 2);
    assert_eq!(count_json(&dir.join("prompts")), 2);
    assert!(dir.join("history.json").exists());
}

#[test]
fn save_removes_files_of_deleted_prompts_and_tabs() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    let id = workspace
        .add_prompt(first, Prompt::new("transient").unwrap())
        .unwrap();
    let tab = workspace.create_tab("closing").unwrap();
    save_dir(&dir, &workspace).unwrap();

    workspace.remove_prompt(id).unwrap();
    workspace.close_tab(tab).unwrap();
    save_dir(&dir, &workspace).unwrap();

    assert_eq!(count_json(&dir.join("tabs")), 1);
    assert_eq!(count_json(&dir.join("prompts")), 0);
}

#[test]
fn save_requires_an_initialized_workspace_directory() {
    let root = TempDir::new().unwrap();
    let dir = root.path().join("not-a-workspace");
    fs::create_dir_all(&dir).unwrap();
    assert!(matches!(
        save_dir(&dir, &Workspace::new()),
        Err(CoreError::Invalid(_))
    ));
}

#[test]
fn unsupported_dir_schema_is_rejected() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    fs::write(
        dir.join("workspace.json"),
        serde_json::json!({"schema": 99, "id": Uuid::new_v4(), "name": "x"}).to_string(),
    )
    .unwrap();
    assert!(matches!(
        load_dir(&dir),
        Err(CoreError::UnsupportedSchema(99))
    ));
}

#[test]
fn prompt_referencing_unknown_tab_is_rejected() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    save_dir(&dir, &Workspace::new()).unwrap();
    let orphan = serde_json::json!({
        "tab": Uuid::new_v4(),
        "id": Uuid::new_v4(),
        "source": {"type": "inline", "text": "orphan"},
        "created_at": Utc::now(),
    });
    fs::write(dir.join("prompts").join("orphan.json"), orphan.to_string()).unwrap();
    assert!(matches!(load_dir(&dir), Err(CoreError::TabNotFound(_))));
}

#[test]
fn save_is_atomic_no_tmp_left_behind() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("hello").unwrap())
        .unwrap();
    save_dir(&dir, &workspace).unwrap();
    for sub in ["tabs", "prompts"] {
        for entry in fs::read_dir(dir.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(path.extension().and_then(|e| e.to_str()), Some("json"));
        }
    }
    assert!(!dir.join("history.json.tmp").exists());
}

#[test]
fn unchanged_prompt_files_are_not_rewritten() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let id = workspace
        .add_prompt(tab, Prompt::new("stable").unwrap())
        .unwrap();
    save_dir(&dir, &workspace).unwrap();
    let prompt_path = dir.join("prompts").join(format!("{}.json", id.0));
    let old = fs::metadata(&prompt_path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));

    workspace
        .add_prompt(tab, Prompt::new("new").unwrap())
        .unwrap();
    save_dir(&dir, &workspace).unwrap();

    assert_eq!(fs::metadata(&prompt_path).unwrap().modified().unwrap(), old);
}

#[test]
fn fingerprint_tracks_missing_dir_and_changes() {
    let root = TempDir::new().unwrap();
    assert_eq!(fingerprint_dir(&root.path().join("absent")).unwrap(), None);

    let dir = init_dir(root.path(), "Personal").unwrap();
    let before = fingerprint_dir(&dir).unwrap().unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("changes the dir").unwrap())
        .unwrap();
    save_dir(&dir, &workspace).unwrap();
    let after = fingerprint_dir(&dir).unwrap().unwrap();
    assert_ne!(before, after);
    assert_eq!(fingerprint_dir(&dir).unwrap().unwrap(), after);
}

#[test]
fn pinned_at_is_persisted_and_unpinned_prompts_omit_it() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "Personal").unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let pinned = workspace
        .add_prompt(tab, Prompt::new("pinned").unwrap())
        .unwrap();
    workspace.set_prompt_pinned(pinned, true).unwrap();
    let plain = workspace
        .add_prompt(tab, Prompt::new("plain").unwrap())
        .unwrap();
    save_dir(&dir, &workspace).unwrap();

    let pinned_json =
        fs::read_to_string(dir.join("prompts").join(format!("{}.json", pinned.0))).unwrap();
    let plain_json =
        fs::read_to_string(dir.join("prompts").join(format!("{}.json", plain.0))).unwrap();
    assert!(pinned_json.contains("pinned_at"));
    assert!(!plain_json.contains("pinned_at"));
}

#[test]
fn read_meta_and_rename_preserve_the_workspace_id() {
    let root = TempDir::new().unwrap();
    let dir = init_dir(root.path(), "before").unwrap();
    let meta = read_meta(&dir).unwrap();
    assert_eq!(meta.name, "before");

    rename_dir(&dir, "after").unwrap();
    let renamed = read_meta(&dir).unwrap();
    assert_eq!(renamed.name, "after");
    assert_eq!(renamed.id, meta.id);
}

#[test]
fn list_dirs_returns_workspaces_sorted_by_name() {
    let root = TempDir::new().unwrap();
    init_dir(root.path(), "zeta").unwrap();
    init_dir(root.path(), "Alpha").unwrap();
    fs::create_dir_all(root.path().join("not-a-workspace")).unwrap();

    let listed = list_dirs(root.path()).unwrap();
    let names: Vec<_> = listed.iter().map(|(_, meta)| meta.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "zeta"]);

    assert!(list_dirs(&root.path().join("absent")).unwrap().is_empty());
}

fn count_json(dir: &Path) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .and_then(|e| e.to_str())
                == Some("json")
        })
        .count()
}

// --- Legacy single-file format (schemas 1-4) ---

#[test]
fn load_missing_legacy_file_returns_initial_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let workspace = load_legacy_file(&path).unwrap();
    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
}

#[test]
fn load_empty_legacy_file_returns_initial_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    fs::write(&path, "").unwrap();
    assert_eq!(load_legacy_file(&path).unwrap().tabs().len(), 1);
}

#[test]
fn schema_one_migrates_without_prompt_loss_or_eager_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let id = Uuid::new_v4();
    let created_at = Utc::now();
    let text = "legacy\nтекст";
    let legacy = serde_json::json!({
        "schema": 1,
        "queue": {
            "prompts": [{
                "id": id,
                "text": text,
                "pinned": true,
                "created_at": created_at,
            }]
        }
    });
    fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

    let workspace = load_legacy_file(&path).unwrap();
    let migrated = workspace
        .get_prompt(crate::PromptId(id))
        .expect("legacy prompt");

    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
    assert_eq!(migrated.inline_text(), Some(text));
    assert!(migrated.pinned());
    assert_eq!(migrated.pinned_at, Some(created_at));
    assert_eq!(migrated.created_at, created_at);
    assert_eq!(workspace.history()[0].inline_text(), Some(text));
    assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 1"));
}

#[test]
fn schema_two_migrates_prompt_text_and_seeds_history() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("remembered exactly\n").unwrap())
        .unwrap();
    let mut legacy_workspace = legacy_workspace_value(&workspace);
    legacy_workspace.as_object_mut().unwrap().remove("history");
    fs::write(
        &path,
        serde_json::to_string(&serde_json::json!({
            "schema": 2,
            "workspace": legacy_workspace,
        }))
        .unwrap(),
    )
    .unwrap();

    let loaded = load_legacy_file(&path).unwrap();

    assert_eq!(loaded.history().len(), 1);
    assert_eq!(
        loaded.history()[0].inline_text(),
        Some("remembered exactly\n")
    );
    assert!(fs::read_to_string(&path).unwrap().contains("\"schema\":2"));
}

#[test]
fn schema_three_migrates_prompt_and_history_text_without_loss() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let removed = workspace
        .add_prompt(tab, Prompt::new("historical\nvalue").unwrap())
        .unwrap();
    workspace.remove_prompt(removed).unwrap();
    workspace
        .add_prompt(tab, Prompt::new("still queued").unwrap())
        .unwrap();
    let legacy = serde_json::json!({
        "schema": 3,
        "workspace": legacy_workspace_value(&workspace),
    });
    fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

    let loaded = load_legacy_file(&path).unwrap();

    assert_eq!(loaded.get_prompt(removed), None);
    let history: Vec<_> = loaded
        .history()
        .iter()
        .map(|entry| entry.inline_text().unwrap())
        .collect();
    assert_eq!(history, vec!["still queued", "historical\nvalue"]);
    assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 3"));
}

#[test]
fn schema_three_migration_keeps_the_entry_crossing_the_history_budget() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("o".repeat(100 * 1024)).unwrap())
        .unwrap();
    workspace
        .add_prompt(tab, Prompt::new("n".repeat(200 * 1024)).unwrap())
        .unwrap();
    assert_eq!(workspace.history().len(), 2);
    let legacy = serde_json::json!({
        "schema": 3,
        "workspace": legacy_workspace_value(&workspace),
    });
    fs::write(&path, serde_json::to_string(&legacy).unwrap()).unwrap();

    let loaded = load_legacy_file(&path).unwrap();

    assert_eq!(loaded.history().len(), 2);
    assert_eq!(loaded.history()[0].inline_text().unwrap().len(), 200 * 1024);
    assert_eq!(loaded.history()[1].inline_text().unwrap().len(), 100 * 1024);
}

#[test]
fn schema_four_pinned_flag_migrates_to_pinned_at() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let pinned_id = Uuid::new_v4();
    let plain_id = Uuid::new_v4();
    let created_at = Utc::now();
    let legacy = serde_json::json!({
        "schema": 4,
        "workspace": {
            "tabs": [{
                "id": Uuid::nil(),
                "name": "1",
                "activity_at": created_at,
                "queue": {
                    "prompts": [
                        {
                            "id": pinned_id,
                            "source": {"type": "inline", "text": "pinned"},
                            "pinned": true,
                            "created_at": created_at,
                        },
                        {
                            "id": plain_id,
                            "source": {"type": "inline", "text": "plain"},
                            "pinned": false,
                            "created_at": created_at,
                        },
                    ]
                }
            }],
            "history": []
        }
    });
    fs::write(&path, serde_json::to_string(&legacy).unwrap()).unwrap();

    let loaded = load_legacy_file(&path).unwrap();

    let pinned = loaded.get_prompt(crate::PromptId(pinned_id)).unwrap();
    assert_eq!(pinned.pinned_at, Some(created_at));
    assert!(!loaded
        .get_prompt(crate::PromptId(plain_id))
        .unwrap()
        .pinned());
}

#[test]
fn unsupported_legacy_schema_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    fs::write(&path, r#"{"schema":99}"#).unwrap();
    assert!(matches!(
        load_legacy_file(&path),
        Err(CoreError::UnsupportedSchema(99))
    ));
}

fn legacy_workspace_value(workspace: &Workspace) -> Value {
    let mut value = serde_json::to_value(workspace).unwrap();
    for tab in value["tabs"].as_array_mut().unwrap() {
        for prompt in tab["queue"]["prompts"].as_array_mut().unwrap() {
            source_to_legacy_text(prompt);
        }
    }
    for entry in value["history"].as_array_mut().unwrap() {
        source_to_legacy_text(entry);
    }
    value
}

fn source_to_legacy_text(value: &mut Value) {
    let object = value.as_object_mut().unwrap();
    let source = object.remove("source").unwrap();
    assert_eq!(source["type"], "inline");
    object.insert("text".to_string(), source["text"].clone());
}

#[test]
fn typed_source_identity_distinguishes_inline_text_from_an_external_path() {
    let path = std::env::temp_dir().join("identity.md");
    let inline = PromptSource::inline(path.to_str().unwrap()).unwrap();
    let external = PromptSource::external_markdown(path).unwrap();
    assert_ne!(inline, external);
}
