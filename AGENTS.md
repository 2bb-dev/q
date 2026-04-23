# AGENTS.md

Guidelines for AI agents (Codex, Claude, Cursor, etc.) working in `q`.

The full policy lives in [CLAUDE.md](./CLAUDE.md). This file exists so agents that look for `AGENTS.md` by convention find the same guidance. Both files are kept in sync; update them together.

---

## What this project is

`q` is a native terminal queue for prompts, tasks, and text snippets. The original desktop app lives in [`q-desktop`](https://github.com/2bb-dev/q-desktop). The codebase is a Rust Cargo workspace:

- `qcli-core` -- queue domain, persistence.
- `qcli-platform` -- app dirs, file locking, clipboard.
- `qcli-providers` -- OpenAI, Anthropic, Codex integrations.
- `qcli-tui` -- `ratatui` + `crossterm` UI shell.
- `qcli-bin` -- the `q` binary, thin orchestration over the above.

Implementation plans live in `docs/superpowers/plans/`. Read the current plan before starting work.

---

## Conversational Style

- Keep answers short and concise.
- No emojis in commits, issues, PR comments, or code.
- No fluff or cheerful filler text.
- Technical prose only. Be kind but direct.
- Always ask before removing functionality or code that appears intentional.

---

## Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them -- don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

## Surgical Changes

Touch only what you must. Clean up only your own mess.

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it -- don't delete it.
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.
- Do not preserve backward compatibility unless the user explicitly asks for it.

The test: every changed line should trace directly to the user's request.

## Goal-Driven Execution

Define success criteria. Loop until verified.

- "Add validation" -> "Write tests for invalid inputs, then make them pass"
- "Fix the bug" -> "Write a test that reproduces it, then make it pass"
- "Refactor X" -> "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] -> verify: [check]
2. [Step] -> verify: [check]
```

---

## Rust Specifics

- **Toolchain**: stable, pinned via `rust-toolchain.toml`.
- **Dependencies**: declare at the workspace level in the root `Cargo.toml`; crates reference them with `{ workspace = true }`.
- **Error types**: `thiserror` for library crates, `anyhow` for the binary crate.
- **No `unwrap()` or `expect()` outside tests.**
- **I/O boundaries**: only `qcli-platform` and `qcli-core::storage` touch the filesystem or OS APIs. Domain code stays pure.
- **Tests**: unit tests live next to the code (`mod tests`). Integration tests live in `crates/qcli-bin/tests/`.

---

## Commands

After code changes (not documentation-only changes), run the full check:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Fix all errors and warnings before committing.

If you create or modify a test file, you MUST run it and iterate until it passes.

NEVER commit unless the user asks.

---

## Git Rules

### Committing

- ONLY commit files YOU changed in THIS session.
- ALWAYS include `fixes #<number>` or `closes #<number>` in the commit message when there is a related issue or PR.
- NEVER use `git add -A` or `git add .` -- these sweep up changes from other agents.
- ALWAYS use `git add <specific-file-paths>` listing only files you modified.
- Before committing, run `git status` and verify you are only staging YOUR files.
- No emojis in commit messages.

### Forbidden Git Operations

These commands can destroy other agents' work:

- `git reset --hard` -- destroys uncommitted changes
- `git checkout .` -- destroys uncommitted changes
- `git clean -fd` -- deletes untracked files
- `git stash` -- stashes ALL changes including other agents' work
- `git add -A` / `git add .` -- stages other agents' uncommitted work
- `git commit --no-verify` -- bypasses required checks, never allowed

---

## PR Workflow

- Analyze PRs without pulling locally first.
- If the user approves: create a feature branch, pull PR, rebase on main, apply adjustments, commit, merge into main, push, close PR.
- You never open PRs yourself. Work in feature branches until everything meets the user's requirements, then merge into main and push.

---

## Changelog

Location: `CHANGELOG.md` at the repo root.

### Format

Use these sections under `## [Unreleased]`:

```
### Breaking Changes - API changes requiring migration
### Added - New features
### Changed - Changes to existing functionality
### Fixed - Bug fixes
### Removed - Removed features
```

### Rules

- Before adding entries, read the full `[Unreleased]` section to see which subsections already exist.
- New entries ALWAYS go under `## [Unreleased]`.
- Append to existing subsections, do not create duplicates.
- NEVER modify already-released version sections.
- Each version section is immutable once released.

### Attribution

- Internal changes (from issues): `Fixed foo bar ([#123](https://github.com/2bb-dev/q/issues/123))`
- External contributions: `Added feature X ([#456](https://github.com/2bb-dev/q/pull/456) by [@username](https://github.com/username))`
