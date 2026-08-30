//! q-cli platform crate: app dirs, file locking, clipboard.
//!
//! This crate is the only one allowed to depend on `arboard`, `fd-lock`,
//! and `directories`. All OS-specific concerns live here.

pub mod clipboard;
pub mod external_document;
pub mod lock;
pub mod paths;
