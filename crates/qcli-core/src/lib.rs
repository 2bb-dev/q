//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
