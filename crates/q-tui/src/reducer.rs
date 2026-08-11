use crate::app::{
    App, CloseTabDialog, Effect, Input, Pane, PreviewSource, PromptPreview, QueueMutation,
    SearchDialog, TabContextMenu, TabDialog, TabDialogMode, TabMenuAction,
};
use chrono::Utc;
use q_core::{Prompt, TabId};
use ratatui_textarea::CursorMove;

pub fn reduce(app: &mut App, input: Input) -> Option<Effect> {
    if matches!(input, Input::Quit) {
        return Some(Effect::Quit);
    }
    if let Input::OpenTabMenu { id, column, row } = &input {
        if app.workspace.tab(*id).is_some() && !app.dialog_open() {
            app.tab_menu = Some(TabContextMenu {
                tab_id: *id,
                column: *column,
                row: *row,
                selected: TabMenuAction::Rename,
            });
        }
        return None;
    }
    if app.close_tab_dialog.is_some() {
        return reduce_close_tab_dialog(app, input);
    }
    if app.tab_dialog.is_some() {
        return reduce_dialog(app, input);
    }
    if app.tab_menu.is_some() {
        return reduce_tab_menu(app, input);
    }
    if app.preview.is_some() {
        return reduce_preview(app, input);
    }
    if app.search.is_some() {
        return reduce_search(app, input);
    }

    match input {
        Input::OpenSearch => {
            app.search = Some(SearchDialog::default());
            return None;
        }
        Input::OpenCreateTab => {
            app.tab_dialog = Some(TabDialog::create());
            return None;
        }
        Input::SelectTab(id) => {
            app.select_tab(id);
            return None;
        }
        Input::SelectPrompt(index) => {
            if index < app.visible_prompts().len() {
                app.selected = Some(index);
                app.focus = Pane::Queue;
            }
            return None;
        }
        Input::FocusComposer => {
            app.focus = Pane::Composer;
            return None;
        }
        Input::PreviousTab => {
            select_adjacent_tab(app, -1);
            return None;
        }
        Input::NextTab => {
            select_adjacent_tab(app, 1);
            return None;
        }
        Input::Tab => {
            app.focus = match app.focus {
                Pane::Queue => Pane::Composer,
                Pane::Composer => Pane::Queue,
            };
            return None;
        }
        _ => {}
    }

    match app.focus {
        Pane::Queue => reduce_queue(app, input),
        Pane::Composer => reduce_composer(app, input),
    }
}

fn reduce_preview(app: &mut App, input: Input) -> Option<Effect> {
    let page = app.preview_page;
    let max_scroll = app.preview_max_scroll;
    match input {
        Input::Esc | Input::Char('f') | Input::Char('q') => app.preview = None,
        Input::Enter => {
            let text = app.preview_text()?;
            app.preview = None;
            app.search = None;
            return Some(Effect::CopyToClipboard(text));
        }
        Input::Up | Input::Char('k') => scroll_preview(app, |scroll| scroll.saturating_sub(1)),
        Input::Down | Input::Char('j') => {
            scroll_preview(app, |scroll| scroll.saturating_add(1).min(max_scroll))
        }
        Input::PageUp => scroll_preview(app, |scroll| scroll.saturating_sub(page)),
        Input::PageDown => {
            scroll_preview(app, |scroll| scroll.saturating_add(page).min(max_scroll))
        }
        Input::Char('g') => scroll_preview(app, |_| 0),
        Input::Char('G') => scroll_preview(app, |_| max_scroll),
        _ => {}
    }
    None
}

fn scroll_preview(app: &mut App, f: impl FnOnce(u16) -> u16) {
    if let Some(preview) = app.preview.as_mut() {
        preview.scroll = f(preview.scroll);
    }
}

fn reduce_search(app: &mut App, input: Input) -> Option<Effect> {
    app.refresh_search_folds();
    let len = app.search_results().len();
    match input {
        Input::Esc | Input::OpenSearch => app.search = None,
        Input::Char(c) => {
            let search = app.search.as_mut()?;
            search.query.push(c);
            search.selected = 0;
        }
        Input::Backspace => {
            let search = app.search.as_mut()?;
            search.query.pop();
            search.selected = 0;
        }
        Input::Up => {
            let search = app.search.as_mut()?;
            search.selected = search.selected.saturating_sub(1);
        }
        Input::Down => {
            let search = app.search.as_mut()?;
            search.selected = (search.selected + 1).min(len.saturating_sub(1));
        }
        Input::Enter => open_history_preview(app, app.search.as_ref()?.selected),
        Input::SelectHistory(index) => {
            app.search.as_mut()?.selected = index;
            open_history_preview(app, index);
        }
        Input::ForgetHistory => return forget_selected_history(app),
        _ => {}
    }
    None
}

fn forget_selected_history(app: &mut App) -> Option<Effect> {
    let selected = app.search.as_ref()?.selected;
    let text = app.search_results().get(selected)?.text.clone();
    if !app.workspace.forget_history(&text) {
        return None;
    }
    app.search_folds.clear();
    let remaining = app.search_results().len();
    if let Some(search) = app.search.as_mut() {
        search.selected = selected.min(remaining.saturating_sub(1));
    }
    Some(Effect::Persist(QueueMutation::ForgetHistory(text)))
}

fn open_history_preview(app: &mut App, index: usize) {
    let Some(entry) = app
        .search_results()
        .get(index)
        .map(|entry| entry.text.clone())
    else {
        return;
    };
    app.preview = Some(PromptPreview {
        source: PreviewSource::History(entry),
        scroll: 0,
    });
}

fn reduce_tab_menu(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Up => app.tab_menu.as_mut()?.selected = TabMenuAction::Rename,
        Input::Down => app.tab_menu.as_mut()?.selected = TabMenuAction::Close,
        Input::Enter => {
            let action = app.tab_menu.as_ref()?.selected;
            return activate_tab_menu_action(app, action);
        }
        Input::SelectTabMenuAction(action) => return activate_tab_menu_action(app, action),
        Input::Esc | Input::DismissTabMenu => app.tab_menu = None,
        _ => {}
    }
    None
}

fn activate_tab_menu_action(app: &mut App, action: TabMenuAction) -> Option<Effect> {
    let menu = app.tab_menu.take()?;
    let tab = app.workspace.tab(menu.tab_id)?;
    match action {
        TabMenuAction::Rename => {
            app.tab_dialog = Some(TabDialog::rename(tab.id(), tab.name()));
        }
        TabMenuAction::Close => {
            if app.workspace.tabs().len() == 1 {
                app.status = "cannot close the last tab".to_string();
            } else {
                app.close_tab_dialog = Some(CloseTabDialog {
                    tab_id: tab.id(),
                    tab_name: tab.name().to_string(),
                });
            }
        }
    }
    None
}

fn reduce_close_tab_dialog(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Esc => {
            app.close_tab_dialog = None;
            None
        }
        Input::Enter => confirm_close_tab(app),
        _ => None,
    }
}

fn confirm_close_tab(app: &mut App) -> Option<Effect> {
    let dialog = app.close_tab_dialog.take()?;
    let replacement = if app.active_tab_id == dialog.tab_id {
        let tabs = app.workspace.tabs();
        let index = tabs.iter().position(|tab| tab.id() == dialog.tab_id)?;
        tabs.get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|previous| tabs.get(previous)))
            .map(|tab| tab.id())
    } else {
        Some(app.active_tab_id)
    };

    if let Err(error) = app.workspace.close_tab(dialog.tab_id) {
        app.status = error.to_string();
        return None;
    }
    if let Some(id) = replacement {
        app.select_tab(id);
    }
    Some(Effect::Persist(QueueMutation::CloseTab(dialog.tab_id)))
}

fn reduce_dialog(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Char(c) => app.tab_dialog.as_mut()?.insert_char(c),
        Input::Backspace => app.tab_dialog.as_mut()?.backspace(),
        Input::Esc => app.tab_dialog = None,
        Input::Enter => return confirm_dialog(app),
        _ => {}
    }
    None
}

fn confirm_dialog(app: &mut App) -> Option<Effect> {
    let mut dialog = app.tab_dialog.take()?;
    let name = dialog.value.trim().to_string();
    let mutation = match dialog.mode {
        TabDialogMode::Create => {
            let id = TabId::new();
            let activity_at = Utc::now();
            if let Err(error) = app.workspace.create_tab_with(id, name.clone(), activity_at) {
                dialog.error = error.to_string();
                app.tab_dialog = Some(dialog);
                return None;
            }
            app.select_tab(id);
            app.focus = Pane::Composer;
            QueueMutation::CreateTab {
                id,
                name,
                activity_at,
            }
        }
        TabDialogMode::Rename(id) => {
            if let Err(error) = app.workspace.rename_tab(id, name.clone()) {
                dialog.error = error.to_string();
                app.tab_dialog = Some(dialog);
                return None;
            }
            QueueMutation::RenameTab { id, name }
        }
    };
    Some(Effect::Persist(mutation))
}

fn select_adjacent_tab(app: &mut App, delta: i32) {
    let tabs = app.workspace.tabs();
    let Some(current) = tabs.iter().position(|tab| tab.id() == app.active_tab_id) else {
        return;
    };
    let next = (current as i32 + delta).clamp(0, tabs.len().saturating_sub(1) as i32) as usize;
    app.select_tab(tabs[next].id());
}

fn reduce_queue(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Down => {
            let len = app.visible_prompts().len();
            if len > 0 {
                let next = app.selected.map(|i| (i + 1).min(len - 1)).unwrap_or(0);
                app.selected = Some(next);
            }
            None
        }
        Input::Up => {
            if let Some(i) = app.selected {
                app.selected = Some(i.saturating_sub(1));
            }
            None
        }
        Input::Enter => {
            let prompt = app.selected_prompt()?.clone();
            if prompt.pinned {
                Some(Effect::CopyToClipboard(prompt.text))
            } else {
                app.workspace.remove_prompt(prompt.id).ok()?;
                reclamp_selection(app);
                if app.visible_prompts().is_empty() {
                    app.focus = Pane::Composer;
                }
                Some(Effect::CopyAndPersist {
                    text: prompt.text,
                    mutation: QueueMutation::Remove(prompt.id),
                })
            }
        }
        Input::Char('p') => {
            let prompt = app.selected_prompt()?.clone();
            app.workspace
                .set_prompt_pinned(prompt.id, !prompt.pinned)
                .ok()?;
            if let Some(new_index) = app
                .visible_prompts()
                .iter()
                .position(|candidate| candidate.id == prompt.id)
            {
                app.selected = Some(new_index);
            }
            Some(Effect::Persist(QueueMutation::SetPinned {
                id: prompt.id,
                pinned: !prompt.pinned,
            }))
        }
        Input::Char('f') => {
            let prompt = app.selected_prompt()?;
            app.preview = Some(PromptPreview {
                source: PreviewSource::Prompt(prompt.id),
                scroll: 0,
            });
            None
        }
        Input::Char('e') => {
            let prompt = app.selected_prompt()?.clone();
            app.workspace.remove_prompt(prompt.id).ok()?;
            reclamp_selection(app);
            app.composer.set_text(&prompt.text);
            app.focus = Pane::Composer;
            Some(Effect::Persist(QueueMutation::Remove(prompt.id)))
        }
        Input::OpenRenameTab => {
            let tab = app.workspace.tab(app.active_tab_id)?;
            app.tab_dialog = Some(TabDialog::rename(tab.id(), tab.name()));
            None
        }
        _ => None,
    }
}

fn reduce_composer(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Char(c) => app.composer.insert_char(c),
        Input::Paste(text) => app.composer.insert_str(&text),
        Input::Newline => app.composer.insert_newline(),
        Input::Backspace => app.composer.delete_char(),
        Input::Delete => app.composer.delete_next_char(),
        Input::DeleteWordBack => app.composer.delete_word(),
        Input::DeleteWordForward => app.composer.delete_next_word(),
        Input::DeleteToLineStart => app.composer.delete_to_line_start(),
        Input::DeleteToLineEnd => app.composer.delete_to_line_end(),
        Input::MoveLeft => app.composer.move_cursor(CursorMove::Back),
        Input::MoveRight => app.composer.move_cursor(CursorMove::Forward),
        Input::MoveUp => app.composer.move_cursor(CursorMove::Up),
        Input::MoveDown => app.composer.move_cursor(CursorMove::Down),
        Input::MoveWordLeft => app.composer.move_cursor(CursorMove::WordBack),
        Input::MoveWordRight => app.composer.move_cursor(CursorMove::WordForward),
        Input::MoveLineStart => app.composer.move_cursor(CursorMove::Head),
        Input::MoveLineEnd => app.composer.move_cursor(CursorMove::End),
        Input::Undo => app.composer.undo(),
        Input::Redo => app.composer.redo(),
        Input::Enter | Input::CtrlS => {
            let text = app.composer.text().trim().to_string();
            if text.is_empty() {
                return None;
            }
            let prompt = Prompt::new(text).ok()?;
            let id = prompt.id;
            let tab_id = app.active_tab_id;
            app.workspace.add_prompt(tab_id, prompt.clone()).ok()?;
            app.composer.clear();
            app.selected = app
                .visible_prompts()
                .iter()
                .position(|candidate| candidate.id == id);
            app.focus = Pane::Queue;
            return Some(Effect::Persist(QueueMutation::Add { tab_id, prompt }));
        }
        Input::Esc => app.focus = Pane::Queue,
        _ => return None,
    }
    None
}

fn reclamp_selection(app: &mut App) {
    let len = app.visible_prompts().len();
    if len == 0 {
        app.selected = None;
    } else if let Some(index) = app.selected {
        app.selected = Some(index.min(len - 1));
    }
}

#[cfg(test)]
#[path = "../tests/unit/reducer.rs"]
mod tests;
