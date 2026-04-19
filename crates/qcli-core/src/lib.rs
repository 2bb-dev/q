//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;
pub mod queue;
pub mod storage;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
pub use queue::Queue;
