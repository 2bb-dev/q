use crate::app::{App, Effect, Input, Pane};

pub fn reduce(app: &mut App, input: Input) -> Option<Effect> {
    match (app.focus, &input) {
        (_, Input::Quit) => return Some(Effect::Quit),
        (_, Input::Tab) => {
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
            if !prompt.pinned {
                app.queue.remove(prompt.id).ok()?;
                reclamp_selection(app);
            }
            Some(Effect::CopyToClipboard(prompt.text))
        }
        Input::Char('y') => {
            let prompt = app.selected_prompt()?.clone();
            Some(Effect::CopyToClipboard(prompt.text))
        }
        Input::Char('p') => {
            let prompt = app.selected_prompt()?.clone();
            app.queue.set_pinned(prompt.id, !prompt.pinned).ok()?;
            Some(Effect::Persist)
        }
        Input::ShiftDown => move_selection(app, 1),
        Input::ShiftUp => move_selection(app, -1),
        Input::Char('e') => {
            let prompt = app.selected_prompt()?.clone();
            app.queue.remove(prompt.id).ok()?;
            reclamp_selection(app);
            app.composer = prompt.text;
            app.focus = Pane::Composer;
            Some(Effect::Persist)
        }
        _ => None,
    }
}

fn reduce_composer(app: &mut App, input: Input) -> Option<Effect> {
    match input {
        Input::Char(c) => {
            app.composer.push(c);
            None
        }
        Input::Enter => {
            app.composer.push('\n');
            None
        }
        Input::Backspace => {
            app.composer.pop();
            None
        }
        Input::CtrlS => {
            let text = app.composer.trim().to_string();
            if text.is_empty() {
                return None;
            }
            app.queue.add_text(text).ok()?;
            app.composer.clear();
            let len = app.visible_prompts().len();
            app.selected = Some(len.saturating_sub(1));
            app.focus = Pane::Queue;
            Some(Effect::Persist)
        }
        Input::CtrlU => Some(Effect::Status(
            "Upgrade not yet wired — configure providers first".to_string(),
        )),
        Input::Esc => {
            app.focus = Pane::Queue;
            None
        }
        _ => None,
    }
}

fn reclamp_selection(app: &mut App) {
    let len = app.visible_prompts().len();
    if len == 0 {
        app.selected = None;
    } else if let Some(i) = app.selected {
        app.selected = Some(i.min(len - 1));
    }
}

fn move_selection(app: &mut App, delta: i32) -> Option<Effect> {
    let prompt = app.selected_prompt()?.clone();
    if app.queue.move_within_group(prompt.id, delta).ok()? {
        let new_idx = app
            .visible_prompts()
            .iter()
            .position(|p| p.id == prompt.id)?;
        app.selected = Some(new_idx);
        Some(Effect::Persist)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcli_core::Queue;

    fn app_with(n: usize) -> App {
        let mut q = Queue::new();
        for i in 0..n {
            q.add_text(format!("prompt-{i}")).unwrap();
        }
        App::new(q)
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
    fn down_moves_selection_clamped() {
        let mut app = app_with(3);
        reduce(&mut app, Input::Down);
        assert_eq!(app.selected, Some(1));
        reduce(&mut app, Input::Down);
        reduce(&mut app, Input::Down);
        assert_eq!(app.selected, Some(2));
    }

    #[test]
    fn up_clamps_at_zero() {
        let mut app = app_with(2);
        reduce(&mut app, Input::Up);
        assert_eq!(app.selected, Some(0));
    }

    #[test]
    fn quit_returns_quit_effect() {
        let mut app = app_with(0);
        assert_eq!(reduce(&mut app, Input::Quit), Some(Effect::Quit));
    }

    #[test]
    fn enter_on_unpinned_copies_and_pops() {
        let mut app = app_with(2);
        let text = app.selected_prompt().unwrap().text.clone();
        let effect = reduce(&mut app, Input::Enter);
        assert_eq!(effect, Some(Effect::CopyToClipboard(text)));
        assert_eq!(app.visible_prompts().len(), 1);
    }

    #[test]
    fn enter_on_pinned_copies_but_does_not_pop() {
        let mut app = app_with(1);
        let pid = app.visible_prompts()[0].id;
        app.queue.set_pinned(pid, true).unwrap();
        let text = app.selected_prompt().unwrap().text.clone();
        let effect = reduce(&mut app, Input::Enter);
        assert_eq!(effect, Some(Effect::CopyToClipboard(text)));
        assert_eq!(app.visible_prompts().len(), 1);
    }

    #[test]
    fn y_copies_without_popping() {
        let mut app = app_with(2);
        let text = app.selected_prompt().unwrap().text.clone();
        let effect = reduce(&mut app, Input::Char('y'));
        assert_eq!(effect, Some(Effect::CopyToClipboard(text)));
        assert_eq!(app.visible_prompts().len(), 2);
    }

    #[test]
    fn p_toggles_pin_and_emits_persist() {
        let mut app = app_with(1);
        let effect = reduce(&mut app, Input::Char('p'));
        assert_eq!(effect, Some(Effect::Persist));
        assert!(app.selected_prompt().unwrap().pinned);
        let effect2 = reduce(&mut app, Input::Char('p'));
        assert_eq!(effect2, Some(Effect::Persist));
        assert!(!app.selected_prompt().unwrap().pinned);
    }

    #[test]
    fn shift_down_moves_prompt_down_within_group() {
        let mut app = app_with(3);
        let ids: Vec<_> = app.visible_prompts().iter().map(|p| p.id).collect();
        let effect = reduce(&mut app, Input::ShiftDown);
        assert_eq!(effect, Some(Effect::Persist));
        let after: Vec<_> = app.visible_prompts().iter().map(|p| p.id).collect();
        assert_eq!(after[0], ids[1]);
        assert_eq!(after[1], ids[0]);
        assert_eq!(app.selected, Some(1));
    }

    #[test]
    fn shift_up_at_top_is_noop() {
        let mut app = app_with(2);
        let effect = reduce(&mut app, Input::ShiftUp);
        assert_eq!(effect, None);
    }

    #[test]
    fn e_loads_selected_into_composer_and_focuses_composer() {
        let mut app = app_with(1);
        let text = app.selected_prompt().unwrap().text.clone();
        reduce(&mut app, Input::Char('e'));
        assert_eq!(app.composer, text);
        assert_eq!(app.focus, Pane::Composer);
    }

    #[test]
    fn composer_chars_append() {
        let mut app = app_with(0);
        app.focus = Pane::Composer;
        reduce(&mut app, Input::Char('h'));
        reduce(&mut app, Input::Char('i'));
        assert_eq!(app.composer, "hi");
    }

    #[test]
    fn composer_enter_inserts_newline() {
        let mut app = app_with(0);
        app.focus = Pane::Composer;
        reduce(&mut app, Input::Char('a'));
        reduce(&mut app, Input::Enter);
        reduce(&mut app, Input::Char('b'));
        assert_eq!(app.composer, "a\nb");
    }

    #[test]
    fn composer_backspace_pops_last_char() {
        let mut app = app_with(0);
        app.focus = Pane::Composer;
        reduce(&mut app, Input::Char('a'));
        reduce(&mut app, Input::Char('b'));
        reduce(&mut app, Input::Backspace);
        assert_eq!(app.composer, "a");
    }

    #[test]
    fn ctrl_s_adds_prompt_and_clears_composer() {
        let mut app = app_with(0);
        app.focus = Pane::Composer;
        app.composer = "new prompt".to_string();
        let effect = reduce(&mut app, Input::CtrlS);
        assert_eq!(effect, Some(Effect::Persist));
        assert_eq!(app.composer, "");
        assert_eq!(app.visible_prompts().len(), 1);
        assert_eq!(app.visible_prompts()[0].text, "new prompt");
        assert_eq!(app.focus, Pane::Queue);
    }

    #[test]
    fn ctrl_s_on_empty_composer_is_noop() {
        let mut app = app_with(0);
        app.focus = Pane::Composer;
        let effect = reduce(&mut app, Input::CtrlS);
        assert_eq!(effect, None);
    }

    #[test]
    fn ctrl_u_in_composer_sets_status() {
        let mut app = app_with(0);
        app.focus = Pane::Composer;
        app.composer = "hi".to_string();
        let effect = reduce(&mut app, Input::CtrlU);
        assert!(matches!(effect, Some(Effect::Status(_))));
    }
}
