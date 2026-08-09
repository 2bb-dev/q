use crate::app::{
    App, CloseTabDialog, Effect, Input, Pane, QueueMutation, TabContextMenu, TabDialog,
    TabDialogMode, TabMenuAction,
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

    match input {
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
mod tests {
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
}
