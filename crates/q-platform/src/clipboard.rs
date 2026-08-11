//! Text clipboard abstraction.
//!
//! `Clipboard` is the trait consumers depend on. `SystemClipboard` is the
//! real impl (via `arboard`). `FakeClipboard` is available behind the
//! `test-support` feature or in this crate's own tests.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard unavailable: {0}")]
    Unavailable(String),
}

pub trait Clipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

pub struct SystemClipboard {
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    pub fn new() -> Result<Self, ClipboardError> {
        arboard::Clipboard::new()
            .map(|inner| Self { inner })
            .map_err(|e| ClipboardError::Unavailable(e.to_string()))
    }
}

impl Clipboard for SystemClipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.inner
            .set_text(text.to_string())
            .map_err(|e| ClipboardError::Unavailable(e.to_string()))
    }
}

#[cfg(any(test, feature = "test-support"))]
pub struct FakeClipboard {
    pub last: Option<String>,
}

#[cfg(any(test, feature = "test-support"))]
impl FakeClipboard {
    pub fn new() -> Self {
        Self { last: None }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Default for FakeClipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Clipboard for FakeClipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.last = Some(text.to_string());
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/clipboard.rs"]
mod tests;
