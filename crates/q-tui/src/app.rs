use chrono::{DateTime, Utc};
use q_core::{HistoryEntry, Prompt, PromptId, PromptSource, TabId, Workspace};
use q_platform::external_document::{self, DocumentFingerprint, EditorDocument};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
};
use ratatui_textarea::{CursorMove, TextArea, WrapMode};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Queue,
    Composer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Char(char),
    Paste(String),
    Enter,
    Newline,
    Backspace,
    Delete,
    DeleteWordBack,
    DeleteWordForward,
    DeleteToLineStart,
    DeleteToLineEnd,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordLeft,
    MoveWordRight,
    MoveLineStart,
    MoveLineEnd,
    Undo,
    Redo,
    Tab,
    Up,
    Down,
    PageUp,
    PageDown,
    PreviousTab,
    NextTab,
    SelectTab(TabId),
    SelectPrompt(usize),
    SelectHistory(usize),
    OpenSearch,
    OpenMenu,
    ForgetHistory,
    FocusComposer,
    OpenCreateTab,
    OpenRenameTab,
    OpenTabMenu { id: TabId, column: u16, row: u16 },
    SelectTabMenuAction(TabMenuAction),
    DismissTabMenu,
    CtrlS,
    Esc,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueMutation {
    Add {
        tab_id: TabId,
        prompt: Prompt,
    },
    Remove {
        id: PromptId,
        expected_source: PromptSource,
        expected_pinned: bool,
        expected_external_content: Option<String>,
    },
    EditInline {
        id: PromptId,
        expected_source: PromptSource,
        expected_pinned: bool,
        text: String,
    },
    SetPinned {
        id: PromptId,
        pinned: bool,
    },
    CreateTab {
        id: TabId,
        name: String,
        activity_at: DateTime<Utc>,
    },
    RenameTab {
        id: TabId,
        name: String,
    },
    CloseTab(TabId),
    ForgetHistory(PromptSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    CopyToClipboard(String),
    CopyAndPersist {
        text: String,
        mutation: QueueMutation,
    },
    Persist(QueueMutation),
    SaveExternal,
    Quit,
    Status(String),
    OpenWorkspacesOverlay,
    SwitchWorkspace(PathBuf),
    CreateWorkspace(String),
    RenameWorkspace {
        dir: PathBuf,
        name: String,
    },
    DeleteWorkspace(PathBuf),
    RefreshGithubStatus,
    GithubConnect,
    GithubDisconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Workspaces,
    Settings,
}

/// GitHub connection state shown in Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubAuthState {
    /// Status has not been checked yet.
    Unknown,
    /// No usable token found.
    NotConnected,
    /// Checking the token or waiting for the device flow to start.
    Checking,
    /// Device flow is running; the user must enter the code.
    Connecting {
        user_code: String,
        verification_uri: String,
    },
    Connected {
        login: String,
        /// True when the token is borrowed from the `gh` CLI.
        gh_cli: bool,
    },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    pub dir: PathBuf,
    pub name: String,
    /// Whether this window is currently on this workspace.
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspacesMode {
    List,
    Create { value: String },
    Info(WorkspaceInfo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoAction {
    Rename,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoMode {
    View,
    Rename { value: String },
    ConfirmDelete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo {
    pub action: InfoAction,
    pub mode: InfoMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacesOverlay {
    pub entries: Vec<WorkspaceEntry>,
    pub selected: usize,
    pub mode: WorkspacesMode,
    pub error: String,
}

impl WorkspacesOverlay {
    pub fn selected_entry(&self) -> Option<&WorkspaceEntry> {
        self.entries.get(self.selected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuState {
    Root { selected: MenuItem },
    Workspaces(WorkspacesOverlay),
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDialogMode {
    Create,
    Rename(TabId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabMenuAction {
    Rename,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabContextMenu {
    pub tab_id: TabId,
    pub column: u16,
    pub row: u16,
    pub selected: TabMenuAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSource {
    Prompt(PromptId),
    History(PromptSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPreview {
    pub source: PreviewSource,
    pub scroll: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchDialog {
    pub query: String,
    pub selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseTabDialog {
    pub tab_id: TabId,
    pub tab_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletePromptDialog {
    pub prompt_id: PromptId,
    pub expected_source: PromptSource,
    pub expected_pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabDialog {
    pub mode: TabDialogMode,
    pub value: String,
    pub error: String,
    replace_on_type: bool,
}

impl TabDialog {
    pub fn create() -> Self {
        Self {
            mode: TabDialogMode::Create,
            value: String::new(),
            error: String::new(),
            replace_on_type: false,
        }
    }

    pub fn rename(id: TabId, name: &str) -> Self {
        Self {
            mode: TabDialogMode::Rename(id),
            value: name.to_string(),
            error: String::new(),
            replace_on_type: true,
        }
    }

    pub(crate) fn insert_char(&mut self, c: char) {
        if self.replace_on_type {
            self.value.clear();
            self.replace_on_type = false;
        }
        self.value.push(c);
        self.error.clear();
    }

    pub(crate) fn backspace(&mut self) {
        if self.replace_on_type {
            self.value.clear();
            self.replace_on_type = false;
        } else {
            self.value.pop();
        }
        self.error.clear();
    }
}

pub struct ComposerEditor {
    textarea: TextArea<'static>,
}

impl Default for ComposerEditor {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl ComposerEditor {
    pub fn from_text(text: &str) -> Self {
        let mut textarea = TextArea::from(text.split('\n'));
        textarea.set_cursor_line_style(Style::default());
        textarea.set_wrap_mode(WrapMode::WordOrGlyph);
        textarea.move_cursor(CursorMove::Bottom);
        textarea.move_cursor(CursorMove::End);
        Self { textarea }
    }

    pub fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.textarea.is_empty()
    }

    pub(crate) fn lines(&self) -> &[String] {
        self.textarea.lines()
    }

    pub fn cursor(&self) -> (usize, usize) {
        let cursor = self.textarea.cursor();
        (cursor.0, cursor.1)
    }

    pub fn set_text(&mut self, text: &str) {
        *self = Self::from_text(text);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn insert_char(&mut self, c: char) {
        self.textarea.insert_char(c);
    }

    pub fn insert_str(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.textarea.insert_str(&text);
    }

    pub fn insert_newline(&mut self) {
        self.textarea.insert_newline();
    }

    pub fn delete_char(&mut self) {
        self.textarea.delete_char();
    }

    pub fn delete_next_char(&mut self) {
        self.textarea.delete_next_char();
    }

    pub fn delete_word(&mut self) {
        self.textarea.delete_word();
    }

    pub fn delete_next_word(&mut self) {
        self.textarea.delete_next_word();
    }

    pub fn delete_to_line_start(&mut self) {
        self.textarea.delete_line_by_head();
    }

    pub fn delete_to_line_end(&mut self) {
        self.textarea.delete_line_by_end();
    }

    pub fn move_cursor(&mut self, movement: CursorMove) {
        self.textarea.move_cursor(movement);
    }

    pub fn undo(&mut self) {
        self.textarea.undo();
    }

    pub fn redo(&mut self) {
        self.textarea.redo();
    }

    pub(crate) fn set_cursor_visible(&mut self, visible: bool) {
        let style = if visible {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        self.textarea.set_cursor_style(style);
    }

    pub(crate) fn widget(&self) -> &TextArea<'static> {
        &self.textarea
    }
}

pub enum EditorOrigin {
    Inline {
        id: PromptId,
        expected_source: PromptSource,
        expected_pinned: bool,
    },
    External {
        document: EditorDocument,
    },
}

pub struct FullScreenEditor {
    pub origin: EditorOrigin,
    pub buffer: ComposerEditor,
    original_text: String,
    pub error: String,
    pub discard_confirmation: bool,
}

impl FullScreenEditor {
    fn inline(id: PromptId, text: &str, expected_pinned: bool) -> Self {
        Self {
            origin: EditorOrigin::Inline {
                id,
                expected_source: PromptSource::Inline {
                    text: text.to_string(),
                },
                expected_pinned,
            },
            buffer: ComposerEditor::from_text(text),
            original_text: text.to_string(),
            error: String::new(),
            discard_confirmation: false,
        }
    }

    fn external(document: EditorDocument) -> Self {
        let text = document.text.clone();
        Self {
            origin: EditorOrigin::External { document },
            buffer: ComposerEditor::from_text(&text),
            original_text: text,
            error: String::new(),
            discard_confirmation: false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.buffer.text() != self.original_text
    }

    pub fn inline_id(&self) -> Option<PromptId> {
        match self.origin {
            EditorOrigin::Inline { id, .. } => Some(id),
            EditorOrigin::External { .. } => None,
        }
    }

    pub fn expected_inline_state(&self) -> Option<(&PromptSource, bool)> {
        match &self.origin {
            EditorOrigin::Inline {
                expected_source,
                expected_pinned,
                ..
            } => Some((expected_source, *expected_pinned)),
            EditorOrigin::External { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabHitTarget {
    Tab(TabId),
    Create,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TabHit {
    pub area: Rect,
    pub target: TabHitTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TabMenuHit {
    pub area: Rect,
    pub action: TabMenuAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PromptHit {
    pub area: Rect,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SearchHit {
    pub area: Rect,
    pub index: usize,
}

struct CachedExternal {
    fingerprint: Result<DocumentFingerprint, String>,
    content: Result<String, String>,
}

pub struct App {
    pub workspace: Workspace,
    pub active_tab_id: TabId,
    pub focus: Pane,
    pub selected: Option<usize>,
    pub composer: ComposerEditor,
    pub tab_dialog: Option<TabDialog>,
    pub tab_menu: Option<TabContextMenu>,
    pub close_tab_dialog: Option<CloseTabDialog>,
    pub delete_prompt_dialog: Option<DeletePromptDialog>,
    pub preview: Option<PromptPreview>,
    pub search: Option<SearchDialog>,
    pub menu: Option<MenuState>,
    pub github: GithubAuthState,
    /// GitHub login used for attribution on new prompts, tabs, and edits.
    pub identity: Option<String>,
    pub editor: Option<FullScreenEditor>,
    pub status: String,
    pub(crate) tab_hits: Vec<TabHit>,
    pub(crate) tab_menu_hits: Vec<TabMenuHit>,
    pub(crate) prompt_hits: Vec<PromptHit>,
    pub(crate) search_hits: Vec<SearchHit>,
    pub(crate) search_folds: Vec<q_core::search::Folded>,
    pub(crate) composer_area: Option<Rect>,
    pub(crate) preview_page: u16,
    pub(crate) preview_max_scroll: u16,
    external_cache: HashMap<PathBuf, CachedExternal>,
}

impl App {
    pub fn new(workspace: impl Into<Workspace>) -> Self {
        let workspace = workspace.into();
        let active_tab_id = workspace.first_tab_id();
        let empty = workspace
            .tab(active_tab_id)
            .map(|tab| tab.queue().is_empty())
            .unwrap_or(true);
        let mut app = Self {
            active_tab_id,
            focus: if empty { Pane::Composer } else { Pane::Queue },
            selected: if empty { None } else { Some(0) },
            workspace,
            composer: ComposerEditor::default(),
            tab_dialog: None,
            tab_menu: None,
            close_tab_dialog: None,
            delete_prompt_dialog: None,
            preview: None,
            search: None,
            menu: None,
            github: GithubAuthState::Unknown,
            identity: None,
            editor: None,
            status: String::new(),
            tab_hits: Vec::new(),
            tab_menu_hits: Vec::new(),
            prompt_hits: Vec::new(),
            search_hits: Vec::new(),
            search_folds: Vec::new(),
            composer_area: None,
            preview_page: 1,
            preview_max_scroll: 0,
            external_cache: HashMap::new(),
        };
        app.refresh_external_content();
        app
    }

    pub fn visible_prompts(&self) -> Vec<&Prompt> {
        self.workspace
            .tab(self.active_tab_id)
            .map(|tab| tab.queue().iter().collect())
            .unwrap_or_default()
    }

    pub fn selected_prompt(&self) -> Option<&Prompt> {
        self.selected
            .and_then(|index| self.visible_prompts().into_iter().nth(index))
    }

    pub fn resolve_source(&self, source: &PromptSource) -> Result<String, String> {
        match source {
            PromptSource::Inline { text } => Ok(text.clone()),
            PromptSource::ExternalMarkdown { path } => self
                .external_cache
                .get(path)
                .map(|cached| cached.content.clone())
                .unwrap_or_else(|| Err("external document has not been loaded".to_string())),
        }
    }

    pub fn resolve_source_owned(&self, source: &PromptSource) -> Result<String, String> {
        match source {
            PromptSource::Inline { text } => Ok(text.clone()),
            PromptSource::ExternalMarkdown { path } => {
                external_document::read_utf8(path).map_err(|error| error.to_string())
            }
        }
    }

    pub fn source_card_text(&self, source: &PromptSource) -> String {
        self.resolve_source(source)
            .unwrap_or_else(|error| error.to_string())
    }

    pub fn refresh_external_content(&mut self) {
        self.refresh_external_content_inner(false);
    }

    pub fn refresh_external_content_forced(&mut self) {
        self.refresh_external_content_inner(true);
    }

    fn refresh_external_content_inner(&mut self, force: bool) {
        let paths: HashSet<PathBuf> = self
            .workspace
            .tabs()
            .iter()
            .flat_map(|tab| tab.queue().iter())
            .filter_map(|prompt| prompt.external_markdown_path().map(Path::to_path_buf))
            .chain(
                self.workspace
                    .history()
                    .iter()
                    .filter_map(|entry| entry.external_markdown_path().map(Path::to_path_buf)),
            )
            .collect();
        self.external_cache.retain(|path, _| paths.contains(path));
        let mut changed = false;
        for path in paths {
            let fingerprint =
                external_document::fingerprint(&path).map_err(|error| error.to_string());
            let unchanged = !force
                && self.external_cache.get(&path).is_some_and(|cached| {
                    cached.content.is_ok() && cached.fingerprint == fingerprint
                });
            if unchanged {
                continue;
            }
            let content = external_document::read_utf8(&path).map_err(|error| error.to_string());
            self.external_cache.insert(
                path,
                CachedExternal {
                    fingerprint,
                    content,
                },
            );
            changed = true;
        }
        if changed {
            // Live external text participates in history search.
            self.search_folds.clear();
        }
    }

    fn searchable_text(&self, source: &PromptSource) -> String {
        match source {
            PromptSource::Inline { text } => text.clone(),
            PromptSource::ExternalMarkdown { path } => {
                let content = self.resolve_source(source).unwrap_or_default();
                format!("{}\n{content}", path.display())
            }
        }
    }

    pub fn search_results(&self) -> Vec<&HistoryEntry> {
        let Some(search) = &self.search else {
            return Vec::new();
        };
        let query = q_core::search::Query::new(&search.query);
        let history = self.workspace.history();
        let cached = self.search_folds.len() == history.len();
        history
            .iter()
            .enumerate()
            .filter(|(index, entry)| {
                if cached {
                    query.is_match_folded(&self.search_folds[*index])
                } else {
                    query.is_match(&self.searchable_text(entry.source()))
                }
            })
            .map(|(_, entry)| entry)
            .collect()
    }

    pub(crate) fn refresh_search_folds(&mut self) {
        if self.search_folds.len() == self.workspace.history().len() {
            return;
        }
        self.search_folds = self
            .workspace
            .history()
            .iter()
            .map(|entry| q_core::search::folded(&self.searchable_text(entry.source())))
            .collect();
    }

    pub(crate) fn preview_source(&self) -> Option<&PromptSource> {
        match &self.preview.as_ref()?.source {
            PreviewSource::Prompt(id) => self.workspace.get_prompt(*id).map(Prompt::source),
            PreviewSource::History(source) => Some(source),
        }
    }

    pub(crate) fn preview_text(&self) -> Result<String, String> {
        let source = self
            .preview_source()
            .ok_or_else(|| "prompt is no longer available".to_string())?;
        self.resolve_source(source)
    }

    pub(crate) fn preview_live_text(&self) -> Result<String, String> {
        let source = self
            .preview_source()
            .ok_or_else(|| "prompt is no longer available".to_string())?;
        self.resolve_source_owned(source)
    }

    pub(crate) fn preview_title(&self) -> String {
        self.preview_source()
            .and_then(PromptSource::external_markdown_path)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Prompt".to_string())
    }

    pub(crate) fn open_editor_for_source(
        &mut self,
        source: PromptSource,
        inline_id: Option<PromptId>,
    ) -> Result<(), String> {
        match source {
            PromptSource::Inline { text } => {
                let id = inline_id.ok_or_else(|| {
                    "history-only inline prompts cannot be edited in place".to_string()
                })?;
                let expected_pinned = self
                    .workspace
                    .get_prompt(id)
                    .ok_or_else(|| "prompt is no longer available".to_string())?
                    .pinned();
                self.editor = Some(FullScreenEditor::inline(id, &text, expected_pinned));
            }
            PromptSource::ExternalMarkdown { path } => {
                let document = EditorDocument::load(&path).map_err(|error| error.to_string())?;
                self.editor = Some(FullScreenEditor::external(document));
            }
        }
        Ok(())
    }

    pub fn select_tab(&mut self, id: TabId) {
        if self.workspace.tab(id).is_none() {
            return;
        }
        self.active_tab_id = id;
        let empty = self.visible_prompts().is_empty();
        self.selected = if empty { None } else { Some(0) };
        if empty {
            self.focus = Pane::Composer;
        }
    }

    pub fn replace_workspace(&mut self, workspace: Workspace) {
        let selected_id = self.selected_prompt().map(|prompt| prompt.id);
        let previous_index = self.selected;
        let active_tab_id = self.active_tab_id;
        self.workspace = workspace;
        self.search_folds.clear();
        self.active_tab_id = self
            .workspace
            .tab(active_tab_id)
            .map(|_| active_tab_id)
            .unwrap_or_else(|| self.workspace.first_tab_id());

        if matches!(
            self.tab_dialog.as_ref().map(|dialog| dialog.mode),
            Some(TabDialogMode::Rename(id)) if self.workspace.tab(id).is_none()
        ) {
            self.tab_dialog = None;
        }
        if self
            .tab_menu
            .as_ref()
            .is_some_and(|menu| self.workspace.tab(menu.tab_id).is_none())
        {
            self.tab_menu = None;
        }
        if self
            .close_tab_dialog
            .as_ref()
            .is_some_and(|dialog| self.workspace.tab(dialog.tab_id).is_none())
        {
            self.close_tab_dialog = None;
        }
        if self.preview.as_ref().is_some_and(|preview| {
            matches!(&preview.source, PreviewSource::Prompt(id) if self.workspace.get_prompt(*id).is_none())
        }) {
            self.preview = None;
        }

        self.selected = selected_id
            .and_then(|id| {
                self.visible_prompts()
                    .iter()
                    .position(|prompt| prompt.id == id)
            })
            .or_else(|| {
                let len = self.visible_prompts().len();
                (len > 0).then(|| previous_index.unwrap_or(0).min(len - 1))
            });
        self.refresh_external_content();
    }

    pub(crate) fn tab_input_at(&self, column: u16, row: u16) -> Option<Input> {
        self.tab_hits
            .iter()
            .find(|hit| {
                hit.area
                    .contains(ratatui::layout::Position::new(column, row))
            })
            .map(|hit| match hit.target {
                TabHitTarget::Tab(id) => Input::SelectTab(id),
                TabHitTarget::Create => Input::OpenCreateTab,
            })
    }

    pub(crate) fn tab_id_at(&self, column: u16, row: u16) -> Option<TabId> {
        self.tab_hits.iter().find_map(|hit| {
            let contains = hit
                .area
                .contains(ratatui::layout::Position::new(column, row));
            match (contains, hit.target) {
                (true, TabHitTarget::Tab(id)) => Some(id),
                _ => None,
            }
        })
    }

    pub(crate) fn content_input_at(&self, column: u16, row: u16) -> Option<Input> {
        let position = ratatui::layout::Position::new(column, row);
        if let Some(hit) = self
            .prompt_hits
            .iter()
            .find(|hit| hit.area.contains(position))
        {
            return Some(Input::SelectPrompt(hit.index));
        }
        self.composer_area
            .filter(|area| area.contains(position))
            .map(|_| Input::FocusComposer)
    }

    pub(crate) fn search_input_at(&self, column: u16, row: u16) -> Option<Input> {
        self.search_hits
            .iter()
            .find(|hit| {
                hit.area
                    .contains(ratatui::layout::Position::new(column, row))
            })
            .map(|hit| Input::SelectHistory(hit.index))
    }

    pub(crate) fn tab_menu_input_at(&self, column: u16, row: u16) -> Option<Input> {
        self.tab_menu_hits
            .iter()
            .find(|hit| {
                hit.area
                    .contains(ratatui::layout::Position::new(column, row))
            })
            .map(|hit| Input::SelectTabMenuAction(hit.action))
    }

    pub(crate) fn dialog_open(&self) -> bool {
        self.tab_dialog.is_some()
            || self.close_tab_dialog.is_some()
            || self.delete_prompt_dialog.is_some()
    }

    pub(crate) fn overlay_open(&self) -> bool {
        self.dialog_open()
            || self.tab_menu.is_some()
            || self.search.is_some()
            || self.preview.is_some()
            || self.menu.is_some()
            || self.editor.is_some()
    }

    /// Opens the workspaces overlay with the current entries, selecting the
    /// workspace this window is on.
    pub fn open_workspaces(&mut self, entries: Vec<WorkspaceEntry>) {
        let selected = entries.iter().position(|entry| entry.current).unwrap_or(0);
        self.menu = Some(MenuState::Workspaces(WorkspacesOverlay {
            entries,
            selected,
            mode: WorkspacesMode::List,
            error: String::new(),
        }));
    }
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
