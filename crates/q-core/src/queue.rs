use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::prompt::{Prompt, PromptId};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Queue {
    prompts: Vec<Prompt>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.prompts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Prompt> {
        self.prompts.iter()
    }

    /// Insert pinned prompts before unpinned prompts, newest first within each group.
    pub fn add(&mut self, prompt: Prompt) -> PromptId {
        let id = prompt.id;
        self.prompts.push(prompt);
        self.normalize();
        id
    }

    /// Resolve a user-supplied id string (full UUID or prefix >= 4 chars).
    pub fn resolve(&self, input: &str) -> Result<PromptId> {
        let prefix = PromptId::parse_input(input)?;
        let matches: Vec<_> = self
            .prompts
            .iter()
            .filter(|p| {
                p.id.0.as_hyphenated().to_string().starts_with(&prefix)
                    || p.id.to_string().starts_with(&prefix)
            })
            .collect();
        match matches.len() {
            0 => Err(CoreError::NotFound(prefix)),
            1 => Ok(matches[0].id),
            _ => Err(CoreError::Invalid(format!("ambiguous id prefix: {prefix}"))),
        }
    }

    pub fn get(&self, id: PromptId) -> Option<&Prompt> {
        self.prompts.iter().find(|p| p.id == id)
    }

    pub fn remove(&mut self, id: PromptId) -> Result<Prompt> {
        let pos = self
            .prompts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        Ok(self.prompts.remove(pos))
    }

    pub fn edit(&mut self, id: PromptId, new_text: impl Into<String>) -> Result<()> {
        let new_text = new_text.into();
        if new_text.trim().is_empty() {
            return Err(CoreError::Invalid("prompt text is empty".into()));
        }
        let p = self
            .prompts
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        p.text = new_text;
        Ok(())
    }

    pub fn set_pinned(&mut self, id: PromptId, pinned: bool) -> Result<()> {
        let pos = self
            .prompts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        let mut p = self.prompts.remove(pos);
        p.pinned = pinned;
        let _ = self.add(p);
        Ok(())
    }

    /// Head of the queue: first pinned if any, else first unpinned, else None.
    pub fn peek_next(&self) -> Option<&Prompt> {
        self.prompts.first()
    }

    /// Pop the first unpinned prompt. Returns None if all prompts are pinned.
    pub fn pop_next_unpinned(&mut self) -> Option<Prompt> {
        let pos = self.prompts.iter().position(|p| !p.pinned)?;
        Some(self.prompts.remove(pos))
    }

    pub fn clear(&mut self) {
        self.prompts.clear();
    }

    pub fn iter_pinned(&self) -> impl Iterator<Item = &Prompt> {
        self.prompts.iter().filter(|p| p.pinned)
    }

    pub fn iter_unpinned(&self) -> impl Iterator<Item = &Prompt> {
        self.prompts.iter().filter(|p| !p.pinned)
    }

    /// Build a Prompt from raw text and add it to the unpinned group.
    /// Returns the new id, or CoreError::Invalid if text is empty/whitespace.
    pub fn add_text(&mut self, text: impl Into<String>) -> Result<PromptId> {
        let prompt = Prompt::new(text)?;
        Ok(self.add(prompt))
    }

    pub(crate) fn normalize(&mut self) {
        self.prompts.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.created_at.cmp(&left.created_at))
                .then_with(|| left.id.cmp(&right.id))
        });
    }

    pub fn move_within_group(&mut self, id: PromptId, delta: i32) -> Result<bool> {
        if delta == 0 {
            return Ok(false);
        }
        let cur = self
            .prompts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        let pinned = self.prompts[cur].pinned;
        let (lo, hi) = if pinned {
            (
                0,
                self.prompts
                    .iter()
                    .position(|p| !p.pinned)
                    .unwrap_or(self.prompts.len()),
            )
        } else {
            (
                self.prompts
                    .iter()
                    .position(|p| !p.pinned)
                    .unwrap_or(self.prompts.len()),
                self.prompts.len(),
            )
        };

        let target_signed = cur as i32 + delta;
        let target = target_signed.clamp(lo as i32, hi as i32 - 1) as usize;
        if target == cur {
            return Ok(false);
        }

        if target > cur {
            for i in cur..target {
                self.prompts.swap(i, i + 1);
            }
        } else {
            for i in (target..cur).rev() {
                self.prompts.swap(i, i + 1);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../tests/unit/queue.rs"]
mod tests;
