//! q-cli platform crate: app dirs, file locking, clipboard (text + image).
//!
//! This crate is the only one allowed to depend on `arboard`, `fd-lock`,
//! `directories`, and `image`. All OS-specific concerns live here.

pub mod clipboard;
pub mod images;
pub mod lock;
pub mod paths;
