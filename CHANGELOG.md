# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
- Improved TUI composer responsiveness with batched input, bracketed paste, cursor-aware editing, and terminal-native navigation and deletion shortcuts
- Allowed multiple editable TUI windows to stay synchronized without losing concurrent queue changes
- Made `q` launch the TUI by default while retaining `q tui` as an explicit alias
- Ordered tabs by recent prompt activity and prompts newest-first within pinned and unpinned groups

### Added
- Added named TUI workspace tabs with mouse and keyboard navigation, create/rename dialogs, per-tab queues, schema-v1 migration, and CLI `--tab` targeting
- Core queue domain model with newest-first ordering and pinning
- JSON persistence via `qcli-core::storage`
- CLI commands: `add`, `list`, `copy`, `pop`, `pin`, `unpin`
- Platform abstractions: clipboard (text + image), app dirs, file locking
- Pipe-friendly I/O: stdin input, stdout output, JSON output
- LLM provider scaffolding: OpenAI, Anthropic, Codex (dormant)
- CI workflow: fmt, clippy, test on push and PR
- CI test matrix across ubuntu-latest, macos-latest, windows-latest with cargo caching; fmt + clippy run once on Linux
- End-to-end CLI workflow integration test (`crates/qcli-bin/tests/cli_workflow.rs`) covering add/list/copy/pop/pin/unpin against a shared queue file
- CLI error-path tests for pin/unpin (unknown id, short-id rejection, pin idempotency)
- TUI render tests using `ratatui::backend::TestBackend`
- TUI `map_key` unit tests covering keyboard routing across panes
- Additional `qcli-platform` tests for `config_path`, `images_dir`, nested app-dir creation, and `FakeClipboard` overwrite/default behavior
- Mutex-guarded env-var access in `qcli-platform::paths` tests to avoid flakes from cargo's parallel test runner

### Fixed
- Restored queue-focused `p` pin and `e` edit keyboard shortcuts
