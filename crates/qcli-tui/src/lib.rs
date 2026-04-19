//! q-cli TUI crate (ratatui + crossterm).
pub mod app;
pub mod reducer;
pub mod render;
pub use app::{App, Effect, Input, Pane};
pub use reducer::reduce;
pub use render::draw;
