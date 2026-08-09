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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    tabs: Vec<Tab>,
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
        }
    }

    pub(crate) fn from_legacy_queue(mut queue: Queue, migrated_at: DateTime<Utc>) -> Self {
        queue.normalize();
        let activity_at = queue
            .iter()
            .map(|prompt| prompt.created_at)
            .max()
            .unwrap_or(migrated_at);
        Self {
            tabs: vec![Tab {
                id: TabId::initial(),
                name: "1".to_string(),
                activity_at,
                queue,
            }],
        }
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
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
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .ok_or_else(|| CoreError::TabNotFound(tab_id.0.to_string()))?;
        tab.queue.add(prompt);
        tab.activity_at = tab.activity_at.max(activity_at);
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
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn new_workspace_starts_with_tab_one() {
        let workspace = Workspace::new();
        assert_eq!(workspace.tabs().len(), 1);
        assert_eq!(workspace.tabs()[0].name(), "1");
        assert!(workspace.tabs()[0].queue().is_empty());
    }

    #[test]
    fn tab_names_are_trimmed_and_case_insensitively_unique() {
        let mut workspace = Workspace::new();
        let id = workspace.create_tab("  Work  ").unwrap();
        assert_eq!(workspace.tab(id).unwrap().name(), "Work");
        assert!(workspace.create_tab("work").is_err());
        assert!(workspace.create_tab("  ").is_err());
    }

    #[test]
    fn rename_preserves_tab_data_and_order() {
        let now = Utc::now();
        let mut workspace = Workspace::with_initial_activity(now);
        let id = workspace
            .create_tab_with(TabId::new(), "work", now + Duration::seconds(1))
            .unwrap();
        let prompt = Prompt::new("hello").unwrap();
        let prompt_id = prompt.id;
        workspace.add_prompt(id, prompt).unwrap();
        let activity = workspace.tab(id).unwrap().activity_at();

        workspace.rename_tab(id, "renamed").unwrap();

        let tab = workspace.tab(id).unwrap();
        assert_eq!(tab.name(), "renamed");
        assert_eq!(tab.activity_at(), activity);
        assert_eq!(tab.queue().get(prompt_id).unwrap().text, "hello");
    }

    #[test]
    fn adding_prompt_moves_tab_first() {
        let now = Utc::now();
        let mut workspace = Workspace::with_initial_activity(now);
        let second = workspace
            .create_tab_with(TabId::new(), "second", now + Duration::seconds(1))
            .unwrap();
        let first = workspace.resolve_tab("1").unwrap();
        assert_eq!(workspace.first_tab_id(), second);

        let mut prompt = Prompt::new("latest").unwrap();
        prompt.created_at = now + Duration::seconds(2);
        workspace.add_prompt(first, prompt).unwrap();

        assert_eq!(workspace.first_tab_id(), first);
    }

    #[test]
    fn close_tab_removes_it_and_its_prompts() {
        let mut workspace = Workspace::new();
        let closed = workspace.create_tab("closed").unwrap();
        let kept = workspace.resolve_tab("1").unwrap();
        let prompt = Prompt::new("discarded").unwrap();
        let prompt_id = prompt.id;
        workspace.add_prompt(closed, prompt).unwrap();

        workspace.close_tab(closed).unwrap();

        assert_eq!(workspace.tabs().len(), 1);
        assert_eq!(workspace.first_tab_id(), kept);
        assert!(workspace.get_prompt(prompt_id).is_none());
    }

    #[test]
    fn last_tab_cannot_be_closed() {
        let mut workspace = Workspace::new();
        let only = workspace.first_tab_id();

        let error = workspace.close_tab(only).unwrap_err();

        assert_eq!(error.to_string(), "invalid tab: cannot close the last tab");
        assert_eq!(workspace.tabs().len(), 1);
    }

    #[test]
    fn out_of_order_prompt_add_does_not_regress_tab_activity() {
        let now = Utc::now();
        let mut workspace = Workspace::with_initial_activity(now);
        let first = workspace.first_tab_id();
        let second = workspace
            .create_tab_with(TabId::new(), "second", now + Duration::seconds(5))
            .unwrap();
        let mut newer = Prompt::new("newer").unwrap();
        newer.created_at = now + Duration::seconds(10);
        workspace.add_prompt(first, newer).unwrap();
        let mut older = Prompt::new("older committed later").unwrap();
        older.created_at = now + Duration::seconds(2);

        workspace.add_prompt(first, older).unwrap();

        assert_eq!(workspace.first_tab_id(), first);
        assert_eq!(
            workspace.tab(first).unwrap().activity_at(),
            now + Duration::seconds(10)
        );
        assert_eq!(workspace.tabs()[1].id(), second);
    }

    #[test]
    fn context_requires_name_only_when_multiple_tabs_exist() {
        let mut workspace = Workspace::new();
        assert!(workspace.resolve_context_tab(None).is_ok());
        workspace.create_tab("work").unwrap();
        assert!(matches!(
            workspace.resolve_context_tab(None),
            Err(CoreError::TabRequired(_))
        ));
        assert_eq!(
            workspace.resolve_context_tab(Some("WORK")).unwrap(),
            workspace.resolve_tab("work").unwrap()
        );
    }

    #[test]
    fn prompt_operations_find_owning_tab_globally() {
        let mut workspace = Workspace::new();
        let second = workspace.create_tab("second").unwrap();
        let prompt = Prompt::new("global").unwrap();
        let id = prompt.id;
        workspace.add_prompt(second, prompt).unwrap();

        assert_eq!(workspace.resolve_prompt(&id.to_string()).unwrap(), id);
        workspace.set_prompt_pinned(id, true).unwrap();
        assert!(workspace.get_prompt(id).unwrap().pinned);
        assert_eq!(workspace.remove_prompt(id).unwrap().text, "global");
        assert!(workspace.get_prompt(id).is_none());
    }
}
