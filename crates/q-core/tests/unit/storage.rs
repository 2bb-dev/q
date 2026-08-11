use super::*;
use crate::prompt::Prompt;
use tempfile::TempDir;

#[test]
fn load_missing_file_returns_initial_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let workspace = load(&path).unwrap();
    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
}

#[test]
fn save_then_load_roundtrips() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace
        .add_prompt(first, Prompt::new("hello").unwrap())
        .unwrap();
    let work = workspace.create_tab("work").unwrap();
    workspace
        .add_prompt(work, Prompt::new("world").unwrap())
        .unwrap();

    save(&path, &workspace).unwrap();
    let loaded = load(&path).unwrap();

    assert_eq!(loaded.tabs().len(), 2);
    assert_eq!(loaded.tab(work).unwrap().queue().len(), 1);
    assert_eq!(loaded.tab(first).unwrap().queue().len(), 1);
}

#[test]
fn schema_one_migrates_without_prompt_loss() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut queue = Queue::new();
    let mut prompt = Prompt::new("legacy").unwrap();
    prompt.pinned = true;
    let id = prompt.id;
    let created_at = prompt.created_at;
    queue.add(prompt);
    let legacy = QueueFileV1 { schema: 1, queue };
    fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

    let workspace = load(&path).unwrap();
    let migrated = workspace.get_prompt(id).unwrap();

    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
    assert_eq!(migrated.text, "legacy");
    assert!(migrated.pinned);
    assert_eq!(migrated.created_at, created_at);
    assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 1"));
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
    assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 3"));
}

#[test]
fn schema_two_seeds_history_from_queued_prompts() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("remembered").unwrap())
        .unwrap();
    let legacy = format!(
        r#"{{"schema":2,"workspace":{}}}"#,
        serde_json::to_string(&workspace).unwrap()
    );
    fs::write(&path, legacy).unwrap();

    let loaded = load(&path).unwrap();

    assert_eq!(loaded.history().len(), 1);
    assert_eq!(loaded.history()[0].text, "remembered");
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
    assert_eq!(loaded.history()[0].text, "popped later");
}

#[test]
fn fingerprint_tracks_missing_and_saved_workspace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.json");
    assert_eq!(fingerprint(&path).unwrap(), None);

    save(&path, &Workspace::new()).unwrap();
    assert!(fingerprint(&path).unwrap().is_some());
}
