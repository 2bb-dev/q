use super::*;
use crate::{Prompt, PromptSource};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn load_missing_file_returns_initial_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let workspace = load(&path).unwrap();
    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
}

#[test]
fn save_then_load_roundtrips_typed_sources() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let markdown = dir.path().join("not-created.md");
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace
        .add_prompt(first, Prompt::new("hello").unwrap())
        .unwrap();
    let work = workspace.create_tab("work").unwrap();
    workspace
        .add_prompt(work, Prompt::from_external_markdown(&markdown).unwrap())
        .unwrap();

    save(&path, &workspace).unwrap();
    let loaded = load(&path).unwrap();

    assert_eq!(loaded.tabs().len(), 2);
    assert_eq!(loaded.tab(work).unwrap().queue().len(), 1);
    assert_eq!(loaded.tab(first).unwrap().queue().len(), 1);
    assert!(loaded
        .history()
        .iter()
        .any(|entry| { entry.external_markdown_path() == Some(markdown.as_path()) }));
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

    let workspace = load(&path).unwrap();
    let migrated = workspace
        .get_prompt(crate::PromptId(id))
        .expect("legacy prompt");

    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
    assert_eq!(migrated.inline_text(), Some(text));
    assert!(migrated.pinned);
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

    let loaded = load(&path).unwrap();

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

    let loaded = load(&path).unwrap();

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

    let loaded = load(&path).unwrap();

    assert_eq!(loaded.history().len(), 2);
    assert_eq!(loaded.history()[0].inline_text().unwrap().len(), 200 * 1024);
    assert_eq!(loaded.history()[1].inline_text().unwrap().len(), 100 * 1024);
}

#[test]
fn schema_four_serializes_explicit_tagged_sources() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let markdown = dir.path().join("live.md");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("inline").unwrap())
        .unwrap();
    workspace
        .add_prompt(tab, Prompt::from_external_markdown(&markdown).unwrap())
        .unwrap();

    save(&path, &workspace).unwrap();
    let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

    assert_eq!(value["schema"], 4);
    let prompts = value["workspace"]["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|tab| tab["queue"]["prompts"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert!(prompts.iter().any(|prompt| {
        prompt["source"] == serde_json::json!({"type": "inline", "text": "inline"})
    }));
    assert!(prompts.iter().any(|prompt| {
        prompt["source"]["type"] == "external_markdown"
            && prompt["source"]["path"] == markdown.to_str().unwrap()
    }));
    assert!(value["workspace"]["history"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry.get("source").is_some() && entry.get("text").is_none()));
}

#[test]
fn unsupported_schema_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    fs::write(&path, r#"{"schema":99}"#).unwrap();
    assert!(matches!(load(&path), Err(CoreError::UnsupportedSchema(99))));
}

#[test]
fn save_is_atomic_no_tmp_left_behind() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    save(&path, &Workspace::new()).unwrap();
    assert!(path.exists());
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn load_empty_file_returns_initial_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    fs::write(&path, "").unwrap();
    assert_eq!(load(&path).unwrap().tabs().len(), 1);
}

#[test]
fn schema_version_is_written() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    save(&path, &Workspace::new()).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 4"));
}

#[test]
fn history_survives_save_and_load_without_the_prompt() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let id = workspace
        .add_prompt(tab, Prompt::new("popped later").unwrap())
        .unwrap();
    workspace.remove_prompt(id).unwrap();

    save(&path, &workspace).unwrap();
    let loaded = load(&path).unwrap();

    assert!(loaded.get_prompt(id).is_none());
    assert_eq!(loaded.history()[0].inline_text(), Some("popped later"));
}

#[test]
fn fingerprint_tracks_missing_and_saved_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    assert_eq!(fingerprint(&path).unwrap(), None);

    save(&path, &Workspace::new()).unwrap();
    assert!(fingerprint(&path).unwrap().is_some());
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
