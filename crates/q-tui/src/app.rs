use chrono::{DateTime, Utc};
use q_core::{HistoryEntry, Prompt, PromptId, TabId, Workspace};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
};
use ratatui_textarea::{CursorMove, TextArea, WrapMode};

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
    Remove(PromptId),
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
    ForgetHistory(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    CopyToClipboard(String),
    CopyAndPersist {
        text: String,
        mutation: QueueMutation,
    },
    Persist(QueueMutation),
    Quit,
    Status(String),
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
    History(String),
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

pub struct App {
    pub workspace: Workspace,
    pub active_tab_id: TabId,
    pub focus: Pane,
    pub selected: Option<usize>,
    pub composer: ComposerEditor,
    pub tab_dialog: Option<TabDialog>,
    pub tab_menu: Option<TabContextMenu>,
    pub close_tab_dialog: Option<CloseTabDialog>,
    pub preview: Option<PromptPreview>,
    pub search: Option<SearchDialog>,
    pub status: String,
    pub(crate) tab_hits: Vec<TabHit>,
    pub(crate) tab_menu_hits: Vec<TabMenuHit>,
    pub(crate) prompt_hits: Vec<PromptHit>,
    pub(crate) search_hits: Vec<SearchHit>,
    /// Folded history texts, positionally aligned with `workspace.history()`.
    /// Cached because folding the whole history on every keystroke and every
    /// frame is too slow once history fills up.
    pub(crate) search_folds: Vec<q_core::search::Folded>,
    pub(crate) composer_area: Option<Rect>,
    pub(crate) preview_page: u16,
    pub(crate) preview_max_scroll: u16,
}

impl App {
    pub fn new(workspace: impl Into<Workspace>) -> Self {
        let workspace = workspace.into();
        let active_tab_id = workspace.first_tab_id();
        let empty = workspace
            .tab(active_tab_id)
            .map(|tab| tab.queue().is_empty())
            .unwrap_or(true);
        Self {
            active_tab_id,
            focus: if empty { Pane::Composer } else { Pane::Queue },
            selected: if empty { None } else { Some(0) },
            workspace,
            composer: ComposerEditor::default(),
            tab_dialog: None,
            tab_menu: None,
            close_tab_dialog: None,
            preview: None,
            search: None,
            status: String::new(),
            tab_hits: Vec::new(),
            tab_menu_hits: Vec::new(),
            prompt_hits: Vec::new(),
            search_hits: Vec::new(),
            search_folds: Vec::new(),
            composer_area: None,
            preview_page: 1,
            preview_max_scroll: 0,
        }
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

    /// History entries matching the current search query, newest first.
    /// Uses the fold cache when it is in step with history, and otherwise folds
    /// on the spot so callers holding only `&self` still get correct results.
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
                    query.is_match(&entry.text)
                }
            })
            .map(|(_, entry)| entry)
            .collect()
    }

    /// Folds history once so later keystrokes and frames only substring-match.
    pub(crate) fn refresh_search_folds(&mut self) {
        if self.search_folds.len() == self.workspace.history().len() {
            return;
        }
        self.search_folds = self
            .workspace
            .history()
            .iter()
            .map(|entry| q_core::search::folded(&entry.text))
            .collect();
    }

    pub(crate) fn preview_text(&self) -> Option<String> {
        match &self.preview.as_ref()?.source {
            PreviewSource::Prompt(id) => self
                .workspace
                .get_prompt(*id)
                .map(|prompt| prompt.text.clone()),
            PreviewSource::History(text) => Some(text.clone()),
        }
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
        // Another window may have reordered or rewritten history.
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
        self.tab_dialog.is_some() || self.close_tab_dialog.is_some()
    }

    /// Any surface that takes over input from the queue and composer panes.
    pub(crate) fn overlay_open(&self) -> bool {
        self.dialog_open()
            || self.tab_menu.is_some()
            || self.search.is_some()
            || self.preview.is_some()
    }
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
