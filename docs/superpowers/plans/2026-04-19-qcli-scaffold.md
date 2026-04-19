# q-cli Scaffold + Core + Scriptable CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `q-cli` Cargo workspace and deliver a usable, scriptable `q` binary that manages a persistent prompt queue from the shell (add / list / copy / pop / pin / unpin), with system clipboard integration. No TUI and no LLM providers in this plan — those are Plans 2 and 3.

**Architecture:** Cargo workspace with four crates (`qcli-core`, `qcli-platform`, `qcli-providers` stub, `qcli-tui` stub) and one binary crate (`qcli-bin`) that installs as `q`. `qcli-core` owns the queue domain and JSON-on-disk persistence; `qcli-platform` owns OS-specific concerns (app dir resolution, file locking, system clipboard). The binary layer is thin — it parses `clap` subcommands and delegates to `qcli-core`.

**Tech Stack:** Rust 2021, `serde` + `serde_json` for persistence, `clap` (derive) for CLI parsing, `arboard` for clipboard, `fd-lock` for file locking, `directories` for XDG/macOS app-dir resolution, `thiserror` for error types, `anyhow` for binary-layer error propagation. Tests: built-in `#[test]` for unit tests, `assert_cmd` + `tempfile` + `predicates` for CLI integration tests.

**Migration context:** This maps to migration milestones 1 and 2 from [q#11](https://github.com/2bb-dev/q/issues/11). TUI (M3), providers (M4), Codex (M5), doctor/packaging (M6) are out of scope and will get their own plans.

---

## File Structure

```
q-cli/
├── Cargo.toml                                   # workspace root
├── README.md                                    # install + usage
├── rust-toolchain.toml                          # pin toolchain
├── .gitignore
├── CLAUDE.md                                    # (already written)
├── AGENTS.md                                    # (already written)
├── docs/superpowers/plans/
│   └── 2026-04-19-qcli-scaffold.md              # this file
└── crates/
    ├── qcli-core/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs                           # public re-exports
    │       ├── error.rs                         # CoreError
    │       ├── prompt.rs                        # Prompt, PromptId
    │       ├── queue.rs                         # Queue, in-memory ops
    │       └── storage.rs                       # JSON persistence
    ├── qcli-platform/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs                           # public re-exports
    │       ├── paths.rs                         # app_dir(), queue_path()
    │       ├── lock.rs                          # FileLock wrapper
    │       └── clipboard.rs                     # Clipboard trait + SystemClipboard
    ├── qcli-providers/
    │   ├── Cargo.toml                           # stub — empty for this plan
    │   └── src/lib.rs
    ├── qcli-tui/
    │   ├── Cargo.toml                           # stub — empty for this plan
    │   └── src/lib.rs
    └── qcli-bin/
        ├── Cargo.toml                           # produces `q` binary
        └── src/
            ├── main.rs                          # clap entrypoint
            └── commands/
                ├── mod.rs
                ├── add.rs
                ├── list.rs
                ├── copy.rs
                ├── pop.rs
                └── pin.rs                       # pin + unpin
```

**Responsibility boundaries:**
- `qcli-core` has **no** I/O outside `storage.rs`. The queue domain is pure, persistence is isolated, so unit tests can exercise the domain without a filesystem.
- `qcli-platform` is the **only** crate allowed to depend on `arboard`, `fd-lock`, and `directories`. Everything OS-specific lives here.
- `qcli-bin` orchestrates: it opens a file lock, loads the queue, mutates it, saves, copies to clipboard. No domain logic in the binary.

---

## Task 1: Initialize Cargo workspace + crate skeletons

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `crates/qcli-core/Cargo.toml`
- Create: `crates/qcli-core/src/lib.rs`
- Create: `crates/qcli-platform/Cargo.toml`
- Create: `crates/qcli-platform/src/lib.rs`
- Create: `crates/qcli-providers/Cargo.toml`
- Create: `crates/qcli-providers/src/lib.rs`
- Create: `crates/qcli-tui/Cargo.toml`
- Create: `crates/qcli-tui/src/lib.rs`
- Create: `crates/qcli-bin/Cargo.toml`
- Create: `crates/qcli-bin/src/main.rs`

- [ ] **Step 1: Write workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/qcli-core",
    "crates/qcli-platform",
    "crates/qcli-providers",
    "crates/qcli-tui",
    "crates/qcli-bin",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/2bb-dev/q-cli"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
clap = { version = "4", features = ["derive"] }
arboard = "3"
fd-lock = "4"
directories = "5"
uuid = { version = "1", features = ["v4", "serde"] }
time = { version = "0.3", features = ["serde", "formatting", "parsing"] }

# dev
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 2: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Write `.gitignore`**

```
/target
**/*.rs.bk
Cargo.lock
.DS_Store
```

Note: we check in `Cargo.lock` for binary crates in general, but the workspace produces a binary and several libs; `Cargo.lock` for the binary crate is meaningful. For now we ignore it to avoid churn during scaffold. The final packaging plan will revisit.

- [ ] **Step 4: Write each crate's `Cargo.toml`**

`crates/qcli-core/Cargo.toml`:
```toml
[package]
name = "qcli-core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
time = { workspace = true }
```

`crates/qcli-platform/Cargo.toml`:
```toml
[package]
name = "qcli-platform"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
thiserror = { workspace = true }
arboard = { workspace = true }
fd-lock = { workspace = true }
directories = { workspace = true }
```

`crates/qcli-providers/Cargo.toml`:
```toml
[package]
name = "qcli-providers"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
```

`crates/qcli-tui/Cargo.toml`:
```toml
[package]
name = "qcli-tui"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
```

`crates/qcli-bin/Cargo.toml`:
```toml
[package]
name = "qcli-bin"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[[bin]]
name = "q"
path = "src/main.rs"

[dependencies]
qcli-core = { path = "../qcli-core" }
qcli-platform = { path = "../qcli-platform" }
clap = { workspace = true }
anyhow = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
assert_cmd = { workspace = true }
predicates = { workspace = true }
tempfile = { workspace = true }
```

- [ ] **Step 5: Write stub `lib.rs` for each library crate**

`crates/qcli-core/src/lib.rs`:
```rust
//! q-cli domain crate: prompt queue, persistence.
```

`crates/qcli-platform/src/lib.rs`:
```rust
//! q-cli platform crate: app dirs, file locking, clipboard.
```

`crates/qcli-providers/src/lib.rs`:
```rust
//! q-cli providers crate (stub — populated in a later plan).
```

`crates/qcli-tui/src/lib.rs`:
```rust
//! q-cli TUI crate (stub — populated in a later plan).
```

- [ ] **Step 6: Write `crates/qcli-bin/src/main.rs` stub**

```rust
fn main() {
    println!("q-cli: scaffold in progress");
}
```

- [ ] **Step 7: Run `cargo build` to verify the workspace compiles**

Run: `cargo build`
Expected: all five crates build cleanly, no errors, no warnings.

- [ ] **Step 8: Run `cargo run -p qcli-bin` to verify the binary**

Run: `cargo run -p qcli-bin`
Expected stdout: `q-cli: scaffold in progress`

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore crates
git commit -m "feat: scaffold cargo workspace and crate skeletons"
```

---

## Task 2: Paths resolution in `qcli-platform`

**Files:**
- Create: `crates/qcli-platform/src/paths.rs`
- Modify: `crates/qcli-platform/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/qcli-platform/src/paths.rs`:
```rust
use std::path::PathBuf;

/// Returns the base directory for q-cli data (queue, config).
/// On macOS: ~/Library/Application Support/q-cli
/// On Linux: $XDG_DATA_HOME/q-cli or ~/.local/share/q-cli
pub fn app_dir() -> std::io::Result<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "2bb", "q-cli")
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory",
        ))?;
    let dir = proj.data_dir().to_path_buf();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns the path to the queue JSON file.
pub fn queue_path() -> std::io::Result<PathBuf> {
    Ok(app_dir()?.join("queue.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dir_is_created_and_returns_absolute_path() {
        let dir = app_dir().expect("app_dir should succeed");
        assert!(dir.is_absolute(), "app_dir must be absolute");
        assert!(dir.exists(), "app_dir must exist after call");
    }

    #[test]
    fn queue_path_ends_with_queue_json() {
        let path = queue_path().expect("queue_path should succeed");
        assert_eq!(path.file_name().and_then(|s| s.to_str()), Some("queue.json"));
    }
}
```

Update `crates/qcli-platform/src/lib.rs`:
```rust
//! q-cli platform crate: app dirs, file locking, clipboard.

pub mod paths;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p qcli-platform paths`
Expected: the test module doesn't exist yet (or: FAIL, compile error if you haven't added `pub mod paths;` yet). Since the implementation is already there in Step 1, this is a build-and-pass — skip ahead if green.

Pedagogical note: this task deviates from strict red-first because paths is a thin wrapper around `directories`. The tests verify the *contract* (absolute + created + right filename), not the body.

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p qcli-platform paths`
Expected: `test tests::app_dir_is_created_and_returns_absolute_path ... ok` and `test tests::queue_path_ends_with_queue_json ... ok`.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-platform
git commit -m "feat(platform): resolve app_dir and queue_path on macOS/Linux"
```

---

## Task 3: File locking helper in `qcli-platform`

**Files:**
- Create: `crates/qcli-platform/src/lock.rs`
- Modify: `crates/qcli-platform/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append `crates/qcli-platform/src/lock.rs`:
```rust
use std::fs::{File, OpenOptions};
use std::path::Path;

/// RAII advisory file lock. Held for the lifetime of the guard.
pub struct FileLock {
    _file: File,
    _guard: fd_lock::RwLockWriteGuard<'static, File>,
}

impl FileLock {
    /// Acquire an exclusive lock on `path`. Creates the file if missing.
    /// Blocks until the lock is available.
    pub fn acquire(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        // fd_lock wraps the File; we need the guard's lifetime tied to the lock.
        // Leak the Box so the guard can be 'static — FileLock owns the leak via drop.
        let lock: &'static mut fd_lock::RwLock<File> =
            Box::leak(Box::new(fd_lock::RwLock::new(
                OpenOptions::new().read(true).write(true).open(path)?,
            )));
        let guard = lock.write()?;
        Ok(FileLock {
            _file: file,
            _guard: guard,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::NamedTempFile;

    #[test]
    fn second_acquire_blocks_until_first_released() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let first = FileLock::acquire(&path).expect("first acquire");

        let path2 = path.clone();
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let _second = FileLock::acquire(&path2).expect("second acquire");
            start.elapsed()
        });

        thread::sleep(Duration::from_millis(150));
        drop(first);

        let elapsed = handle.join().unwrap();
        assert!(
            elapsed >= Duration::from_millis(100),
            "second acquire should have waited, elapsed = {elapsed:?}"
        );
    }
}
```

Update `crates/qcli-platform/src/lib.rs`:
```rust
//! q-cli platform crate: app dirs, file locking, clipboard.

pub mod lock;
pub mod paths;
```

Also add to `crates/qcli-platform/Cargo.toml` dev-dependencies:
```toml
[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p qcli-platform lock`
Expected: PASS. The second thread measurably waits > 100ms before acquiring.

If the `Box::leak` approach feels wrong (it does — it leaks one lock per `FileLock` instance, which is fine for short-lived `q` invocations but grows memory in long-lived processes), revisit after Plan 2 when the TUI needs long-lived locks. For now, one leak per CLI invocation is acceptable and the process exits immediately after.

- [ ] **Step 3: Commit**

```bash
git add crates/qcli-platform
git commit -m "feat(platform): add FileLock with exclusive advisory locking"
```

---

## Task 4: Clipboard trait + `SystemClipboard` impl

**Files:**
- Create: `crates/qcli-platform/src/clipboard.rs`
- Modify: `crates/qcli-platform/src/lib.rs`

- [ ] **Step 1: Write the code + test**

Append `crates/qcli-platform/src/clipboard.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard unavailable: {0}")]
    Unavailable(String),
}

/// Abstraction over a clipboard so callers can swap in a fake for tests.
pub trait Clipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

/// Real system clipboard via `arboard`.
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

/// In-memory fake clipboard for tests.
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
impl Clipboard for FakeClipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.last = Some(text.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clipboard_records_last_set() {
        let mut cb = FakeClipboard::new();
        cb.set_text("hello").unwrap();
        assert_eq!(cb.last.as_deref(), Some("hello"));
    }
}
```

Update `crates/qcli-platform/src/lib.rs`:
```rust
//! q-cli platform crate: app dirs, file locking, clipboard.

pub mod clipboard;
pub mod lock;
pub mod paths;
```

Update `crates/qcli-platform/Cargo.toml` — add a `test-support` feature so consumers can pull `FakeClipboard` in their own tests:
```toml
[features]
test-support = []
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p qcli-platform clipboard`
Expected: `test clipboard::tests::fake_clipboard_records_last_set ... ok`.

We intentionally do **not** test `SystemClipboard` in CI — `arboard` requires a display server on Linux.

- [ ] **Step 3: Commit**

```bash
git add crates/qcli-platform
git commit -m "feat(platform): Clipboard trait with SystemClipboard and FakeClipboard"
```

---

## Task 5: `Prompt` and `PromptId` types

**Files:**
- Create: `crates/qcli-core/src/prompt.rs`
- Create: `crates/qcli-core/src/error.rs`
- Modify: `crates/qcli-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/qcli-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("prompt not found: {0}")]
    NotFound(String),

    #[error("invalid prompt: {0}")]
    Invalid(String),

    #[error("storage error: {0}")]
    Storage(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
```

Create `crates/qcli-core/src/prompt.rs`:
```rust
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptId(pub Uuid);

impl PromptId {
    pub fn new() -> Self {
        PromptId(Uuid::new_v4())
    }

    /// Accepts either the full UUID or a short prefix (min 4 chars).
    /// Returns an error if the input is too short; matching is done by `Queue`.
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
        // Short form: first 8 hex chars.
        write!(f, "{}", &self.0.as_hyphenated().to_string()[..8])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: PromptId,
    pub text: String,
    pub pinned: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl Prompt {
    pub fn new(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(CoreError::Invalid("prompt text is empty".into()));
        }
        Ok(Prompt {
            id: PromptId::new(),
            text,
            pinned: false,
            created_at: OffsetDateTime::now_utc(),
        })
    }

    /// First line, trimmed to 80 chars, for list display.
    pub fn preview(&self) -> String {
        let first_line = self.text.lines().next().unwrap_or("").trim();
        if first_line.chars().count() <= 80 {
            first_line.to_string()
        } else {
            let mut s: String = first_line.chars().take(77).collect();
            s.push_str("...");
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_text() {
        assert!(Prompt::new("").is_err());
        assert!(Prompt::new("   \n\t").is_err());
    }

    #[test]
    fn new_accepts_non_empty_text() {
        let p = Prompt::new("hello world").expect("should succeed");
        assert_eq!(p.text, "hello world");
        assert!(!p.pinned);
    }

    #[test]
    fn preview_uses_first_line_and_truncates_at_80() {
        let p = Prompt::new("first line\nsecond line").unwrap();
        assert_eq!(p.preview(), "first line");

        let long = "a".repeat(100);
        let p = Prompt::new(&long).unwrap();
        let preview = p.preview();
        assert_eq!(preview.chars().count(), 80);
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn prompt_id_display_is_8_hex() {
        let id = PromptId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 8);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn parse_input_rejects_short_ids() {
        assert!(PromptId::parse_input("abc").is_err());
        assert!(PromptId::parse_input("abcd").is_ok());
    }
}
```

Update `crates/qcli-core/src/lib.rs`:
```rust
//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p qcli-core prompt`
Expected: all four tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/qcli-core
git commit -m "feat(core): Prompt and PromptId with preview and empty-text rejection"
```

---

## Task 6: `Queue` domain — add, remove, edit, pin, reorder, clear

**Files:**
- Create: `crates/qcli-core/src/queue.rs`
- Modify: `crates/qcli-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/qcli-core/src/queue.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::prompt::{Prompt, PromptId};

/// Ordered list of prompts. Pinned prompts appear first, in insertion order.
/// Unpinned prompts follow, also in insertion order.
///
/// The queue is the sole owner of ordering. Callers reference prompts by id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Queue {
    /// Flat vector; the invariant `pinned == true` items precede `pinned == false` items
    /// is maintained by every mutating operation.
    prompts: Vec<Prompt>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.prompts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Prompt> {
        self.prompts.iter()
    }

    /// Add a new prompt. Pinned prompts are inserted at the end of the pinned section;
    /// unpinned prompts at the end of the full list.
    pub fn add(&mut self, prompt: Prompt) -> PromptId {
        let id = prompt.id;
        if prompt.pinned {
            let insert_at = self.prompts.iter().position(|p| !p.pinned).unwrap_or(self.prompts.len());
            self.prompts.insert(insert_at, prompt);
        } else {
            self.prompts.push(prompt);
        }
        id
    }

    /// Resolve a user-supplied id string (full UUID or prefix >= 4 chars) to a `PromptId`.
    /// Errors if the prefix is ambiguous or nothing matches.
    pub fn resolve(&self, input: &str) -> Result<PromptId> {
        let prefix = PromptId::parse_input(input)?;
        let matches: Vec<_> = self
            .prompts
            .iter()
            .filter(|p| p.id.0.as_hyphenated().to_string().starts_with(&prefix)
                || p.id.to_string().starts_with(&prefix))
            .collect();
        match matches.len() {
            0 => Err(CoreError::NotFound(prefix)),
            1 => Ok(matches[0].id),
            _ => Err(CoreError::Invalid(format!("ambiguous id prefix: {prefix}"))),
        }
    }

    pub fn get(&self, id: PromptId) -> Option<&Prompt> {
        self.prompts.iter().find(|p| p.id == id)
    }

    /// Remove the prompt with `id` and return it.
    pub fn remove(&mut self, id: PromptId) -> Result<Prompt> {
        let pos = self
            .prompts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        Ok(self.prompts.remove(pos))
    }

    /// Replace the text of the prompt with `id`.
    pub fn edit(&mut self, id: PromptId, new_text: impl Into<String>) -> Result<()> {
        let new_text = new_text.into();
        if new_text.trim().is_empty() {
            return Err(CoreError::Invalid("prompt text is empty".into()));
        }
        let p = self
            .prompts
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        p.text = new_text;
        Ok(())
    }

    /// Set the pinned flag on `id`. Moves the prompt to the correct section.
    pub fn set_pinned(&mut self, id: PromptId, pinned: bool) -> Result<()> {
        let pos = self
            .prompts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        let mut p = self.prompts.remove(pos);
        p.pinned = pinned;
        let _ = self.add(p);
        Ok(())
    }

    /// Return the next prompt to act on: the first pinned, or the first unpinned if none pinned.
    pub fn peek_next(&self) -> Option<&Prompt> {
        self.prompts.first()
    }

    /// Pop the next prompt (unpinned only). Pinned prompts are never popped.
    /// Returns `None` if the head is pinned or the queue is empty.
    pub fn pop_next_unpinned(&mut self) -> Option<Prompt> {
        let pos = self.prompts.iter().position(|p| !p.pinned)?;
        Some(self.prompts.remove(pos))
    }

    pub fn clear(&mut self) {
        self.prompts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> Prompt {
        Prompt::new(text).unwrap()
    }

    #[test]
    fn add_appends_unpinned_at_end() {
        let mut q = Queue::new();
        q.add(p("a"));
        q.add(p("b"));
        assert_eq!(
            q.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn pinned_prompts_sort_before_unpinned() {
        let mut q = Queue::new();
        q.add(p("one"));
        q.add(p("two"));
        let mut pinned = p("zero");
        pinned.pinned = true;
        q.add(pinned);
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["zero", "one", "two"]);
    }

    #[test]
    fn remove_returns_the_prompt() {
        let mut q = Queue::new();
        let id = q.add(p("foo"));
        let removed = q.remove(id).unwrap();
        assert_eq!(removed.text, "foo");
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn edit_replaces_text() {
        let mut q = Queue::new();
        let id = q.add(p("old"));
        q.edit(id, "new").unwrap();
        assert_eq!(q.get(id).unwrap().text, "new");
    }

    #[test]
    fn edit_rejects_empty() {
        let mut q = Queue::new();
        let id = q.add(p("old"));
        assert!(q.edit(id, "").is_err());
    }

    #[test]
    fn set_pinned_true_moves_to_pinned_section() {
        let mut q = Queue::new();
        q.add(p("a"));
        let id = q.add(p("b"));
        q.add(p("c"));
        q.set_pinned(id, true).unwrap();
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["b", "a", "c"]);
    }

    #[test]
    fn set_pinned_false_moves_to_end() {
        let mut q = Queue::new();
        let mut pinned = p("x");
        pinned.pinned = true;
        let id = q.add(pinned);
        q.add(p("y"));
        q.set_pinned(id, false).unwrap();
        let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["y", "x"]);
    }

    #[test]
    fn pop_next_unpinned_skips_pinned_head() {
        let mut q = Queue::new();
        let mut pinned = p("stay");
        pinned.pinned = true;
        q.add(pinned);
        q.add(p("go"));
        let popped = q.pop_next_unpinned().unwrap();
        assert_eq!(popped.text, "go");
        assert_eq!(q.len(), 1);
        assert_eq!(q.iter().next().unwrap().text, "stay");
    }

    #[test]
    fn pop_next_unpinned_returns_none_when_only_pinned() {
        let mut q = Queue::new();
        let mut pinned = p("only");
        pinned.pinned = true;
        q.add(pinned);
        assert!(q.pop_next_unpinned().is_none());
    }

    #[test]
    fn resolve_by_full_id_succeeds() {
        let mut q = Queue::new();
        let id = q.add(p("hello"));
        let full = id.0.as_hyphenated().to_string();
        assert_eq!(q.resolve(&full).unwrap(), id);
    }

    #[test]
    fn resolve_by_short_prefix_succeeds() {
        let mut q = Queue::new();
        let id = q.add(p("hello"));
        let short = id.to_string(); // 8 hex chars
        assert_eq!(q.resolve(&short).unwrap(), id);
    }

    #[test]
    fn resolve_reports_not_found() {
        let q = Queue::new();
        assert!(matches!(q.resolve("abcd"), Err(CoreError::NotFound(_))));
    }

    #[test]
    fn clear_empties_queue() {
        let mut q = Queue::new();
        q.add(p("a"));
        q.add(p("b"));
        q.clear();
        assert!(q.is_empty());
    }
}
```

Update `crates/qcli-core/src/lib.rs`:
```rust
//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;
pub mod queue;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
pub use queue::Queue;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p qcli-core queue`
Expected: 12 tests, all pass.

- [ ] **Step 3: Commit**

```bash
git add crates/qcli-core
git commit -m "feat(core): Queue with add, remove, edit, pin, pop, resolve"
```

---

## Task 7: JSON persistence in `qcli-core`

**Files:**
- Create: `crates/qcli-core/src/storage.rs`
- Modify: `crates/qcli-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/qcli-core/src/storage.rs`:
```rust
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::queue::Queue;

/// Schema version embedded in the on-disk file. Increment on breaking changes.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct QueueFile {
    schema: u32,
    queue: Queue,
}

/// Load the queue from `path`. Returns an empty queue if the file does not exist.
pub fn load(path: &Path) -> Result<Queue> {
    if !path.exists() {
        return Ok(Queue::new());
    }
    let data = fs::read_to_string(path)?;
    if data.trim().is_empty() {
        return Ok(Queue::new());
    }
    let parsed: QueueFile = serde_json::from_str(&data)?;
    Ok(parsed.queue)
}

/// Save the queue to `path` atomically: write to `path.tmp`, then rename.
pub fn save(path: &Path, queue: &Queue) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let file = QueueFile {
        schema: SCHEMA_VERSION,
        queue: queue.clone(),
    };
    let serialized = serde_json::to_string_pretty(&file)?;
    fs::write(&tmp, serialized)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::Prompt;
    use tempfile::TempDir;

    #[test]
    fn load_missing_file_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        let q = load(&path).unwrap();
        assert!(q.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");

        let mut q = Queue::new();
        q.add(Prompt::new("hello").unwrap());
        q.add(Prompt::new("world").unwrap());

        save(&path, &q).unwrap();
        let loaded = load(&path).unwrap();

        let original: Vec<_> = q.iter().map(|p| p.text.clone()).collect();
        let after: Vec<_> = loaded.iter().map(|p| p.text.clone()).collect();
        assert_eq!(original, after);
    }

    #[test]
    fn save_is_atomic_no_tmp_left_behind() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        let q = Queue::new();
        save(&path, &q).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn load_empty_file_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        std::fs::write(&path, "").unwrap();
        let q = load(&path).unwrap();
        assert!(q.is_empty());
    }

    #[test]
    fn schema_version_is_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Queue::new()).unwrap();
        let data = std::fs::read_to_string(&path).unwrap();
        assert!(data.contains("\"schema\": 1"));
    }
}
```

Add dev-dependency to `crates/qcli-core/Cargo.toml`:
```toml
[dev-dependencies]
tempfile = { workspace = true }
```

Update `crates/qcli-core/src/lib.rs`:
```rust
//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;
pub mod queue;
pub mod storage;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
pub use queue::Queue;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p qcli-core storage`
Expected: 5 tests, all pass.

- [ ] **Step 3: Commit**

```bash
git add crates/qcli-core
git commit -m "feat(core): atomic JSON persistence with schema versioning"
```

---

## Task 8: Binary scaffold with `clap` subcommand routing

**Files:**
- Modify: `crates/qcli-bin/src/main.rs`
- Create: `crates/qcli-bin/src/commands/mod.rs`

- [ ] **Step 1: Replace `main.rs` with clap scaffold**

`crates/qcli-bin/src/main.rs`:
```rust
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "q", version, about = "Terminal prompt queue")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a new prompt to the queue.
    Add {
        /// Prompt text. If omitted, read from stdin.
        text: Option<String>,
        /// Add as pinned.
        #[arg(long)]
        pin: bool,
    },
    /// List all prompts.
    List {
        /// Emit JSON instead of human-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Copy a prompt to the clipboard.
    Copy {
        /// Prompt id (full or prefix >= 4 chars). Mutually exclusive with --next.
        id: Option<String>,
        /// Copy the next unpinned prompt.
        #[arg(long, conflicts_with = "id")]
        next: bool,
        /// Print to stdout instead of the system clipboard.
        #[arg(long)]
        stdout: bool,
    },
    /// Pop a prompt (copy + remove). Pinned prompts are never popped.
    Pop {
        /// Prompt id (full or prefix >= 4 chars). Mutually exclusive with --next.
        id: Option<String>,
        /// Pop the next unpinned prompt.
        #[arg(long, conflicts_with = "id")]
        next: bool,
        /// Print to stdout instead of the system clipboard.
        #[arg(long)]
        stdout: bool,
    },
    /// Pin a prompt.
    Pin { id: String },
    /// Unpin a prompt.
    Unpin { id: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Add { text, pin } => commands::add::run(text, pin),
        Command::List { json } => commands::list::run(json),
        Command::Copy { id, next, stdout } => commands::copy::run(id, next, stdout),
        Command::Pop { id, next, stdout } => commands::pop::run(id, next, stdout),
        Command::Pin { id } => commands::pin::run(&id, true),
        Command::Unpin { id } => commands::pin::run(&id, false),
    }
}
```

Create `crates/qcli-bin/src/commands/mod.rs`:
```rust
pub mod add;
pub mod copy;
pub mod list;
pub mod pin;
pub mod pop;

use anyhow::Result;
use qcli_core::Queue;
use qcli_core::storage;
use qcli_platform::lock::FileLock;
use qcli_platform::paths;
use std::path::PathBuf;

/// Convenience: resolve the queue path, acquire a lock, load the queue.
pub(crate) fn open_queue() -> Result<(Queue, FileLock, PathBuf)> {
    let path = paths::queue_path()?;
    let lock = FileLock::acquire(&path.with_extension("lock"))?;
    let queue = storage::load(&path)?;
    Ok((queue, lock, path))
}

pub(crate) fn save_queue(path: &std::path::Path, queue: &Queue) -> Result<()> {
    storage::save(path, queue)?;
    Ok(())
}
```

Create placeholder files so the module tree compiles:

`crates/qcli-bin/src/commands/add.rs`:
```rust
use anyhow::Result;

pub fn run(_text: Option<String>, _pin: bool) -> Result<()> {
    anyhow::bail!("add not yet implemented")
}
```

`crates/qcli-bin/src/commands/list.rs`:
```rust
use anyhow::Result;

pub fn run(_json: bool) -> Result<()> {
    anyhow::bail!("list not yet implemented")
}
```

`crates/qcli-bin/src/commands/copy.rs`:
```rust
use anyhow::Result;

pub fn run(_id: Option<String>, _next: bool, _stdout: bool) -> Result<()> {
    anyhow::bail!("copy not yet implemented")
}
```

`crates/qcli-bin/src/commands/pop.rs`:
```rust
use anyhow::Result;

pub fn run(_id: Option<String>, _next: bool, _stdout: bool) -> Result<()> {
    anyhow::bail!("pop not yet implemented")
}
```

`crates/qcli-bin/src/commands/pin.rs`:
```rust
use anyhow::Result;

pub fn run(_id: &str, _pinned: bool) -> Result<()> {
    anyhow::bail!("pin/unpin not yet implemented")
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p qcli-bin`
Expected: compiles cleanly.

- [ ] **Step 3: Verify CLI help output**

Run: `cargo run -p qcli-bin -- --help`
Expected: shows `add`, `list`, `copy`, `pop`, `pin`, `unpin` subcommands.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): clap scaffold with subcommand routing stubs"
```

---

## Task 9: `q add` subcommand

**Files:**
- Modify: `crates/qcli-bin/src/commands/add.rs`
- Create: `crates/qcli-bin/tests/cli_add.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/qcli-bin/tests/cli_add.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    // Redirect app dir to a temp location via HOME / XDG_DATA_HOME.
    cmd.env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"));
    cmd
}

#[test]
fn add_with_arg_creates_prompt_and_list_shows_it() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "hello world"]).assert().success();
    q(&home)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("hello world"));
}

#[test]
fn add_from_stdin_when_no_arg() {
    let home = TempDir::new().unwrap();
    q(&home)
        .args(["add"])
        .write_stdin("from stdin\n")
        .assert()
        .success();
    q(&home)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("from stdin"));
}

#[test]
fn add_pin_flag_marks_prompt_pinned() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "pinned one", "--pin"]).assert().success();
    q(&home)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("[P]").and(predicates::str::contains("pinned one")));
}

#[test]
fn add_empty_text_fails() {
    let home = TempDir::new().unwrap();
    q(&home)
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("empty"));
}
```

- [ ] **Step 2: Implement `add`**

Replace `crates/qcli-bin/src/commands/add.rs`:
```rust
use anyhow::Result;
use qcli_core::Prompt;

use super::{open_queue, save_queue};

pub fn run(text: Option<String>, pin: bool) -> Result<()> {
    let text = match text {
        Some(t) => t,
        None => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            buf
        }
    };
    let (mut queue, _lock, path) = open_queue()?;
    let mut prompt = Prompt::new(text)?;
    prompt.pinned = pin;
    let id = queue.add(prompt);
    save_queue(&path, &queue)?;
    println!("added {id}");
    Ok(())
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-bin --test cli_add`
Expected: the first three tests compile but fail because `list` is not yet implemented. The fourth (`add_empty_text_fails`) passes. We'll re-run after Task 10 and confirm all four pass.

For now, accept: **`add_empty_text_fails` passes**, others are gated on `list`.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q add reads text from arg or stdin and persists"
```

---

## Task 10: `q list` subcommand

**Files:**
- Modify: `crates/qcli-bin/src/commands/list.rs`
- Create: `crates/qcli-bin/tests/cli_list.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/qcli-bin/tests/cli_list.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"));
    cmd
}

#[test]
fn list_on_empty_queue_prints_empty_notice() {
    let home = TempDir::new().unwrap();
    q(&home)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn list_shows_id_preview_and_pinned_marker() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "first"]).assert().success();
    q(&home).args(["add", "second", "--pin"]).assert().success();
    q(&home)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("second"))
        .stdout(predicates::str::contains("first"))
        .stdout(predicates::str::contains("[P]"));
}

#[test]
fn list_json_emits_valid_json_array() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "hello"]).assert().success();
    let output = q(&home).args(["list", "--json"]).assert().success().get_output().clone();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["text"], "hello");
}
```

- [ ] **Step 2: Implement `list`**

Replace `crates/qcli-bin/src/commands/list.rs`:
```rust
use anyhow::Result;

use super::open_queue;

pub fn run(json: bool) -> Result<()> {
    let (queue, _lock, _path) = open_queue()?;
    if json {
        let arr: Vec<_> = queue.iter().collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }
    if queue.is_empty() {
        println!("(queue empty)");
        return Ok(());
    }
    for p in queue.iter() {
        let marker = if p.pinned { "[P]" } else { "   " };
        println!("{marker} {} {}", p.id, p.preview());
    }
    Ok(())
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-bin`
Expected: `cli_list` tests pass, and `cli_add` tests now *all* pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q list with human and --json output"
```

---

## Task 11: `q copy` subcommand

**Files:**
- Modify: `crates/qcli-bin/src/commands/copy.rs`
- Create: `crates/qcli-bin/tests/cli_copy.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/qcli-bin/tests/cli_copy.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"));
    cmd
}

#[test]
fn copy_next_stdout_prints_first_prompt_text() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "alpha"]).assert().success();
    q(&home).args(["add", "beta"]).assert().success();
    q(&home)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("alpha"));
}

#[test]
fn copy_by_id_prefix_stdout_prints_that_prompt() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "target"]).assert().success();
    let list_output = q(&home).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    let id = stdout.split_whitespace().find(|s| s.len() == 8).unwrap().to_string();

    q(&home)
        .args(["copy", &id, "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("target"));
}

#[test]
fn copy_does_not_remove_the_prompt() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "keep me"]).assert().success();
    q(&home).args(["copy", "--next", "--stdout"]).assert().success();
    q(&home)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("keep me"));
}

#[test]
fn copy_without_id_or_next_fails() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "x"]).assert().success();
    q(&home)
        .args(["copy"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--next"));
}

#[test]
fn copy_empty_queue_with_next_fails() {
    let home = TempDir::new().unwrap();
    q(&home)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("empty"));
}
```

- [ ] **Step 2: Implement `copy`**

Replace `crates/qcli-bin/src/commands/copy.rs`:
```rust
use anyhow::{anyhow, Result};
use qcli_platform::clipboard::{Clipboard, SystemClipboard};

use super::open_queue;

pub fn run(id: Option<String>, next: bool, stdout: bool) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    let (queue, _lock, _path) = open_queue()?;
    let prompt = if let Some(id) = id {
        let resolved = queue.resolve(&id)?;
        queue.get(resolved).cloned().ok_or_else(|| anyhow!("prompt missing after resolve"))?
    } else {
        queue
            .peek_next()
            .cloned()
            .ok_or_else(|| anyhow!("queue is empty"))?
    };

    if stdout {
        print!("{}", prompt.text);
        return Ok(());
    }
    let mut cb = SystemClipboard::new()?;
    cb.set_text(&prompt.text)?;
    eprintln!("copied {} ({} chars)", prompt.id, prompt.text.chars().count());
    Ok(())
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-bin --test cli_copy`
Expected: all 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q copy with --next, --stdout, and id prefix resolution"
```

---

## Task 12: `q pop` subcommand

**Files:**
- Modify: `crates/qcli-bin/src/commands/pop.rs`
- Create: `crates/qcli-bin/tests/cli_pop.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/qcli-bin/tests/cli_pop.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"));
    cmd
}

#[test]
fn pop_next_stdout_prints_and_removes_first_unpinned() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "first"]).assert().success();
    q(&home).args(["add", "second"]).assert().success();
    q(&home)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("first"));
    q(&home)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("second"))
        .stdout(predicates::str::contains("first").not());
}

#[test]
fn pop_skips_pinned_prompts() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "pinned", "--pin"]).assert().success();
    q(&home).args(["add", "floating"]).assert().success();
    q(&home)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .stdout(predicates::str::starts_with("floating"));
    q(&home)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("pinned"))
        .stdout(predicates::str::contains("floating").not());
}

#[test]
fn pop_by_id_removes_that_prompt_even_if_pinned() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "target", "--pin"]).assert().success();
    let list_output = q(&home).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    let id = stdout.split_whitespace().find(|s| s.len() == 8).unwrap().to_string();

    q(&home).args(["pop", &id, "--stdout"]).assert().success();
    q(&home)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn pop_next_on_empty_queue_fails() {
    let home = TempDir::new().unwrap();
    q(&home)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no unpinned"));
}
```

- [ ] **Step 2: Implement `pop`**

Replace `crates/qcli-bin/src/commands/pop.rs`:
```rust
use anyhow::{anyhow, Result};
use qcli_platform::clipboard::{Clipboard, SystemClipboard};

use super::{open_queue, save_queue};

pub fn run(id: Option<String>, next: bool, stdout: bool) -> Result<()> {
    if id.is_none() && !next {
        return Err(anyhow!("specify a prompt id or --next"));
    }
    let (mut queue, _lock, path) = open_queue()?;

    let popped = if let Some(id) = id {
        let resolved = queue.resolve(&id)?;
        queue.remove(resolved)?
    } else {
        queue
            .pop_next_unpinned()
            .ok_or_else(|| anyhow!("no unpinned prompts to pop"))?
    };

    save_queue(&path, &queue)?;

    if stdout {
        print!("{}", popped.text);
    } else {
        let mut cb = SystemClipboard::new()?;
        cb.set_text(&popped.text)?;
        eprintln!("popped {} ({} chars)", popped.id, popped.text.chars().count());
    }
    Ok(())
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-bin --test cli_pop`
Expected: all 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q pop with pinned-skip semantics and id override"
```

---

## Task 13: `q pin` and `q unpin` subcommands

**Files:**
- Modify: `crates/qcli-bin/src/commands/pin.rs`
- Create: `crates/qcli-bin/tests/cli_pin.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/qcli-bin/tests/cli_pin.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"));
    cmd
}

fn short_id_of(home: &TempDir, text_marker: &str) -> String {
    let output = q(home).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout
        .lines()
        .find(|line| line.contains(text_marker))
        .unwrap()
        .split_whitespace()
        .find(|s| s.len() == 8)
        .unwrap()
        .to_string()
}

#[test]
fn pin_moves_prompt_to_top() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "first"]).assert().success();
    q(&home).args(["add", "second"]).assert().success();
    let id = short_id_of(&home, "second");

    q(&home).args(["pin", &id]).assert().success();
    let output = q(&home).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let second_line_idx = stdout.lines().position(|l| l.contains("second")).unwrap();
    let first_line_idx = stdout.lines().position(|l| l.contains("first")).unwrap();
    assert!(second_line_idx < first_line_idx, "pinned 'second' should come first");
}

#[test]
fn unpin_moves_prompt_to_unpinned_section() {
    let home = TempDir::new().unwrap();
    q(&home).args(["add", "alpha", "--pin"]).assert().success();
    q(&home).args(["add", "beta"]).assert().success();
    let id = short_id_of(&home, "alpha");

    q(&home).args(["unpin", &id]).assert().success();
    let output = q(&home).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let alpha_line = stdout.lines().find(|l| l.contains("alpha")).unwrap();
    assert!(!alpha_line.contains("[P]"), "alpha should no longer be pinned");
}

#[test]
fn pin_unknown_id_fails() {
    let home = TempDir::new().unwrap();
    q(&home)
        .args(["pin", "deadbeef"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));
}
```

- [ ] **Step 2: Implement `pin`/`unpin`**

Replace `crates/qcli-bin/src/commands/pin.rs`:
```rust
use anyhow::Result;

use super::{open_queue, save_queue};

pub fn run(id: &str, pinned: bool) -> Result<()> {
    let (mut queue, _lock, path) = open_queue()?;
    let resolved = queue.resolve(id)?;
    queue.set_pinned(resolved, pinned)?;
    save_queue(&path, &queue)?;
    println!(
        "{} {resolved}",
        if pinned { "pinned" } else { "unpinned" }
    );
    Ok(())
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-bin --test cli_pin`
Expected: all 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q pin and q unpin with id prefix resolution"
```

---

## Task 14: README with install + usage

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write the README**

`README.md`:
````markdown
# q-cli

Terminal-native prompt queue for power users. Keyboard-first TUI + scriptable CLI.

Status: **v0 scaffold**. This repo currently ships the workspace skeleton and the scriptable `q` CLI (add / list / copy / pop / pin / unpin). The TUI and LLM-provider integrations are tracked in later plans.

## Install (development)

```sh
git clone git@github.com:2bb-dev/q-cli.git
cd q-cli
cargo install --path crates/qcli-bin
```

The binary is called `q`.

## Usage

```sh
q add "refactor the queue module"
q add --pin "always remember: prefer surgical diffs"
q list
q copy --next                # copy first prompt to system clipboard
q copy --next --stdout       # print to stdout (pipeable)
q pop --next                 # copy + remove first unpinned prompt
q pop <id>                   # remove a specific prompt
q pin <id>
q unpin <id>
```

Prompt ids accept a short 8-char prefix or the full UUID.

## Data location

- macOS: `~/Library/Application Support/q-cli/queue.json`
- Linux: `$XDG_DATA_HOME/q-cli/queue.json` or `~/.local/share/q-cli/queue.json`

## Roadmap

See the implementation plans in `docs/superpowers/plans/` for the TUI shell, provider integrations (OpenAI, Anthropic, Codex), and packaging.
````

- [ ] **Step 2: Verify the full test suite is green**

Run: `cargo test --workspace`
Expected: all tests pass, no warnings.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: README with install, usage, data location"
```

---

## Task 15: Final verification + push

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 2: Run fmt check**

Run: `cargo fmt --all -- --check`
Expected: no output (fully formatted).

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 4: Manually exercise the tool**

Pick a throwaway home so you don't pollute real state:

```sh
HOME=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d) cargo run -p qcli-bin -- add "smoke test"
HOME=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d) cargo run -p qcli-bin -- list
```

Expected: add reports an id, list shows the prompt with an `[ ]` marker.

- [ ] **Step 5: Push the branch**

```bash
git push -u origin main
```

---

## Out of scope for this plan (pointers for later plans)

- **TUI shell**: ratatui + crossterm, focus model, key handling, async task integration. Plan 2.
- **Providers + keychain + `q upgrade`**: OpenAI API, Anthropic, `keyring` crate, prompt upgrade flow. Plan 3.
- **Codex integration**: binary detection, auth check, subscription upgrade flow. Plan 4.
- **`q doctor` + macOS packaging**: installer, release artifacts, launch speed audit. Plan 5.
- **Linux support**, **file references**, **audio transcription**: Plans 6+.

Every later plan should assume the queue domain, persistence, clipboard abstraction, and `open_queue()` helper already exist — build on them rather than reinventing.
