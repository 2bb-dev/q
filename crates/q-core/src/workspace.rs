use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::{Prompt, PromptId, Queue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TabId(pub Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    fn initial() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    id: TabId,
    name: String,
    activity_at: DateTime<Utc>,
    queue: Queue,
}

impl Tab {
    pub fn id(&self) -> TabId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn activity_at(&self) -> DateTime<Utc> {
        self.activity_at
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

/// A prompt that was added at some point, kept even after the prompt is
/// copied, deleted, or its tab is closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub text: String,
    pub created_at: DateTime<Utc>,
}

/// Newest entries are kept; older ones are trimmed.
pub const HISTORY_LIMIT: usize = 500;

/// Upper bound on the bytes of prompt text kept in history. Prompt length is
/// unbounded, so the entry count alone does not bound the size of the file the
/// TUI reloads and every mutation rewrites.
pub const HISTORY_BYTE_BUDGET: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    tabs: Vec<Tab>,
    #[serde(default)]
    history: Vec<HistoryEntry>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Queue> for Workspace {
    fn from(queue: Queue) -> Self {
        Self::from_legacy_queue(queue, Utc::now())
    }
}

impl Workspace {
    pub fn new() -> Self {
        Self::with_initial_activity(Utc::now())
    }

    pub(crate) fn with_initial_activity(activity_at: DateTime<Utc>) -> Self {
        Self {
            tabs: vec![Tab {
                id: TabId::initial(),
                name: "1".to_string(),
                activity_at,
                queue: Queue::new(),
            }],
            history: Vec::new(),
        }
    }

    pub(crate) fn from_legacy_queue(mut queue: Queue, migrated_at: DateTime<Utc>) -> Self {
        queue.normalize();
        let activity_at = queue
            .iter()
            .map(|prompt| prompt.created_at)
            .max()
            .unwrap_or(migrated_at);
        let mut workspace = Self {
            tabs: vec![Tab {
                id: TabId::initial(),
                name: "1".to_string(),
                activity_at,
                queue,
            }],
            history: Vec::new(),
        };
        workspace.seed_history_from_prompts();
        workspace
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Every prompt ever added, newest first.
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Fills history from the prompts currently queued, for workspaces stored
    /// before history was recorded.
    pub(crate) fn seed_history_from_prompts(&mut self) {
        let mut prompts: Vec<_> = self
            .tabs
            .iter()
            .flat_map(|tab| tab.queue.iter())
            .map(|prompt| HistoryEntry {
                text: prompt.text.clone(),
                created_at: prompt.created_at,
            })
            .collect();
        prompts.sort_by_key(|entry| entry.created_at);
        for entry in prompts {
            self.record_history(entry.text, entry.created_at);
        }
    }

    fn record_history(&mut self, text: String, created_at: DateTime<Utc>) {
        self.history.retain(|entry| entry.text != text);
        self.history.insert(0, HistoryEntry { text, created_at });
        self.trim_history();
    }

    /// Drops the oldest entries until history fits both the entry count and the
    /// byte budget. The newest entry is always kept, even when oversized on its
    /// own, so a freshly added prompt is never immediately unsearchable.
    fn trim_history(&mut self) {
        self.history.truncate(HISTORY_LIMIT);
        let mut used = 0usize;
        let mut kept = 0usize;
        for entry in &self.history {
            used = used.saturating_add(entry.text.len());
            kept += 1;
            if used > HISTORY_BYTE_BUDGET {
                break;
            }
        }
        self.history.truncate(kept.max(1));
    }

    /// Forgets the single history entry whose text matches exactly.
    pub fn forget_history(&mut self, text: &str) -> bool {
        let before = self.history.len();
        self.history.retain(|entry| entry.text != text);
        before != self.history.len()
    }

    /// Forgets every history entry matching `query`, returning how many went.
    pub fn forget_history_matching(&mut self, query: &str) -> usize {
        let query = crate::search::Query::new(query);
        let before = self.history.len();
        self.history.retain(|entry| !query.is_match(&entry.text));
        before - self.history.len()
    }

    /// Forgets every history entry.
    pub fn clear_history(&mut self) -> usize {
        let forgotten = self.history.len();
        self.history.clear();
        forgotten
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub fn first_tab_id(&self) -> TabId {
        self.tabs[0].id
    }

    pub fn resolve_tab(&self, name: &str) -> Result<TabId> {
        let normalized = normalize_name(name)?;
        self.tabs
            .iter()
            .find(|tab| tab.name.to_lowercase() == normalized)
            .map(|tab| tab.id)
            .ok_or_else(|| {
                CoreError::TabNotFound(format!(
                    "{} (available: {})",
                    name.trim(),
                    self.available_names()
                ))
            })
    }

    pub fn resolve_context_tab(&self, name: Option<&str>) -> Result<TabId> {
        match name {
            Some(name) => self.resolve_tab(name),
            None if self.tabs.len() == 1 => Ok(self.tabs[0].id),
            None => Err(CoreError::TabRequired(self.available_names())),
        }
    }

    pub fn create_tab(&mut self, name: impl Into<String>) -> Result<TabId> {
        self.create_tab_with(TabId::new(), name, Utc::now())
    }

    pub fn create_tab_with(
        &mut self,
        id: TabId,
        name: impl Into<String>,
        activity_at: DateTime<Utc>,
    ) -> Result<TabId> {
        if self.tabs.iter().any(|tab| tab.id == id) {
            return Err(CoreError::InvalidTab(format!(
                "tab id already exists: {}",
                id.0
            )));
        }
        let name = self.validate_new_name(name.into(), None)?;
        self.tabs.push(Tab {
            id,
            name,
            activity_at,
            queue: Queue::new(),
        });
        self.normalize();
        Ok(id)
    }

    pub fn rename_tab(&mut self, id: TabId, name: impl Into<String>) -> Result<()> {
        let name = self.validate_new_name(name.into(), Some(id))?;
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == id)
            .ok_or_else(|| CoreError::TabNotFound(id.0.to_string()))?;
        tab.name = name;
        Ok(())
    }

    pub fn close_tab(&mut self, id: TabId) -> Result<()> {
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.id == id)
            .ok_or_else(|| CoreError::TabNotFound(id.0.to_string()))?;
        if self.tabs.len() == 1 {
            return Err(CoreError::InvalidTab(
                "cannot close the last tab".to_string(),
            ));
        }
        self.tabs.remove(index);
        Ok(())
    }

    pub fn add_prompt(&mut self, tab_id: TabId, prompt: Prompt) -> Result<PromptId> {
        let prompt_id = prompt.id;
        let activity_at = prompt.created_at;
        let history_text = prompt.text.clone();
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .ok_or_else(|| CoreError::TabNotFound(tab_id.0.to_string()))?;
        tab.queue.add(prompt);
        tab.activity_at = tab.activity_at.max(activity_at);
        self.record_history(history_text, activity_at);
        self.normalize();
        Ok(prompt_id)
    }

    pub fn resolve_prompt(&self, input: &str) -> Result<PromptId> {
        let prefix = crate::PromptId::parse_input(input)?;
        let matches: Vec<_> = self
            .tabs
            .iter()
            .flat_map(|tab| tab.queue.iter())
            .filter(|prompt| {
                prompt.id.0.as_hyphenated().to_string().starts_with(&prefix)
                    || prompt.id.to_string().starts_with(&prefix)
            })
            .collect();
        match matches.len() {
            0 => Err(CoreError::NotFound(prefix)),
            1 => Ok(matches[0].id),
            _ => Err(CoreError::Invalid(format!("ambiguous id prefix: {prefix}"))),
        }
    }

    pub fn get_prompt(&self, id: PromptId) -> Option<&Prompt> {
        self.tabs.iter().find_map(|tab| tab.queue.get(id))
    }

    pub fn remove_prompt(&mut self, id: PromptId) -> Result<Prompt> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.queue.get(id).is_some())
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        tab.queue.remove(id)
    }

    pub fn set_prompt_pinned(&mut self, id: PromptId, pinned: bool) -> Result<()> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.queue.get(id).is_some())
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        tab.queue.set_pinned(id, pinned)
    }

    pub fn pop_next_unpinned(&mut self, tab_id: TabId) -> Result<Option<Prompt>> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .ok_or_else(|| CoreError::TabNotFound(tab_id.0.to_string()))?;
        Ok(tab.queue.pop_next_unpinned())
    }

    pub(crate) fn validate_and_normalize(&mut self) -> Result<()> {
        if self.tabs.is_empty() {
            return Err(CoreError::InvalidTab(
                "workspace must contain at least one tab".to_string(),
            ));
        }
        for tab in &mut self.tabs {
            tab.name = tab.name.trim().to_string();
            normalize_name(&tab.name)?;
        }
        for index in 0..self.tabs.len() {
            let tab = &self.tabs[index];
            if self.tabs[..index].iter().any(|other| other.id == tab.id) {
                return Err(CoreError::InvalidTab(format!(
                    "tab id already exists: {}",
                    tab.id.0
                )));
            }
            if self.tabs[..index]
                .iter()
                .any(|other| other.name.to_lowercase() == tab.name.to_lowercase())
            {
                return Err(CoreError::InvalidTab(format!(
                    "tab name already exists: {}",
                    tab.name
                )));
            }
        }
        self.normalize();
        Ok(())
    }

    fn normalize(&mut self) {
        for tab in &mut self.tabs {
            tab.queue.normalize();
        }
        self.history
            .sort_by_key(|entry| std::cmp::Reverse(entry.created_at));
        self.trim_history();
        self.tabs.sort_by(|left, right| {
            right
                .activity_at
                .cmp(&left.activity_at)
                .then_with(|| left.id.cmp(&right.id))
        });
    }

    fn validate_new_name(&self, name: String, current: Option<TabId>) -> Result<String> {
        let trimmed = name.trim();
        let normalized = normalize_name(trimmed)?;
        if self
            .tabs
            .iter()
            .any(|tab| Some(tab.id) != current && tab.name.to_lowercase() == normalized)
        {
            return Err(CoreError::InvalidTab(format!(
                "tab name already exists: {trimmed}"
            )));
        }
        Ok(trimmed.to_string())
    }

    fn available_names(&self) -> String {
        self.tabs
            .iter()
            .map(|tab| tab.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn normalize_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidTab("tab name is empty".to_string()));
    }
    Ok(trimmed.to_lowercase())
}

#[cfg(test)]
#[path = "../tests/unit/workspace.rs"]
mod tests;
