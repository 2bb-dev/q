//! q-cli platform crate: app dirs, file locking, clipboard.
//!
//! This crate is the only one allowed to depend on `arboard`, `fd-lock`,
//! and `directories`. All OS-specific concerns live here.

pub mod clipboard;
pub mod external_document;
pub mod github;
pub mod lock;
pub mod paths;

/// Serializes `QCLI_APP_DIR` mutation across unit-test modules. The env var
/// is process-wide and unit tests run in parallel within one binary.
#[cfg(test)]
pub(crate) static TEST_ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
