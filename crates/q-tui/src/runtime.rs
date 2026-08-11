use crate::app::{App, Effect, QueueMutation};
use crate::reducer::reduce;
use crate::render::draw;
use crate::{Input, Pane};
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use q_platform::clipboard::{Clipboard, SystemClipboard};
use q_platform::lock::FileLock;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::path::Path;
use std::time::{Duration, Instant};

const BLINK_PERIOD_MS: u128 = 1000;
const BLINK_ON_MS: u128 = 650;
const INPUT_BATCH_BUDGET: Duration = Duration::from_millis(8);
const SYNC_INTERVAL: Duration = Duration::from_millis(250);
const FULL_RELOAD_INTERVAL: Duration = Duration::from_secs(2);
const KEYBOARD_ENHANCEMENTS: KeyboardEnhancementFlags =
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES);

pub fn run(queue_path: &Path) -> Result<()> {
    let queue = q_core::storage::load(queue_path)?;
    let mut app = App::new(queue);
    let mut clipboard = SystemClipboard::new()?;

    let mut terminal = TerminalSession::new()?;
    let result = event_loop(
        terminal.terminal_mut(),
        &mut app,
        &mut clipboard,
        queue_path,
    );
    let restore_result = terminal.restore();
    result.and(restore_result)
}

fn event_loop(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    clipboard: &mut dyn Clipboard,
    queue_path: &Path,
) -> Result<()> {
    let start = Instant::now();
    let mut sync = QueueSync::new(queue_path);
    loop {
        sync.refresh_if_due(app, queue_path);

        let cursor_on = cursor_is_on(start.elapsed());
        term.draw(|f| draw(f, app, cursor_on))?;

        let timeout = blink_timeout(start.elapsed()).min(sync.time_until_check());
        if !event::poll(timeout)? {
            continue;
        }

        let batch_start = Instant::now();
        loop {
            if handle_event(event::read()?, app, clipboard, queue_path)? {
                return Ok(());
            }
            if batch_start.elapsed() >= INPUT_BATCH_BUDGET || !event::poll(Duration::ZERO)? {
                break;
            }
        }
    }
}

fn cursor_is_on(elapsed: Duration) -> bool {
    elapsed.as_millis() % BLINK_PERIOD_MS < BLINK_ON_MS
}

fn blink_timeout(elapsed: Duration) -> Duration {
    let position = elapsed.as_millis() % BLINK_PERIOD_MS;
    let remaining = if position < BLINK_ON_MS {
        BLINK_ON_MS - position
    } else {
        BLINK_PERIOD_MS - position
    };
    Duration::from_millis(remaining.max(1) as u64)
}

struct QueueSync {
    fingerprint: Option<q_core::storage::FileFingerprint>,
    last_check: Instant,
    last_reload: Instant,
}

impl QueueSync {
    fn new(queue_path: &Path) -> Self {
        let now = Instant::now();
        Self {
            fingerprint: q_core::storage::fingerprint(queue_path).ok().flatten(),
            last_check: now,
            last_reload: now - FULL_RELOAD_INTERVAL,
        }
    }

    fn time_until_check(&self) -> Duration {
        SYNC_INTERVAL.saturating_sub(self.last_check.elapsed())
    }

    fn refresh_if_due(&mut self, app: &mut App, queue_path: &Path) {
        if self.last_check.elapsed() < SYNC_INTERVAL {
            return;
        }
        self.last_check = Instant::now();

        let fingerprint = match q_core::storage::fingerprint(queue_path) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                app.status = format!("sync failed: {error}");
                return;
            }
        };
        let forced_reload = self.last_reload.elapsed() >= FULL_RELOAD_INTERVAL;
        if fingerprint == self.fingerprint && !forced_reload {
            return;
        }

        match q_core::storage::load(queue_path) {
            Ok(workspace) => {
                app.replace_workspace(workspace);
                self.fingerprint = fingerprint;
                self.last_reload = Instant::now();
            }
            Err(error) => app.status = format!("sync failed: {error}"),
        }
    }
}

enum MutationOutcome {
    Committed(q_core::Workspace),
    Rejected(q_core::Workspace, String),
}

fn commit_mutation(queue_path: &Path, mutation: &QueueMutation) -> Result<MutationOutcome> {
    let mut lock = FileLock::open(&queue_path.with_extension("lock"))?;
    let _guard = lock.write()?;
    let mut workspace = q_core::storage::load(queue_path)?;

    let result = match mutation {
        QueueMutation::Add { tab_id, prompt } => {
            workspace.add_prompt(*tab_id, prompt.clone()).map(|_| ())
        }
        QueueMutation::Remove(id) => workspace.remove_prompt(*id).map(|_| ()),
        QueueMutation::SetPinned { id, pinned } => workspace.set_prompt_pinned(*id, *pinned),
        QueueMutation::CreateTab {
            id,
            name,
            activity_at,
        } => workspace
            .create_tab_with(*id, name.clone(), *activity_at)
            .map(|_| ()),
        QueueMutation::RenameTab { id, name } => workspace.rename_tab(*id, name.clone()),
        QueueMutation::CloseTab(id) => workspace.close_tab(*id),
        QueueMutation::ForgetHistory(text) => {
            workspace.forget_history(text);
            Ok(())
        }
    };
    if let Err(error) = result {
        return Ok(MutationOutcome::Rejected(workspace, error.to_string()));
    }

    q_core::storage::save(queue_path, &workspace)?;
    Ok(MutationOutcome::Committed(workspace))
}

fn persist_mutation(app: &mut App, queue_path: &Path, mutation: &QueueMutation) -> Result<bool> {
    match commit_mutation(queue_path, mutation)? {
        MutationOutcome::Committed(workspace) => {
            app.replace_workspace(workspace);
            Ok(true)
        }
        MutationOutcome::Rejected(workspace, error) => {
            app.replace_workspace(workspace);
            match mutation {
                QueueMutation::CreateTab { name, .. } => {
                    let mut dialog = crate::app::TabDialog::create();
                    dialog.value = name.clone();
                    dialog.error = error.clone();
                    app.tab_dialog = Some(dialog);
                }
                QueueMutation::RenameTab { id, name } => {
                    let mut dialog = crate::app::TabDialog::rename(*id, name);
                    dialog.error = error.clone();
                    app.tab_dialog = Some(dialog);
                }
                _ => {}
            }
            app.status = error;
            Ok(false)
        }
    }
}

fn handle_event(
    event: Event,
    app: &mut App,
    clipboard: &mut dyn Clipboard,
    queue_path: &Path,
) -> Result<bool> {
    let input = match event {
        Event::Key(key)
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                && app.tab_menu.is_some() =>
        {
            map_tab_menu_key(key)
        }
        Event::Key(key)
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                && app.preview.is_some() =>
        {
            map_preview_key(key)
        }
        Event::Key(key)
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                && app.search.is_some() =>
        {
            map_search_key(key)
        }
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            map_key(key, app.focus, app.dialog_open())
        }
        Event::Paste(text) if app.focus == Pane::Composer && !app.overlay_open() => {
            Some(Input::Paste(text))
        }
        Event::Mouse(mouse) if app.preview.is_some() => match mouse.kind {
            MouseEventKind::ScrollUp => Some(Input::Up),
            MouseEventKind::ScrollDown => Some(Input::Down),
            _ => None,
        },
        Event::Mouse(mouse) if app.search.is_some() => match mouse.kind {
            MouseEventKind::ScrollUp => Some(Input::Up),
            MouseEventKind::ScrollDown => Some(Input::Down),
            MouseEventKind::Down(MouseButton::Left) => app.search_input_at(mouse.column, mouse.row),
            _ => None,
        },
        Event::Mouse(mouse)
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right))
                && !app.dialog_open() =>
        {
            app.tab_id_at(mouse.column, mouse.row)
                .map(|id| Input::OpenTabMenu {
                    id,
                    column: mouse.column,
                    row: mouse.row,
                })
        }
        Event::Mouse(mouse)
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
                && !app.dialog_open() =>
        {
            if app.tab_menu.is_some() {
                app.tab_menu_input_at(mouse.column, mouse.row)
                    .or(Some(Input::DismissTabMenu))
            } else {
                app.tab_input_at(mouse.column, mouse.row)
                    .or_else(|| app.content_input_at(mouse.column, mouse.row))
            }
        }
        _ => None,
    };
    let Some(input) = input else {
        return Ok(false);
    };

    let effect = reduce(app, input);
    match effect {
        Some(Effect::Quit) => return Ok(true),
        Some(Effect::CopyToClipboard(text)) => {
            clipboard.set_text(&text)?;
            app.status = format!("copied {} chars", text.chars().count());
        }
        Some(Effect::CopyAndPersist { text, mutation }) => {
            clipboard.set_text(&text)?;
            if persist_mutation(app, queue_path, &mutation)? {
                app.status = format!("copied {} chars", text.chars().count());
            }
        }
        Some(Effect::Persist(mutation)) => {
            if persist_mutation(app, queue_path, &mutation)? {
                app.status.clear();
            }
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
    Ok(false)
}

fn map_tab_menu_key(key: KeyEvent) -> Option<Input> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Input::Quit)
        }
        (KeyCode::Enter, _) => Some(Input::Enter),
        (KeyCode::Esc, _) => Some(Input::Esc),
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Input::Up),
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Input::Down),
        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Input> {
    let super_key = key.modifiers.contains(KeyModifiers::SUPER);
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Input::Quit)
        }
        (KeyCode::Char('/'), _) if super_key => Some(Input::OpenSearch),
        (KeyCode::Char('d'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Input::ForgetHistory)
        }
        (KeyCode::Enter, _) => Some(Input::Enter),
        (KeyCode::Esc, _) => Some(Input::Esc),
        (KeyCode::Up, _) => Some(Input::Up),
        (KeyCode::Down, _) => Some(Input::Down),
        (KeyCode::Backspace, _) => Some(Input::Backspace),
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => Some(Input::Char(c)),
        _ => None,
    }
}

fn map_preview_key(key: KeyEvent) -> Option<Input> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Input::Quit)
        }
        (KeyCode::Enter, _) => Some(Input::Enter),
        (KeyCode::Esc, _) => Some(Input::Esc),
        (KeyCode::Up, _) => Some(Input::Up),
        (KeyCode::Down, _) => Some(Input::Down),
        (KeyCode::PageUp, _) => Some(Input::PageUp),
        (KeyCode::PageDown, _) => Some(Input::PageDown),
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => Some(Input::Char(c)),
        _ => None,
    }
}

fn map_key(key: KeyEvent, focus: Pane, dialog_open: bool) -> Option<Input> {
    let modifiers = key.modifiers;
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let super_key = modifiers.contains(KeyModifiers::SUPER);

    if ctrl && key.code == KeyCode::Char('c') {
        return Some(Input::Quit);
    }
    if dialog_open {
        return match key.code {
            KeyCode::Enter => Some(Input::Enter),
            KeyCode::Esc => Some(Input::Esc),
            KeyCode::Backspace => Some(Input::Backspace),
            KeyCode::Char(c) if !ctrl && !alt && !super_key => Some(Input::Char(c)),
            _ => None,
        };
    }
    if super_key && key.code == KeyCode::Char('/') {
        return Some(Input::OpenSearch);
    }
    if ctrl && key.code == KeyCode::Char('t') {
        return Some(Input::OpenCreateTab);
    }
    if ctrl && key.code == KeyCode::Char('s') {
        return Some(Input::CtrlS);
    }
    match key.code {
        KeyCode::Tab if modifiers.is_empty() => return Some(Input::Tab),
        KeyCode::Esc => return Some(Input::Esc),
        _ => {}
    }

    match focus {
        Pane::Queue => match (key.code, modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Input::Quit),
            (KeyCode::Enter, _) => Some(Input::Enter),
            (KeyCode::Up, _) => Some(Input::Up),
            (KeyCode::Down, _) => Some(Input::Down),
            (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Input::Down),
            (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Input::Up),
            (KeyCode::Char('p'), KeyModifiers::NONE) => Some(Input::Char('p')),
            (KeyCode::Char('e'), KeyModifiers::NONE) => Some(Input::Char('e')),
            (KeyCode::Char('f'), KeyModifiers::NONE) => Some(Input::Char('f')),
            (KeyCode::Char('['), KeyModifiers::NONE) => Some(Input::PreviousTab),
            (KeyCode::Char(']'), KeyModifiers::NONE) => Some(Input::NextTab),
            (KeyCode::Char('r'), KeyModifiers::NONE) => Some(Input::OpenRenameTab),
            (KeyCode::Char('/'), KeyModifiers::NONE) => Some(Input::OpenSearch),
            (KeyCode::Left, _) => Some(Input::PreviousTab),
            (KeyCode::Right, _) => Some(Input::NextTab),
            _ => None,
        },
        Pane::Composer => match key.code {
            KeyCode::Enter if shift || alt => Some(Input::Newline),
            KeyCode::Enter => Some(Input::Enter),
            KeyCode::Backspace if super_key => Some(Input::DeleteToLineStart),
            KeyCode::Backspace if alt || ctrl => Some(Input::DeleteWordBack),
            KeyCode::Backspace => Some(Input::Backspace),
            KeyCode::Delete if alt || ctrl => Some(Input::DeleteWordForward),
            KeyCode::Delete => Some(Input::Delete),
            KeyCode::Home => Some(Input::MoveLineStart),
            KeyCode::End => Some(Input::MoveLineEnd),
            KeyCode::Left if super_key => Some(Input::MoveLineStart),
            KeyCode::Right if super_key => Some(Input::MoveLineEnd),
            KeyCode::Left if alt || ctrl => Some(Input::MoveWordLeft),
            KeyCode::Right if alt || ctrl => Some(Input::MoveWordRight),
            KeyCode::Left => Some(Input::MoveLeft),
            KeyCode::Right => Some(Input::MoveRight),
            KeyCode::Up => Some(Input::MoveUp),
            KeyCode::Down => Some(Input::MoveDown),
            KeyCode::Char('a') if ctrl => Some(Input::MoveLineStart),
            KeyCode::Char('e') if ctrl => Some(Input::MoveLineEnd),
            KeyCode::Char('u') if ctrl => Some(Input::DeleteToLineStart),
            KeyCode::Char('k') if ctrl => Some(Input::DeleteToLineEnd),
            KeyCode::Char('w') if ctrl => Some(Input::DeleteWordBack),
            KeyCode::Char('z') if (ctrl || super_key) && shift => Some(Input::Redo),
            KeyCode::Char('z') if ctrl || super_key => Some(Input::Undo),
            KeyCode::Char('y') if ctrl => Some(Input::Redo),
            KeyCode::Char(c) if !ctrl && !alt && !super_key => Some(Input::Char(c)),
            _ => None,
        },
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    restored: bool,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        if let Err(error) = execute!(
            out,
            Clear(ClearType::All),
            EnableBracketedPaste,
            EnableMouseCapture,
            PushKeyboardEnhancementFlags(KEYBOARD_ENHANCEMENTS)
        ) {
            let _ = execute!(
                out,
                PopKeyboardEnhancementFlags,
                DisableMouseCapture,
                DisableBracketedPaste,
                Clear(ClearType::All)
            );
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        let terminal = match Terminal::new(CrosstermBackend::new(out)) {
            Ok(terminal) => terminal,
            Err(error) => {
                let mut out = io::stdout();
                let _ = execute!(
                    out,
                    PopKeyboardEnhancementFlags,
                    DisableMouseCapture,
                    DisableBracketedPaste,
                    Clear(ClearType::All)
                );
                let _ = disable_raw_mode();
                return Err(error.into());
            }
        };
        let mut session = Self {
            terminal,
            restored: false,
        };
        session.terminal.clear()?;
        session.terminal.hide_cursor()?;
        Ok(session)
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    fn restore(&mut self) -> Result<()> {
        let modes = execute!(
            self.terminal.backend_mut(),
            PopKeyboardEnhancementFlags,
            DisableMouseCapture,
            DisableBracketedPaste,
            Clear(ClearType::All)
        );
        let raw_mode = disable_raw_mode();
        let cursor = self.terminal.show_cursor();
        modes?;
        raw_mode?;
        cursor?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if !self.restored {
            let _ = execute!(
                self.terminal.backend_mut(),
                PopKeyboardEnhancementFlags,
                DisableMouseCapture,
                DisableBracketedPaste,
                Clear(ClearType::All)
            );
            let _ = disable_raw_mode();
            let _ = self.terminal.show_cursor();
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime.rs"]
mod tests;
