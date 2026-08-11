use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PromptId(pub Uuid);

#[allow(clippy::new_without_default)]
impl PromptId {
    pub fn new() -> Self {
        PromptId(Uuid::new_v4())
    }

    pub fn parse_input(s: &str) -> Result<String> {
        let s = s.trim();
        if s.len() < 4 {
            return Err(CoreError::Invalid(format!(
                "prompt id too short (min 4 chars): {s}"
            )));
        }
        Ok(s.to_string())
    }
}

impl std::fmt::Display for PromptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0.as_hyphenated().to_string()[..8])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prompt {
    pub id: PromptId,
    pub text: String,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
}

impl Prompt {
    pub fn new(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(CoreError::Invalid("prompt text is empty".into()));
        }
        Ok(Prompt {
            id: PromptId::new(),
            text,
            pinned: false,
            created_at: Utc::now(),
        })
    }

    /// First line trimmed to 80 chars, for list display.
    pub fn preview(&self) -> String {
        let first_line = self.text.lines().next().unwrap_or("").trim();
        if first_line.chars().count() <= 80 {
            first_line.to_string()
        } else {
            let mut s: String = first_line.chars().take(77).collect();
            s.push_str("...");
            s
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/prompt.rs"]
mod tests;
