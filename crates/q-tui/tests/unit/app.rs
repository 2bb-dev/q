use super::*;
use q_core::Queue;

#[test]
fn new_app_has_queue_focus_and_selects_first_prompt() {
    let mut queue = Queue::new();
    queue.add_text("hello").unwrap();
    let app = App::new(queue);
    assert_eq!(app.focus, Pane::Queue);
    assert_eq!(app.selected, Some(0));
    assert_eq!(app.composer.text(), "");
}

#[test]
fn empty_workspace_has_no_selection() {
    let app = App::new(Workspace::new());
    assert_eq!(app.selected, None);
}

#[test]
fn selecting_empty_tab_focuses_composer() {
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace
        .add_prompt(first, Prompt::new("first").unwrap())
        .unwrap();
    let empty = workspace.create_tab("empty").unwrap();
    let mut app = App::new(workspace);
    app.select_tab(empty);
    assert_eq!(app.focus, Pane::Composer);
    assert_eq!(app.selected, None);
}

#[test]
fn replace_workspace_closes_preview_of_removed_prompt() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = Prompt::new("preview me").unwrap();
    let id = prompt.id;
    workspace.add_prompt(tab, prompt).unwrap();
    let mut app = App::new(workspace);
    app.preview = Some(PromptPreview {
        source: PreviewSource::Prompt(id),
        scroll: 3,
    });

    app.replace_workspace(Workspace::new());

    assert_eq!(app.preview, None);
}

#[test]
fn history_preview_survives_workspace_reload() {
    let mut app = App::new(Workspace::new());
    app.preview = Some(PromptPreview {
        source: PreviewSource::History(PromptSource::inline("gone from the queue").unwrap()),
        scroll: 0,
    });

    app.replace_workspace(Workspace::new());

    assert_eq!(app.preview_text().as_deref(), Ok("gone from the queue"));
}

#[test]
fn search_results_filter_history_case_insensitively() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    for text in ["deploy the API", "write tests"] {
        workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }
    let mut app = App::new(workspace);
    app.search = Some(SearchDialog::default());

    assert_eq!(app.search_results().len(), 2);

    app.search.as_mut().unwrap().query = "api".to_string();
    let results = app.search_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].inline_text(), Some("deploy the API"));
}

#[test]
fn composer_starts_at_end_of_loaded_text() {
    let mut composer = ComposerEditor::from_text("first\nsecond");
    composer.insert_char('!');
    assert_eq!(composer.text(), "first\nsecond!");
}

#[test]
fn replace_workspace_preserves_tab_selection_and_composer() {
    let mut workspace = Workspace::new();
    let tab = workspace.create_tab("work").unwrap();
    let prompt = Prompt::new("selected").unwrap();
    let selected_id = prompt.id;
    workspace.add_prompt(tab, prompt).unwrap();
    let mut app = App::new(workspace.clone());
    app.select_tab(tab);
    app.focus = Pane::Composer;
    app.composer.set_text("draft");

    app.replace_workspace(workspace);

    assert_eq!(app.active_tab_id, tab);
    assert_eq!(
        app.selected_prompt().map(|prompt| prompt.id),
        Some(selected_id)
    );
    assert_eq!(app.composer.text(), "draft");
    assert_eq!(app.focus, Pane::Composer);
}

#[test]
fn external_cache_refreshes_live_content_and_searches_path_and_text() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("live.md");
    std::fs::write(&path, "first value").unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = Prompt::from_external_markdown(path.clone()).unwrap();
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    let mut app = App::new(workspace);

    assert_eq!(app.resolve_source(&source).as_deref(), Ok("first value"));
    app.search = Some(SearchDialog {
        query: "live.md".to_string(),
        selected: 0,
    });
    assert_eq!(app.search_results().len(), 1);

    std::fs::write(&path, "second value").unwrap();
    app.refresh_external_content();
    app.search.as_mut().unwrap().query = "second".to_string();
    assert_eq!(app.search_results().len(), 1);
    assert_eq!(app.resolve_source(&source).as_deref(), Ok("second value"));
}

#[test]
fn forced_external_refresh_rereads_even_when_metadata_might_be_unchanged() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("live.md");
    std::fs::write(&path, "same size A").unwrap();
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = Prompt::from_external_markdown(path.clone()).unwrap();
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    let mut app = App::new(workspace);

    std::fs::write(&path, "same size B").unwrap();
    app.refresh_external_content_forced();

    assert_eq!(app.resolve_source(&source).as_deref(), Ok("same size B"));
}

#[test]
fn broken_reference_keeps_absolute_path_and_moved_deleted_error_visible() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("gone.md");
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = Prompt::from_external_markdown(path.clone()).unwrap();
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    let app = App::new(workspace);

    let card = app.source_card_text(&source);
    assert!(card.contains(path.to_str().unwrap()));
    assert!(card.contains("moved or deleted"));
}
