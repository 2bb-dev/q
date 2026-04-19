use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptId(pub Uuid);

impl Default for PromptId {
    fn default() -> Self {
        Self::new()
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_text() {
        assert!(Prompt::new("").is_err());
        assert!(Prompt::new("   \n\t").is_err());
    }

    #[test]
    fn new_accepts_non_empty_text() {
        let p = Prompt::new("hello world").expect("should succeed");
        assert_eq!(p.text, "hello world");
        assert!(!p.pinned);
    }

    #[test]
    fn preview_uses_first_line_and_truncates_at_80() {
        let p = Prompt::new("first line\nsecond line").unwrap();
        assert_eq!(p.preview(), "first line");

        let long = "a".repeat(100);
        let p = Prompt::new(&long).unwrap();
        let preview = p.preview();
        assert_eq!(preview.chars().count(), 80);
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn prompt_id_display_is_8_chars() {
        let id = PromptId::new();
        let s = id.to_string();
        assert_eq!(s.chars().count(), 8);
    }

    #[test]
    fn parse_input_rejects_short_ids() {
        assert!(PromptId::parse_input("abc").is_err());
        assert!(PromptId::parse_input("abcd").is_ok());
    }
}
