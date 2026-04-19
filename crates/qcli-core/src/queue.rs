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

    /// Insertion rules:
    ///   pinned == true  → end of pinned section (before first unpinned)
    ///   pinned == false → end of full list
    pub fn add(&mut self, prompt: Prompt) -> PromptId {
        let id = prompt.id;
        if prompt.pinned {
            let insert_at = self
                .prompts
                .iter()
                .position(|p| !p.pinned)
                .unwrap_or(self.prompts.len());
            self.prompts.insert(insert_at, prompt);
        } else {
            self.prompts.push(prompt);
        }
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

    /// Build a Prompt from raw text and add it at the tail (unpinned).
    /// Returns the new id, or CoreError::Invalid if text is empty/whitespace.
    pub fn add_text(&mut self, text: impl Into<String>) -> Result<PromptId> {
        let prompt = Prompt::new(text)?;
        Ok(self.add(prompt))
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
mod tests {
    use super::*;

    fn p(text: &str) -> Prompt {
        Prompt::new(text).unwrap()
    }

    #[test]
    fn add_appends_unpinned_at_end() {
        let mut q = Queue::new();
        q.add(p("a"));
        q.add(p("b"));
        assert_eq!(
            q.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn pinned_prompts_sort_before_unpinned() {
        let mut q = Queue::new();
        q.add(p("one"));
        q.add(p("two"));
        let mut pinned = p("zero");
        pinned.pinned = true;
        q.add(pinned);
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["zero", "one", "two"]);
    }

    #[test]
    fn remove_returns_the_prompt() {
        let mut q = Queue::new();
        let id = q.add(p("foo"));
        let removed = q.remove(id).unwrap();
        assert_eq!(removed.text, "foo");
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn edit_replaces_text() {
        let mut q = Queue::new();
        let id = q.add(p("old"));
        q.edit(id, "new").unwrap();
        assert_eq!(q.get(id).unwrap().text, "new");
    }

    #[test]
    fn edit_rejects_empty() {
        let mut q = Queue::new();
        let id = q.add(p("old"));
        assert!(q.edit(id, "").is_err());
    }

    #[test]
    fn set_pinned_true_moves_to_pinned_section() {
        let mut q = Queue::new();
        q.add(p("a"));
        let id = q.add(p("b"));
        q.add(p("c"));
        q.set_pinned(id, true).unwrap();
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["b", "a", "c"]);
    }

    #[test]
    fn pop_next_unpinned_skips_pinned_head() {
        let mut q = Queue::new();
        let mut pinned = p("stay");
        pinned.pinned = true;
        q.add(pinned);
        q.add(p("go"));
        let popped = q.pop_next_unpinned().unwrap();
        assert_eq!(popped.text, "go");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn pop_next_unpinned_returns_none_when_only_pinned() {
        let mut q = Queue::new();
        let mut pinned = p("only");
        pinned.pinned = true;
        q.add(pinned);
        assert!(q.pop_next_unpinned().is_none());
    }

    #[test]
    fn resolve_by_full_id_succeeds() {
        let mut q = Queue::new();
        let id = q.add(p("hello"));
        let full = id.0.as_hyphenated().to_string();
        assert_eq!(q.resolve(&full).unwrap(), id);
    }

    #[test]
    fn resolve_by_short_prefix_succeeds() {
        let mut q = Queue::new();
        let id = q.add(p("hello"));
        let short = id.to_string();
        assert_eq!(q.resolve(&short).unwrap(), id);
    }

    #[test]
    fn resolve_reports_not_found() {
        let q = Queue::new();
        assert!(matches!(q.resolve("abcd"), Err(CoreError::NotFound(_))));
    }

    #[test]
    fn clear_empties_queue() {
        let mut q = Queue::new();
        q.add(p("a"));
        q.add(p("b"));
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn iter_pinned_only_yields_pinned() {
        let mut q = Queue::new();
        q.add(p("unpinned-a"));
        let mut pin1 = p("pinned-1");
        pin1.pinned = true;
        let mut pin2 = p("pinned-2");
        pin2.pinned = true;
        q.add(pin1);
        q.add(pin2);
        q.add(p("unpinned-b"));
        let pinned: Vec<_> = q.iter_pinned().map(|p| p.text.as_str()).collect();
        assert_eq!(pinned.len(), 2);
        assert!(pinned.contains(&"pinned-1"));
        assert!(pinned.contains(&"pinned-2"));
    }

    #[test]
    fn iter_unpinned_only_yields_unpinned() {
        let mut q = Queue::new();
        q.add(p("unpinned-a"));
        q.add(p("unpinned-b"));
        let mut pin = p("pinned-1");
        pin.pinned = true;
        q.add(pin);
        let unpinned: Vec<_> = q.iter_unpinned().map(|p| p.text.as_str()).collect();
        assert_eq!(unpinned.len(), 2);
        assert!(unpinned.contains(&"unpinned-a"));
        assert!(unpinned.contains(&"unpinned-b"));
    }

    #[test]
    fn add_text_builds_prompt_and_returns_id() {
        let mut q = Queue::new();
        let id = q.add_text("hello").unwrap();
        assert_eq!(q.len(), 1);
        let prompt = q.get(id).unwrap();
        assert_eq!(prompt.text, "hello");
        assert_eq!(prompt.id, id);
    }

    #[test]
    fn add_text_rejects_empty() {
        let mut q = Queue::new();
        assert!(q.add_text("").is_err());
        assert!(q.add_text("   \n").is_err());
    }

    #[test]
    fn move_within_group_swaps_within_unpinned() {
        let mut q = Queue::new();
        let id0 = q.add_text("first").unwrap();
        let _id1 = q.add_text("second").unwrap();
        let _id2 = q.add_text("third").unwrap();
        let result = q.move_within_group(id0, 1).unwrap();
        assert!(result);
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts[0], "second");
        assert_eq!(texts[1], "first");
        assert_eq!(texts[2], "third");
    }

    #[test]
    fn move_within_group_clamps_at_boundary() {
        let mut q = Queue::new();
        let head_id = q.add_text("head").unwrap();
        q.add_text("mid").unwrap();
        let tail_id = q.add_text("tail").unwrap();
        // Move head up — already at boundary
        assert!(!q.move_within_group(head_id, -1).unwrap());
        // Move tail down — already at boundary
        assert!(!q.move_within_group(tail_id, 1).unwrap());
        // Order unchanged
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["head", "mid", "tail"]);
    }

    #[test]
    fn move_within_group_cannot_cross_into_other_group() {
        let mut q = Queue::new();
        let mut pin = p("pinned");
        pin.pinned = true;
        let pin_id = q.add(pin);
        let unpin0_id = q.add_text("unpin-first").unwrap();
        q.add_text("unpin-second").unwrap();
        // Moving the only pinned prompt down by 5 — single-item group, clamps to itself
        assert!(!q.move_within_group(pin_id, 5).unwrap());
        // Moving first unpinned up by 5 — clamps to unpinned-group head, no change
        assert!(!q.move_within_group(unpin0_id, -5).unwrap());
        // Invariant: pinned still first
        assert!(q.prompts[0].pinned);
        assert!(!q.prompts[1].pinned);
    }

    #[test]
    fn move_within_group_unknown_id_is_error() {
        let mut q = Queue::new();
        let unknown = PromptId::new();
        assert!(matches!(
            q.move_within_group(unknown, 1),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn move_within_group_zero_delta_is_noop() {
        let mut q = Queue::new();
        let id = q.add_text("only").unwrap();
        assert!(!q.move_within_group(id, 0).unwrap());
        assert_eq!(q.len(), 1);
    }
}
