# Contributing to q

Thanks for your interest in contributing! Here's how to get started.

## Getting Started

1. **Fork** the repo and clone your fork.
2. Make sure you have **Rust stable** installed (the toolchain is pinned in `rust-toolchain.toml`).
3. Run `cargo build --workspace` to verify everything compiles.

## Development Workflow

### Before submitting a PR

```bash
# Format
cargo fmt --all

# Lint (must pass with zero warnings)
cargo clippy --workspace --all-targets -- -D warnings

# Test
cargo test --workspace
```

All three must pass cleanly. CI will enforce this.

### Code Style

- **Match existing style.** Don't reformat code you didn't change.
- **Keep changes surgical.** Every changed line should trace to the purpose of the PR.
- **No speculative features.** Solve the problem at hand, nothing more.
- **Error handling:** `thiserror` in library crates, `anyhow` in the binary. No `unwrap()` or `expect()` outside tests.
- **Dependencies:** Declare at the workspace level in root `Cargo.toml`; crates use `{ workspace = true }`.

### Architecture Rules

- Only `qcli-platform` and `qcli-core::storage` may perform filesystem or OS operations.
- Domain code in `qcli-core` must remain pure (no I/O).
- Provider integrations go in `qcli-providers`.

## Submitting Changes

1. Create a feature branch: `feat/my-feature` or `fix/my-bug`.
2. Write clear commit messages.
3. Open a Pull Request with a description of **what** changed and **why**.
4. Link any related issues.

## Reporting Bugs

Open an issue with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Your OS and Rust version (`rustc --version`)

## Feature Requests

Open an issue tagged with `enhancement`. Describe the use case, not just the solution.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
