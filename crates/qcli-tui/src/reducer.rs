use crate::app::{App, Effect, Input, Pane};

pub fn reduce(app: &mut App, input: Input) -> Option<Effect> {
    match (app.focus, input) {
        (_, Input::Quit) => Some(Effect::Quit),
        (_, Input::Tab) => {
            app.focus = match app.focus {
                Pane::Queue => Pane::Composer,
                Pane::Composer => Pane::Queue,
            };
            None
        }
        (Pane::Queue, Input::Down) => {
            let len = app.visible_prompts().len();
            if len > 0 {
                let next = app.selected.map(|i| (i + 1).min(len - 1)).unwrap_or(0);
                app.selected = Some(next);
            }
            None
        }
        (Pane::Queue, Input::Up) => {
            if let Some(i) = app.selected {
                app.selected = Some(i.saturating_sub(1));
            }
            None
        }
        _ => None,
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
}
