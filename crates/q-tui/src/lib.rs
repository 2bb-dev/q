//! q-cli TUI crate (ratatui + crossterm).
pub mod app;
pub mod reducer;
pub mod render;
pub mod runtime;
pub use app::{App, Effect, Input, Pane, QueueMutation};
pub use reducer::reduce;
pub use render::draw;
pub use runtime::run;
