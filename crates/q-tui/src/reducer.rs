use crate::app::{
    App, CloseTabDialog, ConnectState, DeletePromptDialog, EditorOrigin, Effect, GithubAuthState,
    InfoAction, InfoMode, Input, MenuItem, MenuState, Pane, PreviewSource, PromptPreview,
    QueueMutation, SearchDialog, TabContextMenu, TabDialog, TabDialogMode, TabMenuAction,
    WorkspaceInfo, WorkspacesMode,
};
use chrono::Utc;
use q_core::{Prompt, PromptSource, TabId};
use ratatui_textarea::CursorMove;

pub fn reduce(app: &mut App, input: Input) -> Option<Effect> {
    if matches!(input, Input::Quit) {
        return Some(Effect::Quit);
    }
    if app.editor.is_some() {
        return reduce_editor(app, input);
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
    if app.menu.is_some() {
        return reduce_menu(app, input);
    }
    if app.delete_prompt_dialog.is_some() {
        return reduce_delete_prompt_dialog(app, input);
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
        Input::OpenMenu => {
            app.menu = Some(MenuState::Root {
                selected: MenuItem::Workspaces,
            });
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

fn reduce_menu(app: &mut App, input: Input) -> Option<Effect> {
    let menu = app.menu.as_mut()?;
    match menu {
        MenuState::Root { selected } => match input {
            Input::Esc => app.menu = None,
            Input::Up | Input::Down | Input::Tab => {
                *selected = match selected {
                    MenuItem::Workspaces => MenuItem::Settings,
                    MenuItem::Settings => MenuItem::Workspaces,
                };
            }
            Input::Enter => match selected {
                MenuItem::Workspaces => return Some(Effect::OpenWorkspacesOverlay),
                MenuItem::Settings => {
                    *menu = MenuState::Settings;
                    return Some(Effect::RefreshGithubStatus);
                }
            },
            _ => {}
        },
        MenuState::Settings => match input {
            Input::Esc => {
                *menu = MenuState::Root {
                    selected: MenuItem::Settings,
                };
            }
            Input::Enter
                if matches!(
                    app.github,
                    GithubAuthState::NotConnected | GithubAuthState::Failed(_)
                ) =>
            {
                return Some(Effect::GithubConnect);
            }
            Input::Char('d') if matches!(app.github, GithubAuthState::Connected { .. }) => {
                return Some(Effect::GithubDisconnect);
            }
            _ => {}
        },
        MenuState::Workspaces(overlay) => {
            overlay.error.clear();
            match &mut overlay.mode {
                WorkspacesMode::List => match input {
                    Input::Esc => {
                        *menu = MenuState::Root {
                            selected: MenuItem::Workspaces,
                        };
                    }
                    Input::Up | Input::Char('k') => {
                        overlay.selected = overlay.selected.saturating_sub(1);
                    }
                    Input::Down | Input::Char('j') => {
                        overlay.selected =
                            (overlay.selected + 1).min(overlay.entries.len().saturating_sub(1));
                    }
                    Input::Enter => {
                        let dir = overlay.selected_entry()?.dir.clone();
                        return Some(Effect::SwitchWorkspace(dir));
                    }
                    Input::Char('n') => {
                        overlay.mode = WorkspacesMode::Create {
                            value: String::new(),
                            team: false,
                        };
                    }
                    Input::Char('t') => {
                        overlay.mode = WorkspacesMode::Create {
                            value: String::new(),
                            team: true,
                        };
                    }
                    Input::Char('i') if overlay.selected_entry().is_some() => {
                        let entry = overlay.selected_entry()?;
                        let team_dir = entry.team.then(|| entry.dir.clone());
                        overlay.mode = WorkspacesMode::Info(WorkspaceInfo {
                            action: InfoAction::Rename,
                            mode: InfoMode::View,
                            details: None,
                        });
                        if let Some(dir) = team_dir {
                            return Some(Effect::FetchTeamInfo(dir));
                        }
                    }
                    Input::Char('c') => {
                        overlay.mode = WorkspacesMode::Connect(ConnectState::Loading);
                        return Some(Effect::OpenConnect);
                    }
                    _ => {}
                },
                WorkspacesMode::Connect(state) => match state {
                    ConnectState::Loading | ConnectState::Working => {
                        if matches!(input, Input::Esc) {
                            overlay.mode = WorkspacesMode::List;
                        }
                    }
                    ConnectState::Ready {
                        invitations,
                        repos,
                        selected,
                    } => match input {
                        Input::Esc => overlay.mode = WorkspacesMode::List,
                        Input::Up | Input::Char('k') => *selected = selected.saturating_sub(1),
                        Input::Down | Input::Char('j') => {
                            let last = (invitations.len() + repos.len()).saturating_sub(1);
                            *selected = (*selected + 1).min(last);
                        }
                        Input::Enter => {
                            if let Some(invitation) = invitations.get(*selected) {
                                let id = invitation.id;
                                *state = ConnectState::Working;
                                return Some(Effect::AcceptInvitation(id));
                            }
                            let index = *selected - invitations.len();
                            if let Some(repo) = repos.get(index) {
                                let effect = Effect::ConnectRepo {
                                    full_name: repo.full_name.clone(),
                                    clone_url: repo.clone_url.clone(),
                                };
                                *state = ConnectState::Working;
                                return Some(effect);
                            }
                        }
                        _ => {}
                    },
                },
                WorkspacesMode::Create { value, team } => match input {
                    Input::Esc => overlay.mode = WorkspacesMode::List,
                    Input::Char(c) => value.push(c),
                    Input::Backspace => {
                        value.pop();
                    }
                    Input::Enter => {
                        return Some(Effect::CreateWorkspace {
                            name: value.clone(),
                            team: *team,
                        });
                    }
                    _ => {}
                },
                WorkspacesMode::Info(info) => match &mut info.mode {
                    InfoMode::View => match input {
                        Input::Esc => overlay.mode = WorkspacesMode::List,
                        Input::Up | Input::Char('k') => {
                            let team = overlay
                                .entries
                                .get(overlay.selected)
                                .is_some_and(|entry| entry.team);
                            info.action = cycle_info_action(info.action, team, -1);
                        }
                        Input::Down | Input::Tab | Input::Char('j') => {
                            let team = overlay
                                .entries
                                .get(overlay.selected)
                                .is_some_and(|entry| entry.team);
                            info.action = cycle_info_action(info.action, team, 1);
                        }
                        Input::Enter => match info.action {
                            InfoAction::Rename => {
                                let value = overlay
                                    .entries
                                    .get(overlay.selected)
                                    .map(|entry| entry.name.clone())
                                    .unwrap_or_default();
                                if let WorkspacesMode::Info(info) = &mut overlay.mode {
                                    info.mode = InfoMode::Rename { value };
                                }
                            }
                            InfoAction::ConvertToTeam => {
                                info.mode = InfoMode::LoadingOwners;
                                return Some(Effect::FetchRepoOwners);
                            }
                            InfoAction::Delete => {
                                if overlay.entries.len() == 1 {
                                    overlay.error = "cannot delete the last workspace".to_string();
                                } else if let WorkspacesMode::Info(info) = &mut overlay.mode {
                                    info.mode = InfoMode::ConfirmDelete;
                                }
                            }
                            InfoAction::Invite => {
                                info.mode = InfoMode::Invite {
                                    value: String::new(),
                                };
                            }
                            InfoAction::Leave => {
                                if overlay.entries.len() == 1 {
                                    overlay.error = "cannot delete the last workspace".to_string();
                                } else if let WorkspacesMode::Info(info) = &mut overlay.mode {
                                    info.mode = InfoMode::ConfirmLeave;
                                }
                            }
                            InfoAction::DeleteRepo => {
                                info.mode = InfoMode::ConfirmDeleteRepo;
                            }
                        },
                        _ => {}
                    },
                    InfoMode::Invite { value } => match input {
                        Input::Esc => info.mode = InfoMode::View,
                        Input::Char(c) => value.push(c),
                        Input::Backspace => {
                            value.pop();
                        }
                        Input::Enter => {
                            let username = value.trim().to_string();
                            if username.is_empty() {
                                return None;
                            }
                            info.mode = InfoMode::View;
                            let dir = overlay.entries.get(overlay.selected)?.dir.clone();
                            return Some(Effect::InviteCollaborator { dir, username });
                        }
                        _ => {}
                    },
                    InfoMode::ConfirmLeave => match input {
                        Input::Esc => info.mode = InfoMode::View,
                        Input::Enter => {
                            let dir = overlay.entries.get(overlay.selected)?.dir.clone();
                            return Some(Effect::DeleteWorkspace(dir));
                        }
                        _ => {}
                    },
                    InfoMode::ConfirmDeleteRepo => match input {
                        Input::Esc => info.mode = InfoMode::View,
                        Input::Enter => {
                            let dir = overlay.entries.get(overlay.selected)?.dir.clone();
                            return Some(Effect::DeleteRepo(dir));
                        }
                        _ => {}
                    },
                    InfoMode::LoadingOwners | InfoMode::Converting => {
                        if matches!(input, Input::Esc) {
                            info.mode = InfoMode::View;
                        }
                    }
                    InfoMode::SelectOwner { owners, selected } => match input {
                        Input::Esc => info.mode = InfoMode::View,
                        Input::Up | Input::Char('k') => *selected = selected.saturating_sub(1),
                        Input::Down | Input::Char('j') => {
                            *selected = (*selected + 1).min(owners.len().saturating_sub(1));
                        }
                        Input::Enter => {
                            let org = owners.get(*selected).cloned().flatten();
                            info.mode = InfoMode::Converting;
                            let dir = overlay.entries.get(overlay.selected)?.dir.clone();
                            return Some(Effect::ConvertToTeam { dir, org });
                        }
                        _ => {}
                    },
                    InfoMode::Rename { value } => match input {
                        Input::Esc => info.mode = InfoMode::View,
                        Input::Char(c) => value.push(c),
                        Input::Backspace => {
                            value.pop();
                        }
                        Input::Enter => {
                            let name = value.clone();
                            let dir = overlay.entries.get(overlay.selected)?.dir.clone();
                            return Some(Effect::RenameWorkspace { dir, name });
                        }
                        _ => {}
                    },
                    InfoMode::ConfirmDelete => match input {
                        Input::Esc => info.mode = InfoMode::View,
                        Input::Enter => {
                            let dir = overlay.entries.get(overlay.selected)?.dir.clone();
                            return Some(Effect::DeleteWorkspace(dir));
                        }
                        _ => {}
                    },
                },
            }
        }
    }
    None
}

/// Moves the info-dialog selection through the actions available for the
/// entry, clamping at the ends.
fn cycle_info_action(current: InfoAction, team: bool, delta: i32) -> InfoAction {
    let actions = InfoAction::available(team);
    let index = actions
        .iter()
        .position(|action| *action == current)
        .unwrap_or(0);
    let target = (index as i32 + delta).clamp(0, actions.len() as i32 - 1) as usize;
    actions[target]
}

fn reduce_editor(app: &mut App, input: Input) -> Option<Effect> {
    if app.editor.as_ref()?.discard_confirmation {
        match input {
            Input::Enter => app.editor = None,
            Input::Esc => app.editor.as_mut()?.discard_confirmation = false,
            _ => {}
        }
        return None;
    }

    match input {
        Input::CtrlS => save_editor(app),
        Input::Esc => {
            if app.editor.as_ref()?.is_dirty() {
                app.editor.as_mut()?.discard_confirmation = true;
            } else {
                app.editor = None;
            }
            None
        }
        Input::Char(c) => {
            app.editor.as_mut()?.buffer.insert_char(c);
            None
        }
        Input::Paste(text) => {
            app.editor.as_mut()?.buffer.insert_str(&text);
            None
        }
        Input::Enter | Input::Newline => {
            app.editor.as_mut()?.buffer.insert_newline();
            None
        }
        Input::Backspace => {
            app.editor.as_mut()?.buffer.delete_char();
            None
        }
        Input::Delete => {
            app.editor.as_mut()?.buffer.delete_next_char();
            None
        }
        Input::DeleteWordBack => {
            app.editor.as_mut()?.buffer.delete_word();
            None
        }
        Input::DeleteWordForward => {
            app.editor.as_mut()?.buffer.delete_next_word();
            None
        }
        Input::DeleteToLineStart => {
            app.editor.as_mut()?.buffer.delete_to_line_start();
            None
        }
        Input::DeleteToLineEnd => {
            app.editor.as_mut()?.buffer.delete_to_line_end();
            None
        }
        Input::MoveLeft => move_editor(app, CursorMove::Back),
        Input::MoveRight => move_editor(app, CursorMove::Forward),
        Input::MoveUp => move_editor(app, CursorMove::Up),
        Input::MoveDown => move_editor(app, CursorMove::Down),
        Input::MoveWordLeft => move_editor(app, CursorMove::WordBack),
        Input::MoveWordRight => move_editor(app, CursorMove::WordForward),
        Input::MoveLineStart => move_editor(app, CursorMove::Head),
        Input::MoveLineEnd => move_editor(app, CursorMove::End),
        Input::Undo => {
            app.editor.as_mut()?.buffer.undo();
            None
        }
        Input::Redo => {
            app.editor.as_mut()?.buffer.redo();
            None
        }
        _ => None,
    }
}

fn move_editor(app: &mut App, movement: CursorMove) -> Option<Effect> {
    app.editor.as_mut()?.buffer.move_cursor(movement);
    None
}

fn save_editor(app: &mut App) -> Option<Effect> {
    if !app.editor.as_ref()?.is_dirty() {
        app.editor = None;
        return None;
    }
    let text = app.editor.as_ref()?.buffer.text();
    let inline_id = app.editor.as_ref()?.inline_id();
    if let Some(id) = inline_id {
        if let Err(error) = PromptSource::inline(text.clone()) {
            app.editor.as_mut()?.error = error.to_string();
            return None;
        }
        let (expected_source, expected_pinned) = app.editor.as_ref()?.expected_inline_state()?;
        return Some(Effect::Persist(QueueMutation::EditInline {
            id,
            expected_source: expected_source.clone(),
            expected_pinned,
            text,
        }));
    }

    match app.editor.as_ref()?.origin {
        EditorOrigin::External { .. } => Some(Effect::SaveExternal),
        EditorOrigin::Inline { .. } => None,
    }
}

fn reduce_preview(app: &mut App, input: Input) -> Option<Effect> {
    let page = app.preview_page;
    let max_scroll = app.preview_max_scroll;
    match input {
        Input::Esc | Input::Char('f') | Input::Char('q') => app.preview = None,
        Input::Enter => match app.preview_live_text() {
            Ok(text) => {
                app.preview = None;
                app.search = None;
                return Some(Effect::CopyToClipboard(text));
            }
            Err(error) => return Some(Effect::Status(error)),
        },
        Input::Char('e') => {
            let source = app.preview_source()?.clone();
            let id = match app.preview.as_ref()?.source {
                PreviewSource::Prompt(id) => Some(id),
                PreviewSource::History(_) => None,
            };
            if let Err(error) = app.open_editor_for_source(source, id) {
                return Some(Effect::Status(error));
            }
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
            app.search.as_mut()?.selected = app.search.as_ref()?.selected.saturating_sub(1)
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
    let source = app.search_results().get(selected)?.source().clone();
    if !app.workspace.forget_history(&source) {
        return None;
    }
    app.search_folds.clear();
    let remaining = app.search_results().len();
    if let Some(search) = app.search.as_mut() {
        search.selected = selected.min(remaining.saturating_sub(1));
    }
    Some(Effect::Persist(QueueMutation::ForgetHistory(source)))
}

fn open_history_preview(app: &mut App, index: usize) {
    let Some(source) = app
        .search_results()
        .get(index)
        .map(|entry| entry.source().clone())
    else {
        return;
    };
    app.preview = Some(PromptPreview {
        source: PreviewSource::History(source),
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
        TabMenuAction::Rename => app.tab_dialog = Some(TabDialog::rename(tab.id(), tab.name())),
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

fn reduce_delete_prompt_dialog(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Esc => app.delete_prompt_dialog = None,
        Input::Enter => {
            let dialog = app.delete_prompt_dialog.take()?;
            return Some(Effect::Persist(QueueMutation::Remove {
                id: dialog.prompt_id,
                expected_source: dialog.expected_source,
                expected_pinned: dialog.expected_pinned,
                expected_external_content: None,
            }));
        }
        _ => {}
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
            if let Err(error) =
                app.workspace
                    .create_tab_with(id, name.clone(), activity_at, app.identity.clone())
            {
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
                app.selected = Some(app.selected.map(|i| (i + 1).min(len - 1)).unwrap_or(0));
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
            let text = match app.resolve_source_owned(prompt.source()) {
                Ok(text) => text,
                Err(error) => return Some(Effect::Status(error)),
            };
            if prompt.pinned() {
                Some(Effect::CopyToClipboard(text))
            } else {
                let expected_external_content = prompt
                    .external_markdown_path()
                    .is_some()
                    .then(|| text.clone());
                Some(Effect::CopyAndPersist {
                    text,
                    mutation: QueueMutation::Remove {
                        id: prompt.id,
                        expected_source: prompt.source().clone(),
                        expected_pinned: prompt.pinned(),
                        expected_external_content,
                    },
                })
            }
        }
        Input::Char('p') => {
            let prompt = app.selected_prompt()?.clone();
            app.workspace
                .set_prompt_pinned(prompt.id, !prompt.pinned())
                .ok()?;
            app.selected = app
                .visible_prompts()
                .iter()
                .position(|candidate| candidate.id == prompt.id);
            Some(Effect::Persist(QueueMutation::SetPinned {
                id: prompt.id,
                pinned: !prompt.pinned(),
            }))
        }
        Input::Char('f') => {
            let id = app.selected_prompt()?.id;
            app.preview = Some(PromptPreview {
                source: PreviewSource::Prompt(id),
                scroll: 0,
            });
            None
        }
        Input::Char('e') => {
            let prompt = app.selected_prompt()?.clone();
            if let Err(error) = app.open_editor_for_source(prompt.source().clone(), Some(prompt.id))
            {
                return Some(Effect::Status(error));
            }
            None
        }
        Input::Char('d') => {
            let prompt = app.selected_prompt()?;
            app.delete_prompt_dialog = Some(DeletePromptDialog {
                prompt_id: prompt.id,
                expected_source: prompt.source().clone(),
                expected_pinned: prompt.pinned(),
            });
            None
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
            let mut prompt = Prompt::new(text).ok()?;
            prompt.created_by = app.identity.clone();
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

#[cfg(test)]
#[path = "../tests/unit/reducer.rs"]
mod tests;
