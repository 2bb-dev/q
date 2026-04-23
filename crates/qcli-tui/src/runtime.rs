use crate::app::{App, Effect};
use crate::reducer::reduce;
use crate::render::draw;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use qcli_platform::clipboard::{Clipboard, SystemClipboard};
use qcli_platform::lock::FileLock;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::Input;
use crate::Pane;

pub fn run(queue_path: &Path) -> Result<()> {
    let _lock = FileLock::acquire(&queue_path.with_extension("lock"))?;
    let queue = qcli_core::storage::load(queue_path)?;
    let mut app = App::new(queue);
    let mut clipboard = SystemClipboard::new()?;

    let mut term = setup_terminal()?;
    let result = event_loop(&mut term, &mut app, &mut clipboard, queue_path);
    restore_terminal(&mut term)?;
    result
}

fn event_loop(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    clipboard: &mut dyn Clipboard,
    queue_path: &Path,
) -> Result<()> {
    const BLINK_PERIOD_MS: u128 = 1000;
    const BLINK_ON_MS: u128 = 650;
    let start = Instant::now();
    loop {
        let cursor_on = start.elapsed().as_millis() % BLINK_PERIOD_MS < BLINK_ON_MS;
        term.draw(|f| draw(f, app, cursor_on))?;
        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        let Some(input) = map_key(key, app.focus) else {
            continue;
        };
        match reduce(app, input) {
            Some(Effect::Quit) => return Ok(()),
            Some(Effect::CopyToClipboard(text)) => {
                clipboard.set_text(&text)?;
                app.status = format!("copied {} chars", text.chars().count());
            }
            Some(Effect::CopyAndPersist(text)) => {
                clipboard.set_text(&text)?;
                qcli_core::storage::save(queue_path, &app.queue)?;
                app.status = format!("copied {} chars", text.chars().count());
            }
            Some(Effect::Persist) => {
                qcli_core::storage::save(queue_path, &app.queue)?;
                app.status.clear();
            }
            Some(Effect::Status(msg)) => {
                app.status = msg;
            }
            None => {
                if !app.status.is_empty() {
                    app.status.clear();
                }
            }
        }
    }
}

fn map_key(key: KeyEvent, focus: Pane) -> Option<Input> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    Some(match (key.code, ctrl, shift, focus) {
        (KeyCode::Char('q'), false, false, Pane::Queue) => Input::Quit,
        (KeyCode::Char('c'), true, _, _) => Input::Quit,
        (KeyCode::Char('s'), true, _, _) => Input::CtrlS,
        (KeyCode::Tab, _, _, _) => Input::Tab,
        (KeyCode::Esc, _, _, _) => Input::Esc,
        (KeyCode::Enter, _, _, _) => Input::Enter,
        (KeyCode::Backspace, _, _, _) => Input::Backspace,
        (KeyCode::Up, _, _, _) => Input::Up,
        (KeyCode::Down, _, _, _) => Input::Down,
        (KeyCode::Char('j'), false, false, Pane::Queue) => Input::Down,
        (KeyCode::Char('k'), false, false, Pane::Queue) => Input::Up,
        (KeyCode::Char(c), false, _, _) => Input::Char(c),
        _ => return None,
    })
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, Clear(ClearType::All))?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;
    term.clear()?;
    term.hide_cursor()?;
    Ok(term)
}

fn restore_terminal(term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    term.show_cursor()?;
    execute!(term.backend_mut(), Clear(ClearType::All))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with_mods(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
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
        assert_eq!(
            map_key(
                with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Pane::Queue
            ),
            Some(Input::Quit)
        );
        assert_eq!(
            map_key(
                with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Pane::Composer
            ),
            Some(Input::Quit)
        );
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
    fn tab_esc_enter_backspace_are_mapped() {
        assert_eq!(map_key(key(KeyCode::Tab), Pane::Queue), Some(Input::Tab));
        assert_eq!(map_key(key(KeyCode::Esc), Pane::Composer), Some(Input::Esc));
        assert_eq!(
            map_key(key(KeyCode::Enter), Pane::Queue),
            Some(Input::Enter)
        );
        assert_eq!(
            map_key(key(KeyCode::Backspace), Pane::Composer),
            Some(Input::Backspace)
        );
    }

    #[test]
    fn arrow_keys_map_to_up_down() {
        assert_eq!(map_key(key(KeyCode::Up), Pane::Queue), Some(Input::Up));
        assert_eq!(map_key(key(KeyCode::Down), Pane::Queue), Some(Input::Down));
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
        assert_eq!(
            map_key(key(KeyCode::Char('k')), Pane::Composer),
            Some(Input::Char('k'))
        );
    }

    #[test]
    fn printable_chars_pass_through_in_composer() {
        assert_eq!(
            map_key(key(KeyCode::Char('x')), Pane::Composer),
            Some(Input::Char('x'))
        );
        assert_eq!(
            map_key(key(KeyCode::Char(' ')), Pane::Composer),
            Some(Input::Char(' '))
        );
    }

    #[test]
    fn unmapped_keys_return_none() {
        assert_eq!(map_key(key(KeyCode::F(5)), Pane::Queue), None);
        assert_eq!(map_key(key(KeyCode::Home), Pane::Composer), None);
    }
}
