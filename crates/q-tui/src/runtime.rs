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
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            map_key(key, app.focus, app.dialog_open())
        }
        Event::Paste(text)
            if app.focus == Pane::Composer && !app.dialog_open() && app.tab_menu.is_none() =>
        {
            Some(Input::Paste(text))
        }
        Event::Mouse(mouse)
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right))
                && !app.dialog_open()
                && app.preview.is_none() =>
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
                && !app.dialog_open()
                && app.preview.is_none() =>
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
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with_mods(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn map_key(key: KeyEvent, focus: Pane) -> Option<Input> {
        super::map_key(key, focus, false)
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
        for pane in [Pane::Queue, Pane::Composer] {
            assert_eq!(
                map_key(with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL), pane),
                Some(Input::Quit)
            );
        }
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
    fn modified_backspace_maps_to_editor_actions() {
        assert_eq!(
            map_key(
                with_mods(KeyCode::Backspace, KeyModifiers::SUPER),
                Pane::Composer
            ),
            Some(Input::DeleteToLineStart)
        );
        assert_eq!(
            map_key(
                with_mods(KeyCode::Backspace, KeyModifiers::ALT),
                Pane::Composer
            ),
            Some(Input::DeleteWordBack)
        );
        assert_eq!(
            map_key(
                with_mods(KeyCode::Char('u'), KeyModifiers::CONTROL),
                Pane::Composer
            ),
            Some(Input::DeleteToLineStart)
        );
    }

    #[test]
    fn composer_navigation_keys_map_to_editor_movements() {
        assert_eq!(
            map_key(key(KeyCode::Left), Pane::Composer),
            Some(Input::MoveLeft)
        );
        assert_eq!(
            map_key(with_mods(KeyCode::Left, KeyModifiers::ALT), Pane::Composer),
            Some(Input::MoveWordLeft)
        );
        assert_eq!(
            map_key(key(KeyCode::Home), Pane::Composer),
            Some(Input::MoveLineStart)
        );
        assert_eq!(
            map_key(key(KeyCode::Delete), Pane::Composer),
            Some(Input::Delete)
        );
    }

    #[test]
    fn shifted_or_alt_enter_inserts_newline() {
        assert_eq!(
            map_key(
                with_mods(KeyCode::Enter, KeyModifiers::SHIFT),
                Pane::Composer
            ),
            Some(Input::Newline)
        );
        assert_eq!(
            map_key(with_mods(KeyCode::Enter, KeyModifiers::ALT), Pane::Composer),
            Some(Input::Newline)
        );
    }

    #[test]
    fn mouse_click_selects_tab_and_opens_create_dialog() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let mut workspace = q_core::Workspace::new();
        let initial = workspace.first_tab_id();
        let target = workspace.create_tab("work").unwrap();
        let mut app = App::new(workspace);
        app.select_tab(initial);
        let mut clipboard = q_platform::clipboard::FakeClipboard::new();
        let area = ratatui::layout::Rect::new(2, 1, 6, 1);
        app.tab_hits = vec![crate::app::TabHit {
            area,
            target: crate::app::TabHitTarget::Tab(target),
        }];
        let click = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        });

        assert!(!handle_event(click, &mut app, &mut clipboard, &queue_path).unwrap());
        assert_eq!(app.active_tab_id, target);

        app.tab_hits[0].target = crate::app::TabHitTarget::Create;
        let click_create = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(!handle_event(click_create, &mut app, &mut clipboard, &queue_path).unwrap());
        assert!(app.tab_dialog.is_some());
    }

    #[test]
    fn left_click_focuses_prompt_and_composer() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let mut workspace = q_core::Workspace::new();
        let tab = workspace.first_tab_id();
        workspace
            .add_prompt(tab, q_core::Prompt::new("prompt").unwrap())
            .unwrap();
        let mut app = App::new(workspace);
        let mut clipboard = q_platform::clipboard::FakeClipboard::new();
        let prompt_area = ratatui::layout::Rect::new(0, 3, 20, 1);
        let composer_area = ratatui::layout::Rect::new(0, 10, 20, 1);
        app.prompt_hits = vec![crate::app::PromptHit {
            area: prompt_area,
            index: 0,
        }];
        app.composer_area = Some(composer_area);
        app.focus = Pane::Composer;

        let click_prompt = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: prompt_area.x,
            row: prompt_area.y,
            modifiers: KeyModifiers::NONE,
        });
        handle_event(click_prompt, &mut app, &mut clipboard, &queue_path).unwrap();
        assert_eq!(app.focus, Pane::Queue);
        assert_eq!(app.selected, Some(0));

        let click_composer = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: composer_area.x,
            row: composer_area.y,
            modifiers: KeyModifiers::NONE,
        });
        handle_event(click_composer, &mut app, &mut clipboard, &queue_path).unwrap();
        assert_eq!(app.focus, Pane::Composer);
    }

    #[test]
    fn right_click_tab_and_click_rename_opens_dialog() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let mut workspace = q_core::Workspace::new();
        let target = workspace.create_tab("work").unwrap();
        let mut app = App::new(workspace);
        let mut clipboard = q_platform::clipboard::FakeClipboard::new();
        let tab_area = ratatui::layout::Rect::new(2, 1, 6, 1);
        app.tab_hits = vec![crate::app::TabHit {
            area: tab_area,
            target: crate::app::TabHitTarget::Tab(target),
        }];
        let right_click = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: tab_area.x,
            row: tab_area.y,
            modifiers: KeyModifiers::NONE,
        });

        assert!(!handle_event(right_click, &mut app, &mut clipboard, &queue_path).unwrap());
        assert_eq!(app.tab_menu.as_ref().unwrap().tab_id, target);

        let rename_area = ratatui::layout::Rect::new(2, 2, 10, 1);
        app.tab_menu_hits = vec![crate::app::TabMenuHit {
            area: rename_area,
            action: crate::app::TabMenuAction::Rename,
        }];
        let left_click = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rename_area.x,
            row: rename_area.y,
            modifiers: KeyModifiers::NONE,
        });

        assert!(!handle_event(left_click, &mut app, &mut clipboard, &queue_path).unwrap());
        assert!(matches!(
            app.tab_dialog.as_ref().map(|dialog| dialog.mode),
            Some(crate::app::TabDialogMode::Rename(id)) if id == target
        ));
    }

    #[test]
    fn tab_menu_keys_select_and_activate_actions() {
        assert_eq!(map_tab_menu_key(key(KeyCode::Up)), Some(Input::Up));
        assert_eq!(map_tab_menu_key(key(KeyCode::Down)), Some(Input::Down));
        assert_eq!(map_tab_menu_key(key(KeyCode::Enter)), Some(Input::Enter));
        assert_eq!(map_tab_menu_key(key(KeyCode::Esc)), Some(Input::Esc));
    }

    #[test]
    fn tab_shortcuts_map_in_queue_pane() {
        assert_eq!(
            map_key(key(KeyCode::Char('[')), Pane::Queue),
            Some(Input::PreviousTab)
        );
        assert_eq!(
            map_key(key(KeyCode::Char(']')), Pane::Queue),
            Some(Input::NextTab)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('r')), Pane::Queue),
            Some(Input::OpenRenameTab)
        );
        assert_eq!(
            map_key(
                with_mods(KeyCode::Char('t'), KeyModifiers::CONTROL),
                Pane::Composer
            ),
            Some(Input::OpenCreateTab)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('[')), Pane::Composer),
            Some(Input::Char('['))
        );
    }

    #[test]
    fn p_and_e_map_to_queue_actions() {
        assert_eq!(
            map_key(key(KeyCode::Char('p')), Pane::Queue),
            Some(Input::Char('p'))
        );
        assert_eq!(
            map_key(key(KeyCode::Char('e')), Pane::Queue),
            Some(Input::Char('e'))
        );
        assert_eq!(
            map_key(key(KeyCode::Char('f')), Pane::Queue),
            Some(Input::Char('f'))
        );
        assert_eq!(
            map_key(key(KeyCode::Char('f')), Pane::Composer),
            Some(Input::Char('f'))
        );
    }

    #[test]
    fn preview_keys_map_to_scrolling_and_closing() {
        assert_eq!(map_preview_key(key(KeyCode::Up)), Some(Input::Up));
        assert_eq!(map_preview_key(key(KeyCode::Down)), Some(Input::Down));
        assert_eq!(map_preview_key(key(KeyCode::PageUp)), Some(Input::PageUp));
        assert_eq!(
            map_preview_key(key(KeyCode::PageDown)),
            Some(Input::PageDown)
        );
        assert_eq!(
            map_preview_key(with_mods(KeyCode::Char('G'), KeyModifiers::SHIFT)),
            Some(Input::Char('G'))
        );
        assert_eq!(
            map_preview_key(key(KeyCode::Char('f'))),
            Some(Input::Char('f'))
        );
        assert_eq!(map_preview_key(key(KeyCode::Esc)), Some(Input::Esc));
        assert_eq!(
            map_preview_key(with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Input::Quit)
        );
    }

    #[test]
    fn clicks_are_ignored_while_the_preview_is_open() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let mut workspace = q_core::Workspace::new();
        let tab = workspace.first_tab_id();
        workspace
            .add_prompt(tab, q_core::Prompt::new("prompt").unwrap())
            .unwrap();
        let mut app = App::new(workspace);
        let mut clipboard = q_platform::clipboard::FakeClipboard::new();
        let composer_area = ratatui::layout::Rect::new(0, 10, 20, 1);
        app.composer_area = Some(composer_area);
        app.preview = Some(crate::app::PromptPreview {
            id: app.visible_prompts()[0].id,
            scroll: 0,
        });

        let click = Event::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: composer_area.x,
            row: composer_area.y,
            modifiers: KeyModifiers::NONE,
        });
        handle_event(click, &mut app, &mut clipboard, &queue_path).unwrap();

        assert_eq!(app.focus, Pane::Queue);
        assert!(app.preview.is_some());
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
    }

    #[test]
    fn printable_chars_require_no_command_modifiers() {
        assert_eq!(
            map_key(key(KeyCode::Char('x')), Pane::Composer),
            Some(Input::Char('x'))
        );
        assert_eq!(
            map_key(
                with_mods(KeyCode::Char('x'), KeyModifiers::ALT),
                Pane::Composer
            ),
            None
        );
    }

    #[test]
    fn keyboard_mode_reports_modifiers_for_backspace() {
        assert!(KEYBOARD_ENHANCEMENTS
            .contains(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES));
    }

    #[test]
    fn blink_timeout_targets_state_transitions() {
        assert_eq!(
            blink_timeout(Duration::from_millis(0)),
            Duration::from_millis(650)
        );
        assert_eq!(
            blink_timeout(Duration::from_millis(700)),
            Duration::from_millis(300)
        );
        assert_eq!(
            blink_timeout(Duration::from_millis(1200)),
            Duration::from_millis(450)
        );
    }

    #[test]
    fn concurrent_add_transactions_preserve_both_prompts() {
        use std::sync::{Arc, Barrier};

        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();

        for text in ["from-a", "from-b"] {
            let path = queue_path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let prompt = q_core::Prompt::new(text).unwrap();
                let tab_id = q_core::Workspace::new().first_tab_id();
                barrier.wait();
                commit_mutation(&path, &QueueMutation::Add { tab_id, prompt }).unwrap();
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().unwrap();
        }

        let workspace = q_core::storage::load(&queue_path).unwrap();
        let texts: Vec<_> = workspace.tabs()[0]
            .queue()
            .iter()
            .map(|prompt| prompt.text.as_str())
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(texts.contains(&"from-a"));
        assert!(texts.contains(&"from-b"));
    }

    #[test]
    fn close_tab_mutation_is_persisted() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let mut workspace = q_core::Workspace::new();
        let closed = workspace.create_tab("closed").unwrap();
        q_core::storage::save(&queue_path, &workspace).unwrap();

        let outcome = commit_mutation(&queue_path, &QueueMutation::CloseTab(closed)).unwrap();

        assert!(matches!(outcome, MutationOutcome::Committed(_)));
        let persisted = q_core::storage::load(&queue_path).unwrap();
        assert!(persisted.tab(closed).is_none());
        assert_eq!(persisted.tabs().len(), 1);
    }

    #[test]
    fn stale_mutation_returns_latest_queue_as_conflict() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let prompt = q_core::Prompt::new("remove me").unwrap();
        let id = prompt.id;

        let tab_id = q_core::Workspace::new().first_tab_id();
        commit_mutation(&queue_path, &QueueMutation::Add { tab_id, prompt }).unwrap();
        commit_mutation(&queue_path, &QueueMutation::Remove(id)).unwrap();
        let outcome = commit_mutation(&queue_path, &QueueMutation::Remove(id)).unwrap();

        assert!(matches!(outcome, MutationOutcome::Rejected(_, _)));
        assert!(q_core::storage::load(&queue_path)
            .unwrap()
            .get_prompt(id)
            .is_none());
    }

    #[test]
    fn external_refresh_preserves_composer_state() {
        let dir = tempfile::TempDir::new().unwrap();
        let queue_path = dir.path().join("queue.json");
        let mut app = App::new(q_core::Queue::new());
        app.composer.set_text("local draft");
        app.focus = Pane::Composer;
        let mut sync = QueueSync::new(&queue_path);

        let mut external = q_core::Workspace::new();
        let tab_id = external.first_tab_id();
        external
            .add_prompt(tab_id, q_core::Prompt::new("external prompt").unwrap())
            .unwrap();
        q_core::storage::save(&queue_path, &external).unwrap();
        sync.last_check = Instant::now() - SYNC_INTERVAL;
        sync.refresh_if_due(&mut app, &queue_path);

        assert_eq!(app.visible_prompts().len(), 1);
        assert_eq!(app.composer.text(), "local draft");
        assert_eq!(app.focus, Pane::Composer);
    }
}
