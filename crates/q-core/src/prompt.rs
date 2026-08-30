use std::path::{Path, PathBuf};

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

/// The live source of a prompt.
///
/// External paths are deliberately stored exactly as supplied. In particular,
/// they are not canonicalized, and constructing a source never accesses the
/// filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptSource {
    Inline { text: String },
    ExternalMarkdown { path: PathBuf },
}

impl PromptSource {
    pub fn inline(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(CoreError::Invalid("prompt text is empty".into()));
        }
        Ok(Self::Inline { text })
    }

    /// Makes a live reference to an external Markdown document.
    ///
    /// Only the shape of the path is checked: it must be absolute and Unicode.
    /// The path is not canonicalized and does not need to exist yet.
    pub fn external_markdown(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        validate_external_path(&path)?;
        Ok(Self::ExternalMarkdown { path })
    }

    pub fn inline_text(&self) -> Option<&str> {
        match self {
            Self::Inline { text } => Some(text),
            Self::ExternalMarkdown { .. } => None,
        }
    }

    pub fn external_markdown_path(&self) -> Option<&Path> {
        match self {
            Self::Inline { .. } => None,
            Self::ExternalMarkdown { path } => Some(path),
        }
    }

    /// Number of UTF-8 bytes charged to the persisted history budget.
    pub(crate) fn byte_len(&self) -> usize {
        match self {
            Self::Inline { text } => text.len(),
            // External paths are guaranteed to be Unicode by construction and
            // are validated again when a workspace is loaded.
            Self::ExternalMarkdown { path } => path.to_str().map_or(usize::MAX, str::len),
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::Inline { text } => {
                if text.trim().is_empty() {
                    return Err(CoreError::Invalid("prompt text is empty".into()));
                }
                Ok(())
            }
            Self::ExternalMarkdown { path } => validate_external_path(path),
        }
    }
}

fn validate_external_path(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(CoreError::Invalid(
            "external Markdown path must be absolute".into(),
        ));
    }
    if path.to_str().is_none() {
        return Err(CoreError::Invalid(
            "external Markdown path must be Unicode".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prompt {
    pub id: PromptId,
    source: PromptSource,
    /// When the prompt was pinned; `None` means unpinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Prompt {
    /// Constructs an inline prompt.
    pub fn new(text: impl Into<String>) -> Result<Self> {
        Self::from_source(PromptSource::inline(text)?)
    }

    /// Constructs a prompt backed by a live external Markdown reference.
    pub fn from_external_markdown(path: impl Into<PathBuf>) -> Result<Self> {
        Self::from_source(PromptSource::external_markdown(path)?)
    }

    pub fn from_source(source: PromptSource) -> Result<Self> {
        source.validate()?;
        Ok(Prompt {
            id: PromptId::new(),
            source,
            pinned_at: None,
            created_at: Utc::now(),
        })
    }

    pub fn pinned(&self) -> bool {
        self.pinned_at.is_some()
    }

    /// Pins or unpins the prompt. Pinning an already pinned prompt keeps the
    /// original pin time.
    pub fn set_pinned(&mut self, pinned: bool) {
        match (pinned, self.pinned_at) {
            (true, None) => self.pinned_at = Some(Utc::now()),
            (false, Some(_)) => self.pinned_at = None,
            _ => {}
        }
    }

    pub fn source(&self) -> &PromptSource {
        &self.source
    }

    pub fn inline_text(&self) -> Option<&str> {
        self.source.inline_text()
    }

    pub fn external_markdown_path(&self) -> Option<&Path> {
        self.source.external_markdown_path()
    }

    pub(crate) fn replace_inline_text(&mut self, text: impl Into<String>) -> Result<()> {
        if !matches!(self.source, PromptSource::Inline { .. }) {
            return Err(CoreError::Invalid(
                "external Markdown prompts cannot be edited as inline text".into(),
            ));
        }
        self.source = PromptSource::inline(text)?;
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<()> {
        self.source.validate()
    }

    /// First inline line trimmed to 80 chars. External prompts use their path.
    pub fn preview(&self) -> String {
        let value = match &self.source {
            PromptSource::Inline { text } => text.lines().next().unwrap_or("").trim(),
            PromptSource::ExternalMarkdown { path } => path.to_str().unwrap_or(""),
        };
        if value.chars().count() <= 80 {
            value.to_string()
        } else {
            let mut preview: String = value.chars().take(77).collect();
            preview.push_str("...");
            preview
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/prompt.rs"]
mod tests;
