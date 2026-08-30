use crate::app::{App, EditorOrigin, Effect, QueueMutation};
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
        .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES)
        .union(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS);

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
        let forced_reload = self.last_reload.elapsed() >= FULL_RELOAD_INTERVAL;
        // Resolve live references at the sync cadence, never while drawing.
        // Periodically bypass metadata equality so coarse timestamps cannot
        // leave same-size external rewrites stale indefinitely.
        if forced_reload {
            app.refresh_external_content_forced();
        } else {
            app.refresh_external_content();
        }

        let fingerprint = match q_core::storage::fingerprint(queue_path) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                app.status = format!("sync failed: {error}");
                return;
            }
        };
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
    let document_lock_path = match mutation {
        QueueMutation::Remove {
            expected_source: q_core::PromptSource::ExternalMarkdown { path },
            expected_external_content: Some(_),
            ..
        } => Some(q_platform::external_document::document_lock_path(path)?),
        _ => None,
    };
    let mut document_lock = match document_lock_path {
        Some(path) => Some(FileLock::open(&path)?),
        None => None,
    };
    let _document_guard = match document_lock.as_mut() {
        Some(lock) => Some(lock.write()?),
        None => None,
    };
    let mut lock = FileLock::open(&queue_path.with_extension("lock"))?;
    let _guard = lock.write()?;
    let mut workspace = q_core::storage::load(queue_path)?;

    let result = match mutation {
        QueueMutation::Add { tab_id, prompt } => {
            workspace.add_prompt(*tab_id, prompt.clone()).map(|_| ())
        }
        QueueMutation::Remove {
            id,
            expected_source,
            expected_pinned,
            expected_external_content,
        } => verify_prompt_state(&workspace, *id, expected_source, *expected_pinned)
            .and_then(|()| verify_external_content(expected_source, expected_external_content))
            .and_then(|()| workspace.remove_prompt(*id).map(|_| ())),
        QueueMutation::EditInline {
            id,
            expected_source,
            expected_pinned,
            text,
        } => verify_prompt_state(&workspace, *id, expected_source, *expected_pinned)
            .and_then(|()| workspace.edit_prompt_inline(*id, text.clone())),
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
        QueueMutation::ForgetHistory(source) => {
            workspace.forget_history(source);
            Ok(())
        }
    };
    if let Err(error) = result {
        return Ok(MutationOutcome::Rejected(workspace, error.to_string()));
    }

    q_core::storage::save(queue_path, &workspace)?;
    Ok(MutationOutcome::Committed(workspace))
}

fn verify_prompt_state(
    workspace: &q_core::Workspace,
    id: q_core::PromptId,
    expected_source: &q_core::PromptSource,
    expected_pinned: bool,
) -> q_core::Result<()> {
    let prompt = workspace
        .get_prompt(id)
        .ok_or_else(|| q_core::CoreError::NotFound(id.to_string()))?;
    if prompt.source() != expected_source || prompt.pinned != expected_pinned {
        return Err(q_core::CoreError::Invalid(
            "prompt changed in another window; retry the operation".to_string(),
        ));
    }
    Ok(())
}

fn verify_external_content(
    source: &q_core::PromptSource,
    expected_content: &Option<String>,
) -> q_core::Result<()> {
    let Some(expected_content) = expected_content else {
        return Ok(());
    };
    let Some(path) = source.external_markdown_path() else {
        return Err(q_core::CoreError::Invalid(
            "external content precondition requires an external source".to_string(),
        ));
    };
    let current = q_platform::external_document::read_utf8(path)
        .map_err(|error| q_core::CoreError::Invalid(error.to_string()))?;
    if &current != expected_content {
        return Err(q_core::CoreError::Invalid(
            "external document changed after it was copied; reference was not removed".to_string(),
        ));
    }
    Ok(())
}

fn persist_mutation(app: &mut App, queue_path: &Path, mutation: &QueueMutation) -> Result<bool> {
    match commit_mutation(queue_path, mutation)? {
        MutationOutcome::Committed(workspace) => {
            let close_editor = match mutation {
                QueueMutation::EditInline { id, .. } => {
                    app.editor.as_ref().and_then(|editor| editor.inline_id()) == Some(*id)
                }
                _ => false,
            };
            app.replace_workspace(workspace);
            if close_editor {
                app.editor = None;
            }
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
                QueueMutation::EditInline { id, .. }
                    if app.editor.as_ref().and_then(|editor| editor.inline_id()) == Some(*id) =>
                {
                    if let Some(editor) = app.editor.as_mut() {
                        editor.error = error.clone();
                    }
                }
                _ => {}
            }
            app.status = error;
            Ok(false)
        }
    }
}

fn save_external_editor(app: &mut App, _queue_path: &Path) -> bool {
    let result: Result<()> = (|| {
        let source_path = match &app
            .editor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("editor is no longer open"))?
            .origin
        {
            EditorOrigin::External { document } => document.path.as_path(),
            EditorOrigin::Inline { .. } => {
                return Err(anyhow::anyhow!("inline editor has no external document"));
            }
        };
        let lock_path = q_platform::external_document::document_lock_path(source_path)?;
        let mut lock = FileLock::open(&lock_path)?;
        let _guard = lock.write()?;
        let editor = app
            .editor
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("editor is no longer open"))?;
        let text = editor.buffer.text();
        match &mut editor.origin {
            EditorOrigin::External { document } => document.save(&text)?,
            EditorOrigin::Inline { .. } => {
                return Err(anyhow::anyhow!("inline editor has no external document"));
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            app.editor = None;
            app.refresh_external_content_forced();
            app.status.clear();
            true
        }
        Err(error) => {
            let message = format!("save failed: {error}");
            if let Some(editor) = app.editor.as_mut() {
                editor.error = message.clone();
            }
            app.status = message;
            false
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
                && app.editor.is_some() =>
        {
            map_editor_key(key)
        }
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
        Event::Paste(text) if app.editor.is_some() => Some(Input::Paste(text)),
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
        Some(Effect::CopyToClipboard(text)) => match clipboard.set_text(&text) {
            Ok(()) => app.status = format!("copied {} chars", text.chars().count()),
            Err(error) => app.status = format!("clipboard failed: {error}"),
        },
        Some(Effect::CopyAndPersist { text, mutation }) => match clipboard.set_text(&text) {
            Ok(()) => match persist_mutation(app, queue_path, &mutation) {
                Ok(true) => app.status = format!("copied {} chars", text.chars().count()),
                Ok(false) => {}
                Err(error) => {
                    app.status = format!("copied, but queue removal failed: {error}");
                }
            },
            Err(error) => app.status = format!("clipboard failed: {error}"),
        },
        Some(Effect::Persist(mutation)) => match persist_mutation(app, queue_path, &mutation) {
            Ok(true) => app.status.clear(),
            Ok(false) => {}
            Err(error) if matches!(mutation, QueueMutation::EditInline { .. }) => {
                let message = format!("save failed: {error}");
                if let Some(editor) = app.editor.as_mut() {
                    editor.error = message.clone();
                }
                app.status = message;
            }
            Err(error) => return Err(error),
        },
        Some(Effect::SaveExternal) => {
            save_external_editor(app, queue_path);
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

fn map_editor_key(key: KeyEvent) -> Option<Input> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Input::Esc);
    }
    // Reuse all composer movement, undo/redo, deletion, and paste bindings.
    // The editor reducer interprets Enter as a newline.
    map_key(key, Pane::Composer, false)
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

fn shifted_char(c: char) -> char {
    let mut uppercase = c.to_uppercase();
    match (uppercase.next(), uppercase.next()) {
        (Some(c), None) => c,
        _ => c,
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
            (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Input::Char('d')),
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
            KeyCode::Char(c) if !ctrl && !alt && !super_key => {
                Some(Input::Char(if shift { shifted_char(c) } else { c }))
            }
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
