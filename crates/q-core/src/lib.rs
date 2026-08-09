//! q-cli domain crate: prompt queues, workspaces, and persistence.

pub mod error;
pub mod prompt;
pub mod queue;
pub mod storage;
pub mod workspace;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
pub use queue::Queue;
pub use workspace::{Tab, TabId, Workspace};
