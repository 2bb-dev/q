use crate::app::{App, Effect};
use crate::reducer::reduce;
use crate::render::draw;
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use qcli_core::Queue;
use qcli_platform::clipboard::{Clipboard, SystemClipboard};
use qcli_platform::lock::FileLock;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::path::Path;
use std::time::Duration;

use crate::Input;
use crate::Pane;

pub fn run(queue_path: &Path) -> Result<()> {
    let _lock = FileLock::acquire(queue_path)?;
    let queue = qcli_core::storage::load(queue_path).unwrap_or_else(|_| Queue::new());
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
    loop {
        term.draw(|f| draw(f, app))?;
        if !event::poll(Duration::from_millis(250))? {
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
        (KeyCode::Char('u'), true, _, _) => Input::CtrlU,
        (KeyCode::Tab, _, _, _) => Input::Tab,
        (KeyCode::Esc, _, _, _) => Input::Esc,
        (KeyCode::Enter, _, _, _) => Input::Enter,
        (KeyCode::Backspace, _, _, _) => Input::Backspace,
        (KeyCode::Up, _, true, _) => Input::ShiftUp,
        (KeyCode::Down, _, true, _) => Input::ShiftDown,
        (KeyCode::Up, _, _, _) => Input::Up,
        (KeyCode::Down, _, _, _) => Input::Down,
        (KeyCode::Char('j'), false, false, Pane::Queue) => Input::Down,
        (KeyCode::Char('k'), false, false, Pane::Queue) => Input::Up,
        (KeyCode::Char('J'), false, _, Pane::Queue) => Input::ShiftDown,
        (KeyCode::Char('K'), false, _, Pane::Queue) => Input::ShiftUp,
        (KeyCode::Char(c), false, _, _) => Input::Char(c),
        _ => return None,
    })
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

fn restore_terminal(term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;
    Ok(())
}
