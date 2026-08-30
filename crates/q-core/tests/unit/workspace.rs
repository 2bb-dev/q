use super::*;
use chrono::Duration;

#[test]
fn new_workspace_starts_with_tab_one() {
    let workspace = Workspace::new();
    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.tabs()[0].name(), "1");
    assert!(workspace.tabs()[0].queue().is_empty());
}

#[test]
fn tab_names_are_trimmed_and_case_insensitively_unique() {
    let mut workspace = Workspace::new();
    let id = workspace.create_tab("  Work  ").unwrap();
    assert_eq!(workspace.tab(id).unwrap().name(), "Work");
    assert!(workspace.create_tab("work").is_err());
    assert!(workspace.create_tab("  ").is_err());
}

#[test]
fn rename_preserves_tab_data_and_order() {
    let now = Utc::now();
    let mut workspace = Workspace::with_initial_activity(now);
    let id = workspace
        .create_tab_with(TabId::new(), "work", now + Duration::seconds(1))
        .unwrap();
    let prompt = Prompt::new("hello").unwrap();
    let prompt_id = prompt.id;
    workspace.add_prompt(id, prompt).unwrap();
    let activity = workspace.tab(id).unwrap().activity_at();

    workspace.rename_tab(id, "renamed").unwrap();

    let tab = workspace.tab(id).unwrap();
    assert_eq!(tab.name(), "renamed");
    assert_eq!(tab.activity_at(), activity);
    assert_eq!(
        tab.queue().get(prompt_id).unwrap().inline_text().unwrap(),
        "hello"
    );
}

#[test]
fn adding_prompt_moves_tab_first() {
    let now = Utc::now();
    let mut workspace = Workspace::with_initial_activity(now);
    let second = workspace
        .create_tab_with(TabId::new(), "second", now + Duration::seconds(1))
        .unwrap();
    let first = workspace.resolve_tab("1").unwrap();
    assert_eq!(workspace.first_tab_id(), second);

    let mut prompt = Prompt::new("latest").unwrap();
    prompt.created_at = now + Duration::seconds(2);
    workspace.add_prompt(first, prompt).unwrap();

    assert_eq!(workspace.first_tab_id(), first);
}

#[test]
fn close_tab_removes_it_and_its_prompts() {
    let mut workspace = Workspace::new();
    let closed = workspace.create_tab("closed").unwrap();
    let kept = workspace.resolve_tab("1").unwrap();
    let prompt = Prompt::new("discarded").unwrap();
    let prompt_id = prompt.id;
    workspace.add_prompt(closed, prompt).unwrap();

    workspace.close_tab(closed).unwrap();

    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.first_tab_id(), kept);
    assert!(workspace.get_prompt(prompt_id).is_none());
}

#[test]
fn last_tab_cannot_be_closed() {
    let mut workspace = Workspace::new();
    let only = workspace.first_tab_id();

    let error = workspace.close_tab(only).unwrap_err();

    assert_eq!(error.to_string(), "invalid tab: cannot close the last tab");
    assert_eq!(workspace.tabs().len(), 1);
}

#[test]
fn out_of_order_prompt_add_does_not_regress_tab_activity() {
    let now = Utc::now();
    let mut workspace = Workspace::with_initial_activity(now);
    let first = workspace.first_tab_id();
    let second = workspace
        .create_tab_with(TabId::new(), "second", now + Duration::seconds(5))
        .unwrap();
    let mut newer = Prompt::new("newer").unwrap();
    newer.created_at = now + Duration::seconds(10);
    workspace.add_prompt(first, newer).unwrap();
    let mut older = Prompt::new("older committed later").unwrap();
    older.created_at = now + Duration::seconds(2);

    workspace.add_prompt(first, older).unwrap();

    assert_eq!(workspace.first_tab_id(), first);
    assert_eq!(
        workspace.tab(first).unwrap().activity_at(),
        now + Duration::seconds(10)
    );
    assert_eq!(workspace.tabs()[1].id(), second);
}

#[test]
fn context_requires_name_only_when_multiple_tabs_exist() {
    let mut workspace = Workspace::new();
    assert!(workspace.resolve_context_tab(None).is_ok());
    workspace.create_tab("work").unwrap();
    assert!(matches!(
        workspace.resolve_context_tab(None),
        Err(CoreError::TabRequired(_))
    ));
    assert_eq!(
        workspace.resolve_context_tab(Some("WORK")).unwrap(),
        workspace.resolve_tab("work").unwrap()
    );
}

#[test]
fn history_outlives_prompts_and_tabs() {
    let mut workspace = Workspace::new();
    let closed = workspace.create_tab("closed").unwrap();
    let kept = workspace.resolve_tab("1").unwrap();
    let id = workspace
        .add_prompt(kept, Prompt::new("copied away").unwrap())
        .unwrap();
    workspace
        .add_prompt(closed, Prompt::new("tab is gone").unwrap())
        .unwrap();

    workspace.remove_prompt(id).unwrap();
    workspace.close_tab(closed).unwrap();

    let texts: Vec<_> = workspace
        .history()
        .iter()
        .map(|entry| entry.inline_text().unwrap())
        .collect();
    assert_eq!(texts, vec!["tab is gone", "copied away"]);
}

#[test]
fn re_adding_the_same_text_moves_one_entry_to_the_top() {
    let now = Utc::now();
    let mut workspace = Workspace::with_initial_activity(now);
    let tab = workspace.first_tab_id();
    for (index, text) in ["first", "second", "first"].into_iter().enumerate() {
        let mut prompt = Prompt::new(text).unwrap();
        prompt.created_at = now + Duration::seconds(index as i64);
        workspace.add_prompt(tab, prompt).unwrap();
    }

    let texts: Vec<_> = workspace
        .history()
        .iter()
        .map(|entry| entry.inline_text().unwrap())
        .collect();
    assert_eq!(texts, vec!["first", "second"]);
}

#[test]
fn history_deduplicates_by_typed_source_identity() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let path = std::env::temp_dir().join("same.md");
    let path_text = path.to_str().unwrap().to_string();

    workspace
        .add_prompt(tab, Prompt::new(&path_text).unwrap())
        .unwrap();
    workspace
        .add_prompt(tab, Prompt::from_external_markdown(&path).unwrap())
        .unwrap();
    workspace
        .add_prompt(tab, Prompt::from_external_markdown(&path).unwrap())
        .unwrap();

    assert_eq!(workspace.history().len(), 2);
    assert_eq!(
        workspace.history()[0].external_markdown_path(),
        Some(path.as_path())
    );
    assert_eq!(
        workspace.history()[1].inline_text(),
        Some(path_text.as_str())
    );
}

#[test]
fn workspace_inline_edit_records_new_history_and_retains_old_text() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let mut prompt = Prompt::new("before").unwrap();
    prompt.set_pinned(true);
    let id = prompt.id;
    let created_at = prompt.created_at;
    workspace.add_prompt(tab, prompt).unwrap();

    workspace.edit_prompt_inline(id, "after").unwrap();

    let edited = workspace.get_prompt(id).unwrap();
    assert_eq!(edited.id, id);
    assert_eq!(edited.created_at, created_at);
    assert!(edited.pinned());
    assert_eq!(edited.inline_text(), Some("after"));
    let history: Vec<_> = workspace
        .history()
        .iter()
        .map(|entry| entry.inline_text().unwrap())
        .collect();
    assert_eq!(history, vec!["after", "before"]);
}

#[test]
fn workspace_inline_edit_rejects_an_external_source_without_new_history() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let path = std::env::temp_dir().join("live.md");
    let id = workspace
        .add_prompt(tab, Prompt::from_external_markdown(&path).unwrap())
        .unwrap();

    assert!(workspace.edit_prompt_inline(id, "inline now").is_err());
    assert_eq!(workspace.history().len(), 1);
    assert_eq!(
        workspace.get_prompt(id).unwrap().external_markdown_path(),
        Some(path.as_path())
    );
}

#[test]
fn history_keeps_only_the_newest_entries() {
    let now = Utc::now();
    let mut workspace = Workspace::with_initial_activity(now);
    let tab = workspace.first_tab_id();
    for index in 0..HISTORY_LIMIT + 10 {
        let mut prompt = Prompt::new(format!("prompt-{index}")).unwrap();
        prompt.created_at = now + Duration::seconds(index as i64);
        workspace.add_prompt(tab, prompt).unwrap();
    }

    assert_eq!(workspace.history().len(), HISTORY_LIMIT);
    assert_eq!(
        workspace.history()[0].inline_text().unwrap(),
        format!("prompt-{}", HISTORY_LIMIT + 9)
    );
}

#[test]
fn history_respects_the_byte_budget_as_well_as_the_entry_count() {
    let now = Utc::now();
    let mut workspace = Workspace::with_initial_activity(now);
    let tab = workspace.first_tab_id();
    let big = "x".repeat(64 * 1024);
    for index in 0..10 {
        let mut prompt = Prompt::new(format!("{big}{index}")).unwrap();
        prompt.created_at = now + Duration::seconds(index);
        workspace.add_prompt(tab, prompt).unwrap();
    }

    let bytes: usize = workspace
        .history()
        .iter()
        .map(|entry| entry.source().byte_len())
        .sum();
    assert!(
        workspace.history().len() < 10,
        "oldest entries should have been dropped"
    );
    assert!(
        bytes <= HISTORY_BYTE_BUDGET + big.len(),
        "history kept {bytes} bytes"
    );
    // Newest survives.
    assert!(workspace.history()[0].inline_text().unwrap().ends_with('9'));
}

#[test]
fn external_path_bytes_count_toward_the_history_budget() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let component = "p".repeat(HISTORY_BYTE_BUDGET / 2);
    for index in 0..3 {
        let path = std::env::temp_dir().join(format!("{component}-{index}.md"));
        workspace
            .add_prompt(tab, Prompt::from_external_markdown(path).unwrap())
            .unwrap();
    }

    assert_eq!(workspace.history().len(), 2);
    assert!(workspace
        .history()
        .iter()
        .all(|entry| entry.external_markdown_path().is_some()));
}

#[test]
fn a_single_oversized_prompt_is_still_remembered() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let huge = "y".repeat(HISTORY_BYTE_BUDGET * 2);
    workspace
        .add_prompt(tab, Prompt::new(huge).unwrap())
        .unwrap();

    assert_eq!(workspace.history().len(), 1);
}

#[test]
fn forget_history_removes_one_entry_by_exact_text() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    for text in ["keep me", "secret token"] {
        workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }

    assert!(workspace.forget_inline_history("secret token"));
    assert!(!workspace.forget_inline_history("secret token"));

    let texts: Vec<_> = workspace
        .history()
        .iter()
        .map(|entry| entry.inline_text().unwrap())
        .collect();
    assert_eq!(texts, vec!["keep me"]);
}

#[test]
fn forget_history_matching_uses_language_agnostic_search() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    for text in ["улучшить конфиги", "unrelated prompt"] {
        workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }

    assert_eq!(workspace.forget_history_matching("uluchshit"), 1);

    let texts: Vec<_> = workspace
        .history()
        .iter()
        .map(|entry| entry.inline_text().unwrap())
        .collect();
    assert_eq!(texts, vec!["unrelated prompt"]);
}

#[test]
fn clear_history_forgets_everything_but_keeps_queued_prompts() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let id = workspace
        .add_prompt(tab, Prompt::new("still queued").unwrap())
        .unwrap();

    assert_eq!(workspace.clear_history(), 1);

    assert!(workspace.history().is_empty());
    assert!(workspace.get_prompt(id).is_some());
}

#[test]
fn prompt_operations_find_owning_tab_globally() {
    let mut workspace = Workspace::new();
    let second = workspace.create_tab("second").unwrap();
    let prompt = Prompt::new("global").unwrap();
    let id = prompt.id;
    workspace.add_prompt(second, prompt).unwrap();

    assert_eq!(workspace.resolve_prompt(&id.to_string()).unwrap(), id);
    workspace.set_prompt_pinned(id, true).unwrap();
    assert!(workspace.get_prompt(id).unwrap().pinned());
    assert_eq!(
        workspace.remove_prompt(id).unwrap().inline_text().unwrap(),
        "global"
    );
    assert!(workspace.get_prompt(id).is_none());
}
