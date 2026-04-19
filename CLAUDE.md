# CLAUDE.md

Project-level guidelines for all contributors and AI agents working in `q-cli`.

---

## What this project is

`q-cli` is the terminal-native, keyboard-first version of [`q`](https://github.com/2bb-dev/q). The installed binary is called `q`. The codebase is a Rust Cargo workspace:

- `qcli-core` — queue domain, persistence.
- `qcli-platform` — app dirs, file locking, clipboard.
- `qcli-providers` — OpenAI, Anthropic, Codex integrations.
- `qcli-tui` — `ratatui` + `crossterm` UI shell.
- `qcli-bin` — the `q` binary, thin orchestration over the above.

Implementation plans live in `docs/superpowers/plans/`. Read the current plan before starting work.

---

## Linear Task Management

**Every piece of work must have a Linear issue. No exceptions.**

### Creating tasks
- Before starting any work, check if a Linear issue exists in the relevant project (e.g., **q-cli**).
- If no issue exists, create one first. Title should be clear and actionable.
- New issues start in **Todo** or **Backlog**.

### Tracking status
Move issues through the workflow as work progresses — never leave a stale status:

| Status | When to use |
|--------|-------------|
| **Todo** | Task is defined and ready to be picked up |
| **In Progress** | Actively being worked on right now |
| **In Review** | Work complete, waiting for review or verification |
| **Done** | Fully completed and verified |
| **Canceled** | No longer needed — add a note why |

- When you **start** a task: move it to **In Progress**.
- When you **finish** a task: move it to **Done**.
- If a task is blocked or abandoned: move it to **Canceled** with a comment.
- Never leave tasks in **In Progress** if work has stopped.

### Rules for contributors
- One issue per logical unit of work — don't bundle unrelated changes.
- Sub-tasks (child issues) should be used for large epics.
- Always link your git branch to the corresponding Linear issue (use the generated branch name when possible).
- Update the Linear issue status on the same day the work state changes.

---

## Coding Guidelines

Behavioral guidelines to reduce common LLM and contributor coding mistakes.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

## Rust specifics

- **Toolchain**: stable, pinned via `rust-toolchain.toml`.
- **Before committing**: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- **Dependencies**: declare at the workspace level in the root `Cargo.toml`; crates reference them with `{ workspace = true }`.
- **Error types**: `thiserror` for library crates, `anyhow` for the binary crate. Don't use `unwrap()` or `expect()` outside tests.
- **I/O boundaries**: only `qcli-platform` and `qcli-core::storage` touch the filesystem or OS APIs. Domain code stays pure.
- **Tests**: unit tests live next to the code they test (`mod tests`). CLI integration tests live in `crates/qcli-bin/tests/`.

---

**These guidelines are working if:** Linear always reflects real work state, diffs have fewer unnecessary changes, rewrites due to overcomplication are rare, and clarifying questions come before implementation rather than after mistakes.
