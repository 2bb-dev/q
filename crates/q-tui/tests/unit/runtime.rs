use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn with_mods(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn map_key(key: KeyEvent, focus: Pane) -> Option<Input> {
    super::map_key(key, focus, false)
}

#[test]
fn q_in_queue_pane_quits() {
    assert_eq!(
        map_key(key(KeyCode::Char('q')), Pane::Queue),
        Some(Input::Quit)
    );
}

#[test]
fn q_in_composer_pane_is_a_char() {
    assert_eq!(
        map_key(key(KeyCode::Char('q')), Pane::Composer),
        Some(Input::Char('q'))
    );
}

#[test]
fn ctrl_c_always_quits() {
    for pane in [Pane::Queue, Pane::Composer] {
        assert_eq!(
            map_key(with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL), pane),
            Some(Input::Quit)
        );
    }
}

#[test]
fn ctrl_s_is_submit() {
    assert_eq!(
        map_key(
            with_mods(KeyCode::Char('s'), KeyModifiers::CONTROL),
            Pane::Composer
        ),
        Some(Input::CtrlS)
    );
}

#[test]
fn ctrl_c_in_full_screen_editor_uses_safe_close_flow() {
    assert_eq!(
        map_editor_key(with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Some(Input::Esc)
    );
}

#[test]
fn modified_backspace_maps_to_editor_actions() {
    assert_eq!(
        map_key(
            with_mods(KeyCode::Backspace, KeyModifiers::SUPER),
            Pane::Composer
        ),
        Some(Input::DeleteToLineStart)
    );
    assert_eq!(
        map_key(
            with_mods(KeyCode::Backspace, KeyModifiers::ALT),
            Pane::Composer
        ),
        Some(Input::DeleteWordBack)
    );
    assert_eq!(
        map_key(
            with_mods(KeyCode::Char('u'), KeyModifiers::CONTROL),
            Pane::Composer
        ),
        Some(Input::DeleteToLineStart)
    );
}

#[test]
fn composer_navigation_keys_map_to_editor_movements() {
    assert_eq!(
        map_key(key(KeyCode::Left), Pane::Composer),
        Some(Input::MoveLeft)
    );
    assert_eq!(
        map_key(with_mods(KeyCode::Left, KeyModifiers::ALT), Pane::Composer),
        Some(Input::MoveWordLeft)
    );
    assert_eq!(
        map_key(key(KeyCode::Home), Pane::Composer),
        Some(Input::MoveLineStart)
    );
    assert_eq!(
        map_key(key(KeyCode::Delete), Pane::Composer),
        Some(Input::Delete)
    );
}

#[test]
fn shifted_or_alt_enter_inserts_newline() {
    assert_eq!(
        map_key(
            with_mods(KeyCode::Enter, KeyModifiers::SHIFT),
            Pane::Composer
        ),
        Some(Input::Newline)
    );
    assert_eq!(
        map_key(with_mods(KeyCode::Enter, KeyModifiers::ALT), Pane::Composer),
        Some(Input::Newline)
    );
}

#[test]
fn mouse_click_selects_tab_and_opens_create_dialog() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let initial = workspace.first_tab_id();
    let target = workspace.create_tab("work").unwrap();
    let mut app = App::new(workspace);
    app.select_tab(initial);
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();
    let area = ratatui::layout::Rect::new(2, 1, 6, 1);
    app.tab_hits = vec![crate::app::TabHit {
        area,
        target: crate::app::TabHitTarget::Tab(target),
    }];
    let click = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });

    assert!(!handle_event(
        click,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path)
    )
    .unwrap());
    assert_eq!(app.active_tab_id, target);

    app.tab_hits[0].target = crate::app::TabHitTarget::Create;
    let click_create = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!handle_event(
        click_create,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path)
    )
    .unwrap());
    assert!(app.tab_dialog.is_some());
}

#[test]
fn left_click_focuses_prompt_and_composer() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, q_core::Prompt::new("prompt").unwrap())
        .unwrap();
    let mut app = App::new(workspace);
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();
    let prompt_area = ratatui::layout::Rect::new(0, 3, 20, 1);
    let composer_area = ratatui::layout::Rect::new(0, 10, 20, 1);
    app.prompt_hits = vec![crate::app::PromptHit {
        area: prompt_area,
        index: 0,
    }];
    app.composer_area = Some(composer_area);
    app.focus = Pane::Composer;

    let click_prompt = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: prompt_area.x,
        row: prompt_area.y,
        modifiers: KeyModifiers::NONE,
    });
    handle_event(
        click_prompt,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();
    assert_eq!(app.focus, Pane::Queue);
    assert_eq!(app.selected, Some(0));

    let click_composer = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: composer_area.x,
        row: composer_area.y,
        modifiers: KeyModifiers::NONE,
    });
    handle_event(
        click_composer,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();
    assert_eq!(app.focus, Pane::Composer);
}

#[test]
fn right_click_tab_and_click_rename_opens_dialog() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let target = workspace.create_tab("work").unwrap();
    let mut app = App::new(workspace);
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();
    let tab_area = ratatui::layout::Rect::new(2, 1, 6, 1);
    app.tab_hits = vec![crate::app::TabHit {
        area: tab_area,
        target: crate::app::TabHitTarget::Tab(target),
    }];
    let right_click = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: tab_area.x,
        row: tab_area.y,
        modifiers: KeyModifiers::NONE,
    });

    assert!(!handle_event(
        right_click,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path)
    )
    .unwrap());
    assert_eq!(app.tab_menu.as_ref().unwrap().tab_id, target);

    let rename_area = ratatui::layout::Rect::new(2, 2, 10, 1);
    app.tab_menu_hits = vec![crate::app::TabMenuHit {
        area: rename_area,
        action: crate::app::TabMenuAction::Rename,
    }];
    let left_click = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rename_area.x,
        row: rename_area.y,
        modifiers: KeyModifiers::NONE,
    });

    assert!(!handle_event(
        left_click,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path)
    )
    .unwrap());
    assert!(matches!(
        app.tab_dialog.as_ref().map(|dialog| dialog.mode),
        Some(crate::app::TabDialogMode::Rename(id)) if id == target
    ));
}

#[test]
fn tab_menu_keys_select_and_activate_actions() {
    assert_eq!(map_tab_menu_key(key(KeyCode::Up)), Some(Input::Up));
    assert_eq!(map_tab_menu_key(key(KeyCode::Down)), Some(Input::Down));
    assert_eq!(map_tab_menu_key(key(KeyCode::Enter)), Some(Input::Enter));
    assert_eq!(map_tab_menu_key(key(KeyCode::Esc)), Some(Input::Esc));
}

#[test]
fn tab_shortcuts_map_in_queue_pane() {
    assert_eq!(
        map_key(key(KeyCode::Char('[')), Pane::Queue),
        Some(Input::PreviousTab)
    );
    assert_eq!(
        map_key(key(KeyCode::Char(']')), Pane::Queue),
        Some(Input::NextTab)
    );
    assert_eq!(
        map_key(key(KeyCode::Char('r')), Pane::Queue),
        Some(Input::OpenRenameTab)
    );
    assert_eq!(
        map_key(
            with_mods(KeyCode::Char('t'), KeyModifiers::CONTROL),
            Pane::Composer
        ),
        Some(Input::OpenCreateTab)
    );
    assert_eq!(
        map_key(key(KeyCode::Char('[')), Pane::Composer),
        Some(Input::Char('['))
    );
}

#[test]
fn p_and_e_map_to_queue_actions() {
    assert_eq!(
        map_key(key(KeyCode::Char('p')), Pane::Queue),
        Some(Input::Char('p'))
    );
    assert_eq!(
        map_key(key(KeyCode::Char('e')), Pane::Queue),
        Some(Input::Char('e'))
    );
    assert_eq!(
        map_key(key(KeyCode::Char('f')), Pane::Queue),
        Some(Input::Char('f'))
    );
    assert_eq!(
        map_key(key(KeyCode::Char('f')), Pane::Composer),
        Some(Input::Char('f'))
    );
}

#[test]
fn preview_keys_map_to_scrolling_and_closing() {
    assert_eq!(map_preview_key(key(KeyCode::Up)), Some(Input::Up));
    assert_eq!(map_preview_key(key(KeyCode::Down)), Some(Input::Down));
    assert_eq!(map_preview_key(key(KeyCode::PageUp)), Some(Input::PageUp));
    assert_eq!(
        map_preview_key(key(KeyCode::PageDown)),
        Some(Input::PageDown)
    );
    assert_eq!(
        map_preview_key(with_mods(KeyCode::Char('G'), KeyModifiers::SHIFT)),
        Some(Input::Char('G'))
    );
    assert_eq!(
        map_preview_key(key(KeyCode::Char('f'))),
        Some(Input::Char('f'))
    );
    assert_eq!(map_preview_key(key(KeyCode::Esc)), Some(Input::Esc));
    assert_eq!(
        map_preview_key(with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Some(Input::Quit)
    );
}

#[test]
fn wheel_scrolls_the_preview() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, q_core::Prompt::new("prompt").unwrap())
        .unwrap();
    let mut app = App::new(workspace);
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();
    app.preview = Some(crate::app::PromptPreview {
        source: crate::app::PreviewSource::Prompt(app.visible_prompts()[0].id),
        scroll: 0,
    });
    app.preview_max_scroll = 5;

    let wheel = |kind| {
        Event::Mouse(crossterm::event::MouseEvent {
            kind,
            column: 4,
            row: 4,
            modifiers: KeyModifiers::NONE,
        })
    };

    handle_event(
        wheel(MouseEventKind::ScrollDown),
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();
    assert_eq!(app.preview.as_ref().unwrap().scroll, 1);

    handle_event(
        wheel(MouseEventKind::ScrollUp),
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();
    assert_eq!(app.preview.as_ref().unwrap().scroll, 0);
}

#[test]
fn clicks_are_ignored_while_the_preview_is_open() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, q_core::Prompt::new("prompt").unwrap())
        .unwrap();
    let mut app = App::new(workspace);
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();
    let composer_area = ratatui::layout::Rect::new(0, 10, 20, 1);
    app.composer_area = Some(composer_area);
    app.preview = Some(crate::app::PromptPreview {
        source: crate::app::PreviewSource::Prompt(app.visible_prompts()[0].id),
        scroll: 0,
    });

    let click = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: composer_area.x,
        row: composer_area.y,
        modifiers: KeyModifiers::NONE,
    });
    handle_event(
        click,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();

    assert_eq!(app.focus, Pane::Queue);
    assert!(app.preview.is_some());
}

#[test]
fn cmd_slash_opens_history_from_both_panes() {
    for pane in [Pane::Queue, Pane::Composer] {
        assert_eq!(
            map_key(with_mods(KeyCode::Char('/'), KeyModifiers::SUPER), pane),
            Some(Input::OpenSearch)
        );
    }
    assert_eq!(
        map_key(key(KeyCode::Char('/')), Pane::Queue),
        Some(Input::OpenSearch)
    );
    assert_eq!(
        map_key(key(KeyCode::Char('/')), Pane::Composer),
        Some(Input::Char('/'))
    );
}

#[test]
fn arrow_keys_switch_tabs_in_queue_pane() {
    assert_eq!(
        map_key(key(KeyCode::Left), Pane::Queue),
        Some(Input::PreviousTab)
    );
    assert_eq!(
        map_key(key(KeyCode::Right), Pane::Queue),
        Some(Input::NextTab)
    );
    assert_eq!(
        map_key(key(KeyCode::Left), Pane::Composer),
        Some(Input::MoveLeft)
    );
}

#[test]
fn search_keys_filter_navigate_and_close() {
    assert_eq!(
        map_search_key(key(KeyCode::Char('a'))),
        Some(Input::Char('a'))
    );
    assert_eq!(
        map_search_key(key(KeyCode::Backspace)),
        Some(Input::Backspace)
    );
    assert_eq!(map_search_key(key(KeyCode::Up)), Some(Input::Up));
    assert_eq!(map_search_key(key(KeyCode::Down)), Some(Input::Down));
    assert_eq!(map_search_key(key(KeyCode::Enter)), Some(Input::Enter));
    assert_eq!(map_search_key(key(KeyCode::Esc)), Some(Input::Esc));
    assert_eq!(
        map_search_key(with_mods(KeyCode::Char('/'), KeyModifiers::SUPER)),
        Some(Input::OpenSearch)
    );
}

#[test]
fn clicking_a_history_result_opens_it_fullscreen() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, q_core::Prompt::new("clicked prompt").unwrap())
        .unwrap();
    let mut app = App::new(workspace);
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();
    app.search = Some(crate::app::SearchDialog::default());
    let hit = ratatui::layout::Rect::new(1, 4, 30, 1);
    app.search_hits = vec![crate::app::SearchHit {
        area: hit,
        index: 0,
    }];

    let click = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hit.x,
        row: hit.y,
        modifiers: KeyModifiers::NONE,
    });
    handle_event(
        click,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();

    assert_eq!(
        app.preview.as_ref().map(|preview| preview.source.clone()),
        Some(crate::app::PreviewSource::History(
            q_core::PromptSource::inline("clicked prompt").unwrap()
        ))
    );

    let enter = Event::Key(key(KeyCode::Enter));
    handle_event(
        enter,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap();

    assert_eq!(clipboard.last.as_deref(), Some("clicked prompt"));
    assert!(app.preview.is_none());
    assert!(app.search.is_none());
}

#[test]
fn j_k_navigate_in_queue_pane_only() {
    assert_eq!(
        map_key(key(KeyCode::Char('j')), Pane::Queue),
        Some(Input::Down)
    );
    assert_eq!(
        map_key(key(KeyCode::Char('k')), Pane::Queue),
        Some(Input::Up)
    );
    assert_eq!(
        map_key(key(KeyCode::Char('j')), Pane::Composer),
        Some(Input::Char('j'))
    );
}

#[test]
fn printable_chars_require_no_command_modifiers() {
    assert_eq!(
        map_key(key(KeyCode::Char('x')), Pane::Composer),
        Some(Input::Char('x'))
    );
    assert_eq!(
        map_key(
            with_mods(KeyCode::Char('x'), KeyModifiers::ALT),
            Pane::Composer
        ),
        None
    );
}

#[test]
fn shifted_letters_are_uppercase_in_composer() {
    assert_eq!(
        map_key(
            with_mods(KeyCode::Char('a'), KeyModifiers::SHIFT),
            Pane::Composer
        ),
        Some(Input::Char('A'))
    );
    assert_eq!(
        map_key(
            with_mods(KeyCode::Char('ф'), KeyModifiers::SHIFT),
            Pane::Composer
        ),
        Some(Input::Char('Ф'))
    );
}

#[test]
fn keyboard_mode_reports_modifiers_and_layout_aware_shifted_keys() {
    assert!(
        KEYBOARD_ENHANCEMENTS.contains(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES)
    );
    assert!(KEYBOARD_ENHANCEMENTS.contains(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS));
}

#[test]
fn blink_timeout_targets_state_transitions() {
    assert_eq!(
        blink_timeout(Duration::from_millis(0)),
        Duration::from_millis(650)
    );
    assert_eq!(
        blink_timeout(Duration::from_millis(700)),
        Duration::from_millis(300)
    );
    assert_eq!(
        blink_timeout(Duration::from_millis(1200)),
        Duration::from_millis(450)
    );
}

#[test]
fn concurrent_add_transactions_preserve_both_prompts() {
    use std::sync::{Arc, Barrier};

    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();

    for text in ["from-a", "from-b"] {
        let path = queue_path.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            let prompt = q_core::Prompt::new(text).unwrap();
            let tab_id = q_core::Workspace::new().first_tab_id();
            barrier.wait();
            commit_mutation(&path, &QueueMutation::Add { tab_id, prompt }).unwrap();
        }));
    }

    barrier.wait();
    for handle in handles {
        handle.join().unwrap();
    }

    let workspace = q_core::storage::load_dir(&queue_path).unwrap();
    let texts: Vec<_> = workspace.tabs()[0]
        .queue()
        .iter()
        .map(|prompt| prompt.inline_text().unwrap_or(""))
        .collect();
    assert_eq!(texts.len(), 2);
    assert!(texts.contains(&"from-a"));
    assert!(texts.contains(&"from-b"));
}

#[test]
fn close_tab_mutation_is_persisted() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let closed = workspace.create_tab("closed").unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();

    let outcome = commit_mutation(&queue_path, &QueueMutation::CloseTab(closed)).unwrap();

    assert!(matches!(outcome, MutationOutcome::Committed(_)));
    let persisted = q_core::storage::load_dir(&queue_path).unwrap();
    assert!(persisted.tab(closed).is_none());
    assert_eq!(persisted.tabs().len(), 1);
}

#[test]
fn stale_mutation_returns_latest_queue_as_conflict() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let prompt = q_core::Prompt::new("remove me").unwrap();
    let id = prompt.id;
    let expected_source = prompt.source().clone();

    let tab_id = q_core::Workspace::new().first_tab_id();
    commit_mutation(&queue_path, &QueueMutation::Add { tab_id, prompt }).unwrap();
    let removal = QueueMutation::Remove {
        id,
        expected_source,
        expected_pinned: false,
        expected_external_content: None,
    };
    commit_mutation(&queue_path, &removal).unwrap();
    let outcome = commit_mutation(&queue_path, &removal).unwrap();

    assert!(matches!(outcome, MutationOutcome::Rejected(_, _)));
    assert!(q_core::storage::load_dir(&queue_path)
        .unwrap()
        .get_prompt(id)
        .is_none());
}

#[test]
fn stale_inline_edit_and_remove_reject_the_latest_source() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::new("before").unwrap();
    let id = prompt.id;
    let stale_source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();

    workspace.edit_prompt_inline(id, "newer").unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();

    let edit = commit_mutation(
        &queue_path,
        &QueueMutation::EditInline {
            id,
            expected_source: stale_source.clone(),
            expected_pinned: false,
            text: "stale edit".to_string(),
        },
    )
    .unwrap();
    let remove = commit_mutation(
        &queue_path,
        &QueueMutation::Remove {
            id,
            expected_source: stale_source,
            expected_pinned: false,
            expected_external_content: None,
        },
    )
    .unwrap();

    assert!(matches!(edit, MutationOutcome::Rejected(_, _)));
    assert!(matches!(remove, MutationOutcome::Rejected(_, _)));
    assert_eq!(
        q_core::storage::load_dir(&queue_path)
            .unwrap()
            .get_prompt(id)
            .unwrap()
            .inline_text(),
        Some("newer")
    );
}

#[test]
fn stale_pop_rejects_a_newly_pinned_prompt() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::new("body").unwrap();
    let id = prompt.id;
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();
    workspace.set_prompt_pinned(id, true).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();

    let outcome = commit_mutation(
        &queue_path,
        &QueueMutation::Remove {
            id,
            expected_source: source,
            expected_pinned: false,
            expected_external_content: None,
        },
    )
    .unwrap();

    assert!(matches!(outcome, MutationOutcome::Rejected(_, _)));
    assert!(q_core::storage::load_dir(&queue_path)
        .unwrap()
        .get_prompt(id)
        .unwrap()
        .pinned());
}

#[test]
fn stale_external_pop_rejects_content_changed_after_copy() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let source_path = dir.path().join("live.md");
    std::fs::write(&source_path, "copied version").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::from_external_markdown(source_path.clone()).unwrap();
    let id = prompt.id;
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();
    std::fs::write(&source_path, "new version").unwrap();

    let outcome = commit_mutation(
        &queue_path,
        &QueueMutation::Remove {
            id,
            expected_source: source,
            expected_pinned: false,
            expected_external_content: Some("copied version".to_string()),
        },
    )
    .unwrap();

    assert!(matches!(outcome, MutationOutcome::Rejected(_, _)));
    assert!(q_core::storage::load_dir(&queue_path)
        .unwrap()
        .get_prompt(id)
        .is_some());
}

#[test]
fn external_refresh_preserves_composer_state() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut app = App::new(q_core::Queue::new());
    app.composer.set_text("local draft");
    app.focus = Pane::Composer;
    let mut sync = QueueSync::new(&queue_path);

    let mut external = q_core::Workspace::new();
    let tab_id = external.first_tab_id();
    external
        .add_prompt(tab_id, q_core::Prompt::new("external prompt").unwrap())
        .unwrap();
    q_core::storage::save_dir(&queue_path, &external).unwrap();
    sync.last_check = Instant::now() - SYNC_INTERVAL;
    sync.refresh_if_due(&mut app, &queue_path);

    assert_eq!(app.visible_prompts().len(), 1);
    assert_eq!(app.composer.text(), "local draft");
    assert_eq!(app.focus, Pane::Composer);
}

#[test]
fn inline_edit_commit_preserves_record_metadata_and_adds_typed_history() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let mut prompt = q_core::Prompt::new("before").unwrap();
    prompt.set_pinned(true);
    let id = prompt.id;
    let created_at = prompt.created_at;
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();

    let mutation = QueueMutation::EditInline {
        id,
        expected_source: q_core::PromptSource::Inline {
            text: "before".to_string(),
        },
        expected_pinned: true,
        text: "after".to_string(),
    };
    let outcome = commit_mutation(&queue_path, &mutation).unwrap();
    assert!(matches!(outcome, MutationOutcome::Committed(_)));
    let saved = q_core::storage::load_dir(&queue_path).unwrap();
    let edited = saved.get_prompt(id).unwrap();
    assert_eq!(edited.inline_text(), Some("after"));
    assert!(edited.pinned());
    assert_eq!(edited.created_at, created_at);
    assert!(saved
        .history()
        .iter()
        .any(|entry| entry.inline_text() == Some("before")));
    assert!(saved
        .history()
        .iter()
        .any(|entry| entry.inline_text() == Some("after")));
}

#[test]
fn successful_inline_editor_save_returns_to_queue() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::new("before").unwrap();
    let id = prompt.id;
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();
    let mut app = App::new(workspace);
    app.open_editor_for_source(source.clone(), Some(id))
        .unwrap();

    assert!(persist_mutation(
        &mut app,
        &queue_path,
        &QueueMutation::EditInline {
            id,
            expected_source: source,
            expected_pinned: false,
            text: "after".to_string(),
        },
    )
    .unwrap());

    assert!(app.editor.is_none());
    assert_eq!(
        app.workspace.get_prompt(id).unwrap().inline_text(),
        Some("after")
    );
}

#[test]
fn inline_editor_save_error_keeps_the_unsaved_buffer_open() {
    let dir = tempfile::TempDir::new().unwrap();
    let blocked_parent = dir.path().join("not-a-directory");
    std::fs::write(&blocked_parent, "block parent creation").unwrap();
    let queue_path = blocked_parent.join("queue.json");
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::new("before").unwrap();
    let id = prompt.id;
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    let mut app = App::new(workspace);
    app.open_editor_for_source(source, Some(id)).unwrap();
    app.editor.as_mut().unwrap().buffer.insert_char('!');
    let mut clipboard = q_platform::clipboard::FakeClipboard::new();

    assert!(!handle_event(
        Event::Key(with_mods(KeyCode::Char('s'), KeyModifiers::CONTROL)),
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path),
    )
    .unwrap());

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.buffer.text(), "before!");
    assert!(editor.error.contains("save failed"));
}

#[test]
fn serialized_external_editor_saves_reject_the_stale_writer() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let source_path = dir.path().join("document.md");
    std::fs::write(&source_path, "original").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::from_external_markdown(source_path.clone()).unwrap();
    let source = prompt.source().clone();
    let id = prompt.id;
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();
    let mut first = App::new(workspace.clone());
    let mut second = App::new(workspace);
    first
        .open_editor_for_source(source.clone(), Some(id))
        .unwrap();
    second.open_editor_for_source(source, Some(id)).unwrap();
    first.editor.as_mut().unwrap().buffer.insert_char('A');
    second.editor.as_mut().unwrap().buffer.insert_char('B');

    assert!(save_external_editor(&mut first, &queue_path));
    assert!(!save_external_editor(&mut second, &queue_path));

    assert_eq!(std::fs::read_to_string(source_path).unwrap(), "originalA");
    assert!(first.editor.is_none());
    let editor = second.editor.as_ref().unwrap();
    assert_eq!(editor.buffer.text(), "originalB");
    assert!(editor.error.contains("changed on disk"));
}

#[test]
fn remove_commit_preserves_typed_history_and_external_source_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let source_path = dir.path().join("source.md");
    std::fs::write(&source_path, "body").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::from_external_markdown(source_path.clone()).unwrap();
    let id = prompt.id;
    let source = prompt.source().clone();
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();

    commit_mutation(
        &queue_path,
        &QueueMutation::Remove {
            id,
            expected_source: source.clone(),
            expected_pinned: false,
            expected_external_content: None,
        },
    )
    .unwrap();
    let saved = q_core::storage::load_dir(&queue_path).unwrap();
    assert!(saved.get_prompt(id).is_none());
    assert!(saved
        .history()
        .iter()
        .any(|entry| entry.source() == &source));
    assert_eq!(std::fs::read_to_string(source_path).unwrap(), "body");

    commit_mutation(&queue_path, &QueueMutation::ForgetHistory(source.clone())).unwrap();
    let forgotten = q_core::storage::load_dir(&queue_path).unwrap();
    assert!(!forgotten
        .history()
        .iter()
        .any(|entry| entry.source() == &source));
}

#[test]
fn clipboard_failure_does_not_remove_unpinned_prompt() {
    struct FailingClipboard;
    impl q_platform::clipboard::Clipboard for FailingClipboard {
        fn set_text(&mut self, _text: &str) -> Result<(), q_platform::clipboard::ClipboardError> {
            Err(q_platform::clipboard::ClipboardError::Unavailable(
                "test failure".to_string(),
            ))
        }
    }

    let dir = tempfile::TempDir::new().unwrap();
    let queue_path = q_core::storage::init_dir(dir.path(), "test").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    let prompt = q_core::Prompt::new("keep me").unwrap();
    let id = prompt.id;
    workspace.add_prompt(tab, prompt).unwrap();
    q_core::storage::save_dir(&queue_path, &workspace).unwrap();
    let mut app = App::new(workspace);
    let mut clipboard = FailingClipboard;

    let event = Event::Key(key(KeyCode::Enter));
    assert!(!handle_event(
        event,
        &mut app,
        &mut clipboard,
        &mut queue_path.clone(),
        &mut QueueSync::new(&queue_path)
    )
    .unwrap());
    assert!(app.workspace.get_prompt(id).is_some());
    assert!(q_core::storage::load_dir(&queue_path)
        .unwrap()
        .get_prompt(id)
        .is_some());
    assert!(app.status.contains("clipboard failed"));
}

// --- Workspace menu effects ---

fn menu_fixture() -> (tempfile::TempDir, App, std::path::PathBuf, QueueSync) {
    let dir = tempfile::TempDir::new().unwrap();
    let personal = q_core::storage::init_dir(dir.path(), "personal").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, q_core::Prompt::new("personal prompt").unwrap())
        .unwrap();
    q_core::storage::save_dir(&personal, &workspace).unwrap();
    let sync = QueueSync::new(&personal);
    (dir, App::new(workspace), personal, sync)
}

#[test]
fn open_workspaces_effect_lists_entries_with_the_current_marker() {
    let (dir, mut app, mut current, mut sync) = menu_fixture();
    q_core::storage::init_dir(dir.path(), "team").unwrap();

    handle_menu_effect(
        &mut app,
        &Effect::OpenWorkspacesOverlay,
        &mut current,
        &mut sync,
    )
    .unwrap();

    let Some(MenuState::Workspaces(overlay)) = &app.menu else {
        panic!("workspaces overlay expected");
    };
    let entries: Vec<_> = overlay
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.current))
        .collect();
    assert_eq!(entries, vec![("personal", true), ("team", false)]);
    assert_eq!(overlay.selected, 0);
}

#[test]
fn create_workspace_effect_creates_and_switches_this_window() {
    let (_dir, mut app, mut current, mut sync) = menu_fixture();

    handle_menu_effect(
        &mut app,
        &Effect::CreateWorkspace("team".to_string()),
        &mut current,
        &mut sync,
    )
    .unwrap();

    assert!(current.join("workspace.json").exists());
    assert_eq!(q_core::storage::read_meta(&current).unwrap().name, "team");
    assert!(app.menu.is_none());
    assert!(app.visible_prompts().is_empty());
}

#[test]
fn create_workspace_effect_rejects_duplicate_names() {
    let (_dir, mut app, mut current, mut sync) = menu_fixture();
    let before = current.clone();
    app.open_workspaces(Vec::new());

    handle_menu_effect(
        &mut app,
        &Effect::CreateWorkspace("PERSONAL".to_string()),
        &mut current,
        &mut sync,
    )
    .unwrap();

    assert_eq!(current, before);
    let Some(MenuState::Workspaces(overlay)) = &app.menu else {
        panic!("workspaces overlay expected");
    };
    assert!(overlay.error.contains("already exists"));
}

#[test]
fn switch_workspace_effect_loads_the_other_queue() {
    let (dir, mut app, mut current, mut sync) = menu_fixture();
    let team = q_core::storage::init_dir(dir.path(), "team").unwrap();
    let mut workspace = q_core::Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, q_core::Prompt::new("team prompt").unwrap())
        .unwrap();
    q_core::storage::save_dir(&team, &workspace).unwrap();

    handle_menu_effect(
        &mut app,
        &Effect::SwitchWorkspace(team.clone()),
        &mut current,
        &mut sync,
    )
    .unwrap();

    assert_eq!(current, team);
    assert_eq!(app.visible_prompts()[0].inline_text(), Some("team prompt"));
}

#[test]
fn delete_workspace_effect_removes_dir_and_falls_back_when_current() {
    let (dir, mut app, mut current, mut sync) = menu_fixture();
    q_core::storage::init_dir(dir.path(), "team").unwrap();
    let deleted = current.clone();

    handle_menu_effect(
        &mut app,
        &Effect::DeleteWorkspace(current.clone()),
        &mut current,
        &mut sync,
    )
    .unwrap();

    assert!(!deleted.exists());
    assert_eq!(q_core::storage::read_meta(&current).unwrap().name, "team");
    let Some(MenuState::Workspaces(overlay)) = &app.menu else {
        panic!("workspaces overlay expected");
    };
    assert_eq!(overlay.entries.len(), 1);
}

#[test]
fn delete_workspace_effect_refuses_the_last_workspace() {
    let (_dir, mut app, mut current, mut sync) = menu_fixture();
    app.open_workspaces(Vec::new());

    handle_menu_effect(
        &mut app,
        &Effect::DeleteWorkspace(current.clone()),
        &mut current,
        &mut sync,
    )
    .unwrap();

    assert!(current.exists());
    let Some(MenuState::Workspaces(overlay)) = &app.menu else {
        panic!("workspaces overlay expected");
    };
    assert_eq!(overlay.error, "cannot delete the last workspace");
}
