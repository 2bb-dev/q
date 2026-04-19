# q-cli Queue Domain + Scriptable CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the queue domain (`qcli-core`) and the scriptable `q` binary (add / list / copy / pop / pin / unpin) on top of the already-migrated Rust backend. No TUI in this plan — the TUI lives in `2026-04-19-qcli-tui.md`.

**Architecture:** The workspace is already scaffolded with five crates and a fully-ported provider/codex/platform layer. This plan fills the one remaining library crate (`qcli-core`) and wires it into the binary (`qcli-bin`) through a thin `clap` subcommand surface. The binary opens a file lock, loads the queue, mutates it, saves, and copies text to the system clipboard using `qcli-platform::clipboard`. No domain logic in the binary.

**Tech Stack:** Rust 2021, `serde` + `serde_json` for persistence, `clap` (derive) for CLI, `arboard` for clipboard (already in `qcli-platform`), `fd-lock` for locking (already in `qcli-platform`), `uuid` + `chrono` for prompt IDs and timestamps. Tests: unit tests via `#[test]`, CLI integration tests via `assert_cmd` + `tempfile` + `predicates`.

---

## Workspace state at the start of this plan

**Already built — do not rewrite.** Read `git log --oneline` and these files for details, but do not modify them in this plan unless a task explicitly says so.

### `crates/qcli-platform` — ready

| Module | Public API you will use |
|---|---|
| `paths` | `app_dir() -> io::Result<PathBuf>`, `queue_path()`, `config_path()`, `images_dir()`. Honors `QCLI_APP_DIR` env override — **this is the hook integration tests use**. |
| `lock` | `FileLock::acquire(path) -> io::Result<FileLock>` — advisory exclusive lock, RAII. Released on drop. |
| `clipboard` | `Clipboard` trait with `set_text(&mut self, text: &str)`, `SystemClipboard::new()`, and `FakeClipboard` (for tests, behind `#[cfg(any(test, feature = "test-support"))]`). |
| `images` | `save_image`, `delete_image`, `delete_all_images`, `copy_image_to_clipboard`. **Dormant in v1** — ignore. |

### `crates/qcli-providers` — ready, dormant in v1

| Module | Purpose |
|---|---|
| `upgrade` | `run_openai_compatible_upgrade`, `run_anthropic_upgrade`, `upgrade_prompt` dispatcher. Ignore in this plan. |
| `models` | `list_provider_models` with `ModelDescriptor` / `ModelListResponse`. Ignore in this plan. |
| `codex` | Binary resolution, `codex_auth_status`, `start_codex_device_auth`, `run_codex_upgrade`, ANSI stripping. Ignore in this plan. |
| (crate root) | `META_PROMPT`, `bearer_request`, `openai_models_url`, `ollama_tags_url`. Ignore in this plan. |

All of this is ported from `a3io/q:src-tauri/src/lib.rs` with `#[tauri::command]` removed and `tauri::AppHandle` / `State<..>` replaced by plain function parameters. Dormant (marked `#[allow(dead_code)]` at module level), but compiles and has its own tests.

### `crates/qcli-core` — empty placeholder

Tasks 1–3 fill it.

### `crates/qcli-bin` — prints a one-liner

Tasks 4–9 turn it into the real `q` binary.

### `crates/qcli-tui` — empty placeholder

Plan `2026-04-19-qcli-tui.md` fills it.

---

## File Structure (what this plan creates)

```
crates/qcli-core/src/
├── lib.rs                                 # modify — public re-exports
├── error.rs                               # create — CoreError
├── prompt.rs                              # create — Prompt, PromptId
├── queue.rs                               # create — Queue ops
└── storage.rs                             # create — JSON persistence

crates/qcli-bin/src/
├── main.rs                                # modify — clap entrypoint
└── commands/
    ├── mod.rs                             # create — open_queue helper
    ├── add.rs                             # create
    ├── list.rs                            # create
    ├── copy.rs                            # create
    ├── pop.rs                             # create
    └── pin.rs                             # create — pin + unpin

crates/qcli-bin/tests/
├── cli_add.rs                             # create
├── cli_list.rs                            # create
├── cli_copy.rs                            # create
├── cli_pop.rs                             # create
└── cli_pin.rs                             # create

README.md                                  # create — install + usage
```

---

## Task 1: `Prompt`, `PromptId`, and `CoreError`

**Files:**
- Create: `crates/qcli-core/src/error.rs`
- Create: `crates/qcli-core/src/prompt.rs`
- Modify: `crates/qcli-core/src/lib.rs`

**Source of truth:** JS shape lives at `a3io/q:src/main.js` around line 827 (`{ id, text, pinned: false, images: [] }`). We match `id`, `text`, `pinned`. We add `created_at` (Rust benefits from stable ordering; JS didn't need it). We omit `images` per the text-first scope in `q#11`.

- [ ] **Step 1: Write `crates/qcli-core/src/error.rs`**

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

- [ ] **Step 2: Write `crates/qcli-core/src/prompt.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptId(pub Uuid);

impl PromptId {
    pub fn new() -> Self {
        PromptId(Uuid::new_v4())
    }

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
        write!(f, "{}", &self.0.as_hyphenated().to_string()[..8])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: PromptId,
    pub text: String,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
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
            created_at: Utc::now(),
        })
    }

    /// First line trimmed to 80 chars, for list display.
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
    fn prompt_id_display_is_8_chars() {
        let id = PromptId::new();
        let s = id.to_string();
        assert_eq!(s.chars().count(), 8);
    }

    #[test]
    fn parse_input_rejects_short_ids() {
        assert!(PromptId::parse_input("abc").is_err());
        assert!(PromptId::parse_input("abcd").is_ok());
    }
}
```

- [ ] **Step 3: Update `crates/qcli-core/src/lib.rs`**

```rust
//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p qcli-core`
Expected: 5 tests, all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/qcli-core
git commit -m "feat(core): Prompt, PromptId, and CoreError"
```

---

## Task 2: `Queue` — add, remove, edit, pin, pop, resolve, clear

**Files:**
- Create: `crates/qcli-core/src/queue.rs`
- Modify: `crates/qcli-core/src/lib.rs`

**Source of truth:** Behavior matches `a3io/q:src/main.js`. Pinned prompts sort before unpinned, preserving insertion order within each group. Pop-on-copy (`copy()` in main.js around line 906) skips pinned prompts entirely (`src/main.js:883` — `if (!prompt || prompt.pinned) return`).

- [ ] **Step 1: Write the module**

`crates/qcli-core/src/queue.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::prompt::{Prompt, PromptId};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Queue {
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

    /// Insertion rules:
    ///   pinned == true  → end of pinned section (before first unpinned)
    ///   pinned == false → end of full list
    pub fn add(&mut self, prompt: Prompt) -> PromptId {
        let id = prompt.id;
        if prompt.pinned {
            let insert_at = self
                .prompts
                .iter()
                .position(|p| !p.pinned)
                .unwrap_or(self.prompts.len());
            self.prompts.insert(insert_at, prompt);
        } else {
            self.prompts.push(prompt);
        }
        id
    }

    /// Resolve a user-supplied id string (full UUID or prefix >= 4 chars).
    pub fn resolve(&self, input: &str) -> Result<PromptId> {
        let prefix = PromptId::parse_input(input)?;
        let matches: Vec<_> = self
            .prompts
            .iter()
            .filter(|p| {
                p.id.0.as_hyphenated().to_string().starts_with(&prefix)
                    || p.id.to_string().starts_with(&prefix)
            })
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

    pub fn remove(&mut self, id: PromptId) -> Result<Prompt> {
        let pos = self
            .prompts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        Ok(self.prompts.remove(pos))
    }

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

    /// Head of the queue: first pinned if any, else first unpinned, else None.
    pub fn peek_next(&self) -> Option<&Prompt> {
        self.prompts.first()
    }

    /// Pop the first unpinned prompt. Returns None if all prompts are pinned.
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
    fn pop_next_unpinned_skips_pinned_head() {
        let mut q = Queue::new();
        let mut pinned = p("stay");
        pinned.pinned = true;
        q.add(pinned);
        q.add(p("go"));
        let popped = q.pop_next_unpinned().unwrap();
        assert_eq!(popped.text, "go");
        assert_eq!(q.len(), 1);
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
        let short = id.to_string();
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

- [ ] **Step 2: Update `crates/qcli-core/src/lib.rs`**

```rust
//! q-cli domain crate: prompt queue, persistence.

pub mod error;
pub mod prompt;
pub mod queue;

pub use error::{CoreError, Result};
pub use prompt::{Prompt, PromptId};
pub use queue::Queue;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-core queue`
Expected: 12 tests, all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-core
git commit -m "feat(core): Queue ops with pin/pop/resolve semantics"
```

---

## Task 3: JSON persistence with schema versioning

**Files:**
- Create: `crates/qcli-core/src/storage.rs`
- Modify: `crates/qcli-core/src/lib.rs`

- [ ] **Step 1: Write the module**

`crates/qcli-core/src/storage.rs`:
```rust
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::queue::Queue;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct QueueFile {
    schema: u32,
    queue: Queue,
}

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

/// Atomic save: write to `<path>.tmp`, then rename.
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
        assert!(load(&path).unwrap().is_empty());
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
        let a: Vec<_> = q.iter().map(|p| p.text.clone()).collect();
        let b: Vec<_> = loaded.iter().map(|p| p.text.clone()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn save_is_atomic_no_tmp_left_behind() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Queue::new()).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn load_empty_file_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        fs::write(&path, "").unwrap();
        assert!(load(&path).unwrap().is_empty());
    }

    #[test]
    fn schema_version_is_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queue.json");
        save(&path, &Queue::new()).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"schema\": 1"));
    }
}
```

- [ ] **Step 2: Update `crates/qcli-core/src/lib.rs`**

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

- [ ] **Step 3: Run the tests**

Run: `cargo test -p qcli-core storage`
Expected: 5 tests, all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-core
git commit -m "feat(core): atomic JSON persistence with schema versioning"
```

---

## Task 4: Binary scaffold — `clap` subcommand routing

**Files:**
- Modify: `crates/qcli-bin/src/main.rs`
- Create: `crates/qcli-bin/src/commands/mod.rs`
- Create: `crates/qcli-bin/src/commands/{add,list,copy,pop,pin}.rs`

- [ ] **Step 1: Write `crates/qcli-bin/src/main.rs`**

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
        id: Option<String>,
        #[arg(long, conflicts_with = "id")]
        next: bool,
        #[arg(long)]
        stdout: bool,
    },
    /// Pop a prompt (copy + remove). Pinned prompts are never popped when using --next.
    Pop {
        id: Option<String>,
        #[arg(long, conflicts_with = "id")]
        next: bool,
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

- [ ] **Step 2: Write `crates/qcli-bin/src/commands/mod.rs`**

```rust
pub mod add;
pub mod copy;
pub mod list;
pub mod pin;
pub mod pop;

use anyhow::Result;
use qcli_core::{storage, Queue};
use qcli_platform::lock::FileLock;
use qcli_platform::paths;
use std::path::PathBuf;

/// Lock the queue file, load the queue, and return the bundle.
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

- [ ] **Step 3: Write placeholder command files so the module tree compiles**

Each of `add.rs`, `list.rs`, `copy.rs`, `pop.rs`, `pin.rs` starts as:
```rust
use anyhow::Result;

// Argument list varies per file — keep the signatures from main.rs.
pub fn run(/* ... */) -> Result<()> {
    anyhow::bail!("not yet implemented")
}
```

For exact per-file signatures, copy from the `match cli.command` arms in `main.rs`:
- `add::run(text: Option<String>, pin: bool)`
- `list::run(json: bool)`
- `copy::run(id: Option<String>, next: bool, stdout: bool)`
- `pop::run(id: Option<String>, next: bool, stdout: bool)`
- `pin::run(id: &str, pinned: bool)`

- [ ] **Step 4: Build**

Run: `cargo build -p qcli-bin`
Expected: compiles cleanly.

- [ ] **Step 5: Verify help**

Run: `cargo run -p qcli-bin -- --help`
Expected: lists the six subcommands.

- [ ] **Step 6: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): clap scaffold with subcommand routing stubs"
```

---

## Task 5: `q add`

**Files:**
- Modify: `crates/qcli-bin/src/commands/add.rs`
- Create: `crates/qcli-bin/tests/cli_add.rs`

- [ ] **Step 1: Write the integration test**

`crates/qcli-bin/tests/cli_add.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn add_with_arg_creates_prompt_and_list_shows_it() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "hello world"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("hello world"));
}

#[test]
fn add_from_stdin_when_no_arg() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add"])
        .write_stdin("from stdin\n")
        .assert()
        .success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("from stdin"));
}

#[test]
fn add_pin_flag_marks_prompt_pinned() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "pinned one", "--pin"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(
            predicates::str::contains("[P]").and(predicates::str::contains("pinned one")),
        );
}

#[test]
fn add_empty_text_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("empty"));
}
```

- [ ] **Step 2: Implement `add`**

`crates/qcli-bin/src/commands/add.rs`:
```rust
use std::io::Read;

use anyhow::Result;
use qcli_core::Prompt;

use super::{open_queue, save_queue};

pub fn run(text: Option<String>, pin: bool) -> Result<()> {
    let text = match text {
        Some(t) => t,
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
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

- [ ] **Step 3: Run**

Run: `cargo test -p qcli-bin --test cli_add`
Expected: `add_empty_text_fails` passes. The three tests that depend on `list` will pass after Task 6.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q add reads text from arg or stdin"
```

---

## Task 6: `q list`

**Files:**
- Modify: `crates/qcli-bin/src/commands/list.rs`
- Create: `crates/qcli-bin/tests/cli_list.rs`

- [ ] **Step 1: Write the integration test**

`crates/qcli-bin/tests/cli_list.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn list_on_empty_queue_prints_empty_notice() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn list_shows_id_preview_and_pinned_marker() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "first"]).assert().success();
    q(&dir).args(["add", "second", "--pin"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first"))
        .stdout(predicates::str::contains("second"))
        .stdout(predicates::str::contains("[P]"));
}

#[test]
fn list_json_emits_valid_json_array() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "hello"]).assert().success();
    let output = q(&dir).args(["list", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["text"], "hello");
}
```

- [ ] **Step 2: Implement `list`**

`crates/qcli-bin/src/commands/list.rs`:
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

`serde_json` is already in `qcli-bin/Cargo.toml` via the binary's deps — add it if missing:
```toml
[dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 3: Run**

Run: `cargo test -p qcli-bin`
Expected: `cli_list` tests pass AND the three `cli_add` tests gated on list now pass too.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q list with human and --json output"
```

---

## Task 7: `q copy`

**Files:**
- Modify: `crates/qcli-bin/src/commands/copy.rs`
- Create: `crates/qcli-bin/tests/cli_copy.rs`

- [ ] **Step 1: Write the integration test**

`crates/qcli-bin/tests/cli_copy.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn copy_next_stdout_prints_first_prompt_text() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "alpha"]).assert().success();
    q(&dir).args(["add", "beta"]).assert().success();
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("alpha"));
}

#[test]
fn copy_by_id_prefix_stdout_prints_that_prompt() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "target"]).assert().success();
    let output = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let id = stdout
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .unwrap()
        .to_string();

    q(&dir)
        .args(["copy", &id, "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("target"));
}

#[test]
fn copy_does_not_remove_the_prompt() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "keep me"]).assert().success();
    q(&dir).args(["copy", "--next", "--stdout"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("keep me"));
}

#[test]
fn copy_without_id_or_next_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "x"]).assert().success();
    q(&dir)
        .args(["copy"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--next"));
}

#[test]
fn copy_empty_queue_with_next_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["copy", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("empty"));
}
```

- [ ] **Step 2: Implement `copy`**

`crates/qcli-bin/src/commands/copy.rs`:
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
        queue
            .get(resolved)
            .cloned()
            .ok_or_else(|| anyhow!("prompt missing after resolve"))?
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

- [ ] **Step 3: Run**

Run: `cargo test -p qcli-bin --test cli_copy`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q copy with --next, --stdout, id prefix resolution"
```

---

## Task 8: `q pop`

**Files:**
- Modify: `crates/qcli-bin/src/commands/pop.rs`
- Create: `crates/qcli-bin/tests/cli_pop.rs`

- [ ] **Step 1: Write the integration test**

`crates/qcli-bin/tests/cli_pop.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

#[test]
fn pop_next_stdout_prints_and_removes_first_unpinned() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "first"]).assert().success();
    q(&dir).args(["add", "second"]).assert().success();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("first"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("second"))
        .stdout(predicates::str::contains("first").not());
}

#[test]
fn pop_skips_pinned_prompts() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "pinned", "--pin"]).assert().success();
    q(&dir).args(["add", "floating"]).assert().success();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .stdout(predicates::str::starts_with("floating"));
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("pinned"))
        .stdout(predicates::str::contains("floating").not());
}

#[test]
fn pop_by_id_removes_that_prompt_even_if_pinned() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "target", "--pin"]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let id = stdout
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .unwrap()
        .to_string();

    q(&dir).args(["pop", &id, "--stdout"]).assert().success();
    q(&dir)
        .args(["list"])
        .assert()
        .stdout(predicates::str::contains("(queue empty)"));
}

#[test]
fn pop_next_on_empty_queue_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["pop", "--next", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no unpinned"));
}
```

- [ ] **Step 2: Implement `pop`**

`crates/qcli-bin/src/commands/pop.rs`:
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

- [ ] **Step 3: Run**

Run: `cargo test -p qcli-bin --test cli_pop`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q pop with pinned-skip semantics and id override"
```

---

## Task 9: `q pin` and `q unpin`

**Files:**
- Modify: `crates/qcli-bin/src/commands/pin.rs`
- Create: `crates/qcli-bin/tests/cli_pin.rs`

- [ ] **Step 1: Write the integration test**

`crates/qcli-bin/tests/cli_pin.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn q(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("q").unwrap();
    cmd.env("QCLI_APP_DIR", dir.path());
    cmd
}

fn short_id_of(dir: &TempDir, text_marker: &str) -> String {
    let out = q(dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    stdout
        .lines()
        .find(|line| line.contains(text_marker))
        .unwrap()
        .split_whitespace()
        .find(|s| s.chars().count() == 8)
        .unwrap()
        .to_string()
}

#[test]
fn pin_moves_prompt_to_top() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "first"]).assert().success();
    q(&dir).args(["add", "second"]).assert().success();
    let id = short_id_of(&dir, "second");

    q(&dir).args(["pin", &id]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let second_idx = stdout.lines().position(|l| l.contains("second")).unwrap();
    let first_idx = stdout.lines().position(|l| l.contains("first")).unwrap();
    assert!(second_idx < first_idx, "pinned 'second' should come first");
}

#[test]
fn unpin_moves_prompt_to_unpinned_section() {
    let dir = TempDir::new().unwrap();
    q(&dir).args(["add", "alpha", "--pin"]).assert().success();
    q(&dir).args(["add", "beta"]).assert().success();
    let id = short_id_of(&dir, "alpha");

    q(&dir).args(["unpin", &id]).assert().success();
    let out = q(&dir).args(["list"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let alpha_line = stdout.lines().find(|l| l.contains("alpha")).unwrap();
    assert!(!alpha_line.contains("[P]"), "alpha should no longer be pinned");
}

#[test]
fn pin_unknown_id_fails() {
    let dir = TempDir::new().unwrap();
    q(&dir)
        .args(["pin", "deadbeef"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found"));
}
```

- [ ] **Step 2: Implement `pin` / `unpin`**

`crates/qcli-bin/src/commands/pin.rs`:
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

- [ ] **Step 3: Run**

Run: `cargo test -p qcli-bin --test cli_pin`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/qcli-bin
git commit -m "feat(bin): q pin and q unpin"
```

---

## Task 10: README, full verification, push

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write `README.md`**

````markdown
# q-cli

Terminal-native prompt queue for power users. Keyboard-first TUI (coming) + scriptable CLI (shipping).

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
q add                          # reads from stdin
q list
q list --json
q copy --next                  # copy first prompt to system clipboard
q copy --next --stdout         # print to stdout (pipeable)
q copy <id> [--stdout]
q pop --next                   # copy + remove first unpinned prompt
q pop <id>
q pin <id>
q unpin <id>
```

Prompt ids accept an 8-char prefix or the full UUID.

## Data location

- macOS: `~/Library/Application Support/q-cli/queue.json`
- Linux: `$XDG_DATA_HOME/q-cli/queue.json` or `~/.local/share/q-cli/queue.json`
- Override anywhere: `QCLI_APP_DIR=/tmp/q-sandbox q ...`

## Architecture

- `qcli-core` — queue domain + persistence.
- `qcli-platform` — app dirs, file locking, system clipboard, image helpers.
- `qcli-providers` — OpenAI / Anthropic / Codex integrations (dormant; unlocked by future plans).
- `qcli-tui` — ratatui-based TUI shell (coming).
- `qcli-bin` — the `q` binary.

## Roadmap

See `docs/superpowers/plans/` for the next milestones: TUI shell, prompt upgrade, Codex auth.
````

- [ ] **Step 2: Run the full test suite**

Run: `cargo test --workspace`
Expected: all tests green.

- [ ] **Step 3: Run clippy and fmt**

Run:
```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```
Expected: clean.

- [ ] **Step 4: Manually exercise the binary**

```sh
export QCLI_APP_DIR=$(mktemp -d)
cargo run -p qcli-bin -- add "smoke test"
cargo run -p qcli-bin -- add --pin "pinned smoke"
cargo run -p qcli-bin -- list
cargo run -p qcli-bin -- copy --next --stdout
cargo run -p qcli-bin -- pop --next --stdout
cargo run -p qcli-bin -- list
```

Expected: all six commands behave as described in the README.

- [ ] **Step 5: Commit + push**

```bash
git add README.md
git commit -m "docs: README with install, usage, and architecture"
git push
```

---

## Done — what ships after this plan

- `q add`, `q list`, `q copy`, `q pop`, `q pin`, `q unpin`, all scriptable, all with `--stdout` or `--json` escape hatches where it matters.
- JSON-on-disk persistence with schema versioning and atomic writes.
- Advisory file locking, so concurrent CLI invocations don't corrupt the queue.
- Clipboard integration via `qcli-platform::clipboard::SystemClipboard`.

## Next — the TUI

See `docs/superpowers/plans/2026-04-19-qcli-tui.md` for the TUI implementation plan: three-pane layout, key bindings per `q#11` (Enter, y, p, e, J, K, Ctrl+S, Ctrl+U), and the `q tui` subcommand that launches it.
