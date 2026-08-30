# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
- **Breaking:** replaced the single `queue.json` with a directory-based workspace format under `workspaces/<id>/` with one file per tab and prompt. An existing `queue.json` (schemas 1–4) migrates automatically into a workspace named "Personal" on first run and is renamed to `queue.json.migrated`; older binaries cannot read the new layout ([#6](https://github.com/2bb-dev/q/issues/6))
- **Breaking:** replaced the persisted `pinned` boolean with a `pinned_at` timestamp; pin state and ordering behave as before ([#6](https://github.com/2bb-dev/q/issues/6))
- **Breaking:** bumped the `queue.json` schema to 4 for typed inline and external Markdown sources. Schemas 1–3 migrate automatically on load, but older binaries cannot read the workspace after it is next saved
- **Breaking:** required `--tab <name>` for every `q add`, including workspaces with only one tab
- Changed queue cards to show only the first source line, truncating long lines with a trailing ellipsis
- Aligned the footer hints with the composer prompt marker
- Moved every unit test out of `src/` into `crates/<pkg>/tests/unit/`, attached to its module with `#[cfg(test)] #[path = ...] mod tests;`

### Removed
- Removed the composer placeholder text
- Removed the dormant image attachment support, including image storage, the images directory, and image-clipboard code ([#5](https://github.com/2bb-dev/q/issues/5))

### Added
- Added the join flow: `c` in the Workspaces overlay lists pending GitHub repository invitations (acceptable in-app with Enter) and accessible `q-workspace` repos not yet connected, and clones the selected repo into a new local team workspace ([#13](https://github.com/2bb-dev/q/issues/13))
- Added background sync for team workspaces: the TUI fetches and merges every ~20 seconds and pushes local changes debounced; CLI commands pull before and push after; per-prompt files merge cleanly, same-prompt conflicts resolve to the newer `updated_at` and deletions win; the Workspaces overlay shows git-style sync colors (green synced, yellow pending, red error) and offline work degrades to a warning ([#12](https://github.com/2bb-dev/q/issues/12))
- Added team workspaces backed by a private GitHub repo: create one with `t` in the Workspaces overlay or convert a personal workspace from its info dialog, choosing the repo owner (personal account or organization); the repo is created private with the `q-workspace` topic and the workspace content pushed. Team workspaces are inline-only and per-user history is never committed ([#11](https://github.com/2bb-dev/q/issues/11))
- Added author attribution: with a connected GitHub identity, new prompts and tabs record `created_by` and prompt edits record `updated_at`/`updated_by`, visible in `q list --json`; without an identity nothing is recorded ([#10](https://github.com/2bb-dev/q/issues/10))
- Added GitHub sign-in under Settings → GitHub: reuses an existing `gh` CLI login automatically, or runs the OAuth device flow in-app (requires the `QCLI_GITHUB_CLIENT_ID` environment variable) storing the token in a `0600` file; shows the connected login and supports disconnect ([#9](https://github.com/2bb-dev/q/issues/9))
- Added a TUI menu overlay opened with `Cmd+I` (or `w` in the queue pane): Workspaces lists workspaces with the current one marked, switches on Enter, creates with `n`, and opens a per-workspace info dialog with `i` for rename and confirmed delete; Settings is a placeholder shell; `Esc` walks back one level at a time ([#8](https://github.com/2bb-dev/q/issues/8))
- Added multiple workspaces: `q workspace list|create|rename|delete|switch` manages them, `state.json` remembers the active one, and `--workspace <name>` targets any workspace for a single command ([#7](https://github.com/2bb-dev/q/issues/7))
- Added live `.md` and `.markdown` references through `q add --tab <name> <path>`, including current-content copy, pop, preview, history search, availability-aware JSON, and `--text` for literal Markdown-looking input
- Added a built-in full-screen editor for inline prompts and referenced Markdown files, with identity-preserving inline edits, format-preserving atomic file saves, external-change conflict detection, and safe unsaved-buffer handling
- Added `q remove <ID>` and confirmed TUI deletion to discard queue records without copying or deleting referenced files
- Added a scrollable full-text preview for the selected prompt, opened with `f` in the queue pane (`↑↓`/`j`/`k`, `PgUp`/`PgDn`, `g`/`G` and the mouse wheel scroll, `Enter` copies, `Esc` closes)
- Added searchable prompt history that keeps every prompt ever added, including popped prompts and prompts from closed tabs, capped at the 500 most recent entries and 256 KiB of prompt text
- Added ways to forget remembered prompts: `^d` on the selected entry in the TUI history search, `q history --forget <text>` to drop everything matching a term, and `q history --clear` to forget all of it
- Added a TUI history search opened with `Cmd+/` from either pane or `/` in the queue pane, with click-to-open results, wheel scrolling, a fullscreen view of the selected entry, and `Enter` to copy and return to the queue
- Added the `q history [search] [--json]` command for searching prompt history from the CLI
- Added language-agnostic prompt search that transliterates both the query and the stored text to ASCII, so a Latin query finds Cyrillic text (`uluchshit` matches `улучшить`), accents are ignored (`cafe` matches `café`), both the `ia` and `ya` transliteration conventions match, and composed (NFC) and decomposed (NFD) text compare equal
- Added `←`/`→` tab switching in the queue pane, alongside the existing `[` and `]` shortcuts

### Fixed
- Fixed uppercase letters entered with Shift in the TUI composer, including non-Latin keyboard layouts

## [0.1.0] - 2026-08-10

### Changed
- Improved TUI composer responsiveness with batched input, bracketed paste, cursor-aware editing, and terminal-native navigation and deletion shortcuts
- Allowed multiple editable TUI windows to stay synchronized without losing concurrent queue changes
- Made `q` launch the TUI by default while retaining `q tui` as an explicit alias
- Ordered tabs by recent prompt activity and prompts newest-first within pinned and unpinned groups
- Styled the full tab row and placed the `+` action directly beside visible tabs
- Renamed workspace crates from `qcli-*` to `q-*`, including `q-cli`, `q-core`, `q-platform`, and `q-tui`

### Added
- Added a checksum-verifying installer for macOS, Linux, and Git Bash on Windows
- Added named TUI workspace tabs with mouse and keyboard navigation, create/rename dialogs, per-tab queues, schema-v1 migration, and CLI `--tab` targeting
- Added right-click tab actions for renaming and confirmed tab closure
- Added mouse selection and focus for prompts and the composer
- Core queue domain model with newest-first ordering and pinning
- JSON persistence via `q-core::storage`
- CLI commands: `add`, `list`, `copy`, `pop`, `pin`, `unpin`
- Platform abstractions: clipboard (text + image), app dirs, file locking
- Pipe-friendly I/O: stdin input, stdout output, JSON output
- CI workflow: fmt, clippy, test on push and PR
- CI test matrix across ubuntu-latest, macos-latest, windows-latest with cargo caching; fmt + clippy run once on Linux
- End-to-end CLI workflow integration test (`crates/q-cli/tests/cli_workflow.rs`) covering add/list/copy/pop/pin/unpin against a shared queue file
- CLI error-path tests for pin/unpin (unknown id, short-id rejection, pin idempotency)
- TUI render tests using `ratatui::backend::TestBackend`
- TUI `map_key` unit tests covering keyboard routing across panes
- Additional `q-platform` tests for `config_path`, `images_dir`, nested app-dir creation, and `FakeClipboard` overwrite/default behavior
- Mutex-guarded env-var access in `q-platform::paths` tests to avoid flakes from cargo's parallel test runner

### Fixed
- Restored queue-focused `p` pin and `e` edit keyboard shortcuts

### Removed
- Removed the unused OpenAI, Anthropic, and Codex provider scaffolding
