use super::*;
use q_core::Workspace;

fn app_with(n: usize) -> App {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    for index in 0..n {
        workspace
            .add_prompt(tab, Prompt::new(format!("prompt-{index}")).unwrap())
            .unwrap();
    }
    App::new(workspace)
}

#[test]
fn mouse_inputs_focus_prompt_and_composer() {
    let mut app = app_with(2);
    app.focus = Pane::Composer;

    reduce(&mut app, Input::SelectPrompt(1));
    assert_eq!(app.focus, Pane::Queue);
    assert_eq!(app.selected, Some(1));

    reduce(&mut app, Input::FocusComposer);
    assert_eq!(app.focus, Pane::Composer);
    assert_eq!(app.selected, Some(1));
}

#[test]
fn tab_cycles_focus() {
    let mut app = app_with(1);
    reduce(&mut app, Input::Tab);
    assert_eq!(app.focus, Pane::Composer);
    reduce(&mut app, Input::Tab);
    assert_eq!(app.focus, Pane::Queue);
}

#[test]
fn create_dialog_creates_and_selects_empty_tab() {
    let mut app = app_with(1);
    reduce(&mut app, Input::OpenCreateTab);
    for c in "work".chars() {
        reduce(&mut app, Input::Char(c));
    }
    let effect = reduce(&mut app, Input::Enter);
    assert!(matches!(
        effect,
        Some(Effect::Persist(QueueMutation::CreateTab { name, .. })) if name == "work"
    ));
    assert_eq!(app.workspace.tab(app.active_tab_id).unwrap().name(), "work");
    assert_eq!(app.focus, Pane::Composer);
    assert!(app.visible_prompts().is_empty());
}

#[test]
fn duplicate_tab_name_keeps_dialog_open_with_error() {
    let mut app = app_with(0);
    app.workspace.create_tab("work").unwrap();
    reduce(&mut app, Input::OpenCreateTab);
    for c in "WORK".chars() {
        reduce(&mut app, Input::Char(c));
    }
    assert_eq!(reduce(&mut app, Input::Enter), None);
    assert!(app
        .tab_dialog
        .as_ref()
        .unwrap()
        .error
        .contains("already exists"));
}

#[test]
fn next_and_previous_select_adjacent_tabs() {
    let mut app = app_with(0);
    let second = app.workspace.create_tab("second").unwrap();
    let first = app.workspace.resolve_tab("1").unwrap();
    app.select_tab(second);
    reduce(&mut app, Input::NextTab);
    assert_eq!(app.active_tab_id, first);
    reduce(&mut app, Input::PreviousTab);
    assert_eq!(app.active_tab_id, second);
}

#[test]
fn enter_on_unpinned_copies_and_pops() {
    let mut app = app_with(2);
    let prompt = app.selected_prompt().unwrap().clone();
    let effect = reduce(&mut app, Input::Enter);
    assert_eq!(
        effect,
        Some(Effect::CopyAndPersist {
            text: prompt.text,
            mutation: QueueMutation::Remove(prompt.id),
        })
    );
    assert_eq!(app.visible_prompts().len(), 1);
}

#[test]
fn enter_on_pinned_copies_but_does_not_pop() {
    let mut app = app_with(1);
    let id = app.visible_prompts()[0].id;
    app.workspace.set_prompt_pinned(id, true).unwrap();
    let text = app.selected_prompt().unwrap().text.clone();
    assert_eq!(
        reduce(&mut app, Input::Enter),
        Some(Effect::CopyToClipboard(text))
    );
    assert_eq!(app.visible_prompts().len(), 1);
}

#[test]
fn p_pins_selected_prompt() {
    let mut app = app_with(1);
    let id = app.selected_prompt().unwrap().id;
    assert_eq!(
        reduce(&mut app, Input::Char('p')),
        Some(Effect::Persist(QueueMutation::SetPinned {
            id,
            pinned: true
        }))
    );
    assert!(app.selected_prompt().unwrap().pinned);
}

#[test]
fn e_moves_selected_prompt_into_composer() {
    let mut app = app_with(1);
    let text = app.selected_prompt().unwrap().text.clone();
    assert!(matches!(
        reduce(&mut app, Input::Char('e')),
        Some(Effect::Persist(QueueMutation::Remove(_)))
    ));
    assert_eq!(app.composer.text(), text);
    assert_eq!(app.focus, Pane::Composer);
}

#[test]
fn f_opens_preview_for_selected_prompt_and_closes_again() {
    let mut app = app_with(1);
    let id = app.selected_prompt().unwrap().id;

    assert_eq!(reduce(&mut app, Input::Char('f')), None);
    assert_eq!(
        app.preview.as_ref().map(|preview| preview.source.clone()),
        Some(PreviewSource::Prompt(id))
    );

    assert_eq!(reduce(&mut app, Input::Char('f')), None);
    assert!(app.preview.is_none());

    reduce(&mut app, Input::Char('f'));
    reduce(&mut app, Input::Esc);
    assert!(app.preview.is_none());
}

#[test]
fn preview_scrolling_is_clamped_to_rendered_metrics() {
    let mut app = app_with(1);
    reduce(&mut app, Input::Char('f'));
    app.preview_page = 4;
    app.preview_max_scroll = 10;

    reduce(&mut app, Input::Up);
    assert_eq!(app.preview.as_ref().unwrap().scroll, 0);

    reduce(&mut app, Input::PageDown);
    assert_eq!(app.preview.as_ref().unwrap().scroll, 4);

    reduce(&mut app, Input::Char('G'));
    assert_eq!(app.preview.as_ref().unwrap().scroll, 10);

    reduce(&mut app, Input::Char('j'));
    assert_eq!(app.preview.as_ref().unwrap().scroll, 10);

    reduce(&mut app, Input::Char('g'));
    assert_eq!(app.preview.as_ref().unwrap().scroll, 0);
}

#[test]
fn search_filters_history_and_previews_the_selected_entry() {
    let mut app = app_with(0);
    let tab = app.active_tab_id;
    for text in ["write the docs", "deploy the api"] {
        app.workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }

    reduce(&mut app, Input::OpenSearch);
    for c in "docs".chars() {
        reduce(&mut app, Input::Char(c));
    }

    assert_eq!(app.search_results().len(), 1);
    assert_eq!(reduce(&mut app, Input::Enter), None);
    assert_eq!(
        app.preview.as_ref().map(|preview| preview.source.clone()),
        Some(PreviewSource::History("write the docs".to_string()))
    );
}

#[test]
fn search_matches_across_scripts_and_accents() {
    let mut app = app_with(0);
    let tab = app.active_tab_id;
    for text in ["улучшить конфиги", "café Müller"] {
        app.workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }

    reduce(&mut app, Input::OpenSearch);
    for c in "uluchshit".chars() {
        reduce(&mut app, Input::Char(c));
    }
    assert_eq!(app.search_results().len(), 1);
    assert_eq!(app.search_results()[0].text, "улучшить конфиги");

    for _ in 0.."uluchshit".len() {
        reduce(&mut app, Input::Backspace);
    }
    for c in "muller".chars() {
        reduce(&mut app, Input::Char(c));
    }
    assert_eq!(app.search_results().len(), 1);
    assert_eq!(app.search_results()[0].text, "café Müller");
}

#[test]
fn search_finds_prompts_that_were_already_copied_away() {
    let mut app = app_with(1);
    let text = app.selected_prompt().unwrap().text.clone();
    reduce(&mut app, Input::Enter);
    assert!(app.visible_prompts().is_empty());

    reduce(&mut app, Input::OpenSearch);

    assert_eq!(app.search_results().len(), 1);
    assert_eq!(app.search_results()[0].text, text);
}

#[test]
fn copying_from_search_preview_returns_to_the_main_page() {
    let mut app = app_with(1);
    let text = app.selected_prompt().unwrap().text.clone();
    reduce(&mut app, Input::OpenSearch);
    reduce(&mut app, Input::SelectHistory(0));
    assert!(app.preview.is_some());

    assert_eq!(
        reduce(&mut app, Input::Enter),
        Some(Effect::CopyToClipboard(text))
    );
    assert!(app.preview.is_none());
    assert!(app.search.is_none());
}

#[test]
fn escaping_the_preview_returns_to_the_search_results() {
    let mut app = app_with(1);
    reduce(&mut app, Input::OpenSearch);
    reduce(&mut app, Input::SelectHistory(0));

    reduce(&mut app, Input::Esc);

    assert!(app.preview.is_none());
    assert!(app.search.is_some());

    reduce(&mut app, Input::Esc);
    assert!(app.search.is_none());
}

#[test]
fn search_navigation_is_clamped_to_the_result_count() {
    let mut app = app_with(2);
    reduce(&mut app, Input::OpenSearch);

    reduce(&mut app, Input::Up);
    assert_eq!(app.search.as_ref().unwrap().selected, 0);

    reduce(&mut app, Input::Down);
    reduce(&mut app, Input::Down);
    assert_eq!(app.search.as_ref().unwrap().selected, 1);
}

#[test]
fn typing_in_search_resets_the_selection() {
    let mut app = app_with(2);
    reduce(&mut app, Input::OpenSearch);
    reduce(&mut app, Input::Down);
    reduce(&mut app, Input::Char('p'));
    assert_eq!(app.search.as_ref().unwrap().selected, 0);
    reduce(&mut app, Input::Backspace);
    assert_eq!(app.search.as_ref().unwrap().query, "");
}

#[test]
fn preview_enter_copies_prompt_without_removing_it() {
    let mut app = app_with(1);
    let text = app.selected_prompt().unwrap().text.clone();
    reduce(&mut app, Input::Char('f'));

    assert_eq!(
        reduce(&mut app, Input::Enter),
        Some(Effect::CopyToClipboard(text))
    );
    assert!(app.preview.is_none());
    assert_eq!(app.visible_prompts().len(), 1);
}

#[test]
fn forget_history_drops_the_selected_entry_and_persists_it() {
    let mut app = app_with(0);
    let tab = app.active_tab_id;
    for text in ["keep me", "secret token"] {
        app.workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }
    reduce(&mut app, Input::OpenSearch);
    // Newest first, so the secret is selected.
    assert_eq!(app.search_results()[0].text, "secret token");

    let effect = reduce(&mut app, Input::ForgetHistory);

    assert_eq!(
        effect,
        Some(Effect::Persist(QueueMutation::ForgetHistory(
            "secret token".to_string()
        )))
    );
    let texts: Vec<_> = app
        .search_results()
        .iter()
        .map(|entry| entry.text.clone())
        .collect();
    assert_eq!(texts, vec!["keep me".to_string()]);
    assert_eq!(app.search.as_ref().unwrap().selected, 0);
}

#[test]
fn forget_history_on_an_empty_result_set_does_nothing() {
    let mut app = app_with(0);
    reduce(&mut app, Input::OpenSearch);

    assert_eq!(reduce(&mut app, Input::ForgetHistory), None);
    assert!(app.search.is_some());
}

#[test]
fn fold_cache_stays_in_step_with_history() {
    let mut app = app_with(0);
    let tab = app.active_tab_id;
    for text in ["улучшить конфиги", "second prompt"] {
        app.workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }
    reduce(&mut app, Input::OpenSearch);
    for c in "uluchshit".chars() {
        reduce(&mut app, Input::Char(c));
    }
    assert_eq!(app.search_folds.len(), 2);
    assert_eq!(app.search_results().len(), 1);

    // Forgetting shrinks history. The cache is dropped rather than left stale,
    // and results stay correct on the uncached fallback path.
    reduce(&mut app, Input::ForgetHistory);

    assert!(app.search_folds.is_empty());
    assert!(app
        .search_results()
        .iter()
        .all(|entry| entry.text != "улучшить конфиги"));

    // The cache still serves the remaining entry correctly.
    for _ in 0.."uluchshit".len() {
        reduce(&mut app, Input::Backspace);
    }
    for c in "second".chars() {
        reduce(&mut app, Input::Char(c));
    }
    assert_eq!(app.search_results().len(), 1);
    assert_eq!(app.search_results()[0].text, "second prompt");
}

#[test]
fn search_swallows_queue_shortcuts() {
    let mut app = app_with(1);
    reduce(&mut app, Input::OpenSearch);

    assert_eq!(reduce(&mut app, Input::Char('p')), None);

    assert!(!app.visible_prompts()[0].pinned);
    assert_eq!(app.search.as_ref().unwrap().query, "p");
}

#[test]
fn preview_swallows_queue_shortcuts() {
    let mut app = app_with(2);
    reduce(&mut app, Input::Char('f'));

    assert_eq!(reduce(&mut app, Input::Char('p')), None);
    assert_eq!(reduce(&mut app, Input::Char('e')), None);

    assert!(!app.visible_prompts()[0].pinned);
    assert_eq!(app.composer.text(), "");
    assert_eq!(app.focus, Pane::Queue);
}

#[test]
fn rename_dialog_renames_active_tab_without_reordering() {
    let mut app = app_with(0);
    let id = app.active_tab_id;
    app.focus = Pane::Queue;
    reduce(&mut app, Input::OpenRenameTab);
    reduce(&mut app, Input::Char('w'));
    for c in "ork".chars() {
        reduce(&mut app, Input::Char(c));
    }
    assert_eq!(
        reduce(&mut app, Input::Enter),
        Some(Effect::Persist(QueueMutation::RenameTab {
            id,
            name: "work".to_string(),
        }))
    );
    assert_eq!(app.workspace.tab(id).unwrap().name(), "work");
    assert_eq!(app.workspace.first_tab_id(), id);
}

#[test]
fn tab_menu_rename_opens_rename_dialog_for_target_tab() {
    let mut app = app_with(0);
    let target = app.workspace.create_tab("work").unwrap();
    reduce(
        &mut app,
        Input::OpenTabMenu {
            id: target,
            column: 4,
            row: 1,
        },
    );

    reduce(&mut app, Input::SelectTabMenuAction(TabMenuAction::Rename));

    let dialog = app.tab_dialog.as_ref().unwrap();
    assert_eq!(dialog.mode, TabDialogMode::Rename(target));
    assert_eq!(dialog.value, "work");
    assert!(app.tab_menu.is_none());
}

#[test]
fn closing_active_tab_requires_confirmation_and_selects_neighbor() {
    let mut app = app_with(0);
    let remaining = app.active_tab_id;
    let closed = app.workspace.create_tab("closed").unwrap();
    app.select_tab(closed);
    reduce(
        &mut app,
        Input::OpenTabMenu {
            id: closed,
            column: 4,
            row: 1,
        },
    );
    reduce(&mut app, Input::SelectTabMenuAction(TabMenuAction::Close));

    assert!(app.workspace.tab(closed).is_some());
    assert_eq!(app.close_tab_dialog.as_ref().unwrap().tab_id, closed);

    assert_eq!(
        reduce(&mut app, Input::Enter),
        Some(Effect::Persist(QueueMutation::CloseTab(closed)))
    );
    assert!(app.workspace.tab(closed).is_none());
    assert_eq!(app.active_tab_id, remaining);
}

#[test]
fn escape_cancels_tab_close() {
    let mut app = app_with(0);
    let closed = app.workspace.create_tab("closed").unwrap();
    app.close_tab_dialog = Some(CloseTabDialog {
        tab_id: closed,
        tab_name: "closed".to_string(),
    });

    assert_eq!(reduce(&mut app, Input::Esc), None);

    assert!(app.close_tab_dialog.is_none());
    assert!(app.workspace.tab(closed).is_some());
}

#[test]
fn tab_menu_does_not_offer_confirmation_for_last_tab() {
    let mut app = app_with(0);
    let only = app.active_tab_id;
    reduce(
        &mut app,
        Input::OpenTabMenu {
            id: only,
            column: 4,
            row: 1,
        },
    );

    reduce(&mut app, Input::SelectTabMenuAction(TabMenuAction::Close));

    assert!(app.close_tab_dialog.is_none());
    assert_eq!(app.status, "cannot close the last tab");
}

#[test]
fn composer_enter_saves_prompt_in_active_tab() {
    let mut app = app_with(0);
    app.focus = Pane::Composer;
    app.composer.set_text("hi");
    let tab_id = app.active_tab_id;
    let effect = reduce(&mut app, Input::Enter);
    assert!(matches!(
        effect,
        Some(Effect::Persist(QueueMutation::Add { tab_id: id, prompt }))
            if id == tab_id && prompt.text == "hi"
    ));
    assert_eq!(app.visible_prompts().len(), 1);
}

#[test]
fn composer_paste_is_inserted_as_one_multiline_edit() {
    let mut app = app_with(0);
    app.focus = Pane::Composer;
    reduce(&mut app, Input::Paste("first\rsecond".to_string()));
    assert_eq!(app.composer.text(), "first\nsecond");
    reduce(&mut app, Input::Undo);
    assert_eq!(app.composer.text(), "");
}
