<div align="center">
  <h1><b>q</b></h1>
  <p>A lightning-fast, terminal-native prompt queue for power users.</p>

  [![Version](https://img.shields.io/github/v/release/2bb-dev/q-cli?style=flat-square)](https://github.com/2bb-dev/q-cli/releases)
  [![CI](https://img.shields.io/github/actions/workflow/status/2bb-dev/q-cli/ci.yml?style=flat-square&label=CI)](https://github.com/2bb-dev/q-cli/actions)
  [![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-blue?style=flat-square)]()
  [![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)
</div>

---

> **Created by [2bb](https://github.com/2bb-dev)**

---

**q** is the terminal-native, keyboard-first version of [`q`](https://github.com/2bb-dev/q) — a zero-bloat prompt queue designed for prompt engineering workflows. Queue up text snippets, pin the important ones, and copy-pop them into your LLM sessions without ever leaving the terminal.

Built in **Rust** with [`ratatui`](https://ratatui.rs) for the TUI and [`clap`](https://docs.rs/clap) for the CLI. Fast startup, tiny binary, no runtime dependencies.

## ✨ Features

- **CLI + TUI** — Use `q add`, `q pop`, `q list` from scripts, or launch the interactive TUI for a full visual queue.
- **Pop-on-Copy** — Copy a prompt to your clipboard and remove it from the queue in one step. True FIFO efficiency.
- **Pinning** — Lock frequently-used prompts to the top. Pinned prompts copy without popping.
- **Pipe-friendly** — Read from stdin (`echo "prompt" | q add`), write to stdout (`q pop --stdout`), emit JSON (`q list --json`).
- **AI Upgrade** *(coming soon)* — Send prompts to OpenAI, Anthropic, or Codex to refine them before use.
- **Cross-platform** — macOS, Linux, and Windows.

## 🚀 Quick Start

### Install from source

```bash
# Clone
git clone https://github.com/2bb-dev/q-cli.git
cd q-cli

# Build & install
cargo install --path crates/qcli-bin
```

The binary is called `q` and will be placed in your Cargo bin directory (`~/.cargo/bin/`).

### Usage

```bash
# Add prompts to the queue
q add "Explain monads like I'm five"
q add "Write a Rust macro that generates Builder patterns" --pin
echo "Summarize this diff" | q add

# List your queue
q list
q list --json

# Copy the next prompt (FIFO) to clipboard
q copy --next

# Pop: copy + remove
q pop --next

# Pop a specific prompt by ID
q pop <id>

# Output to stdout instead of clipboard
q pop --next --stdout

# Pin / unpin
q pin <id>
q unpin <id>
```

## 🏗 Architecture

`q-cli` is a Cargo workspace split into focused crates:

```
crates/
├── qcli-core       # Queue domain model, persistence (JSON storage)
├── qcli-platform   # OS abstractions: app dirs, file locking, clipboard, images
├── qcli-providers   # LLM integrations: OpenAI, Anthropic, Codex
├── qcli-tui        # Interactive terminal UI (ratatui + crossterm)
└── qcli-bin        # The `q` binary — thin CLI orchestration
```

**Design principles:**
- Domain logic in `qcli-core` is pure — no I/O, no OS calls.
- Only `qcli-platform` and `qcli-core::storage` touch the filesystem.
- Provider integrations are isolated and optional.

## 🛠 Development

### Prerequisites

- **Rust** (stable, pinned via `rust-toolchain.toml`)
- **Node.js** (for tooling only, not required at runtime)

### Build & Test

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Project Conventions

- Dependencies are declared at the **workspace level** in the root `Cargo.toml`; crates reference them with `{ workspace = true }`.
- Error handling: `thiserror` for library crates, `anyhow` for the binary.
- No `unwrap()` or `expect()` outside tests.
- Unit tests live next to the code (`mod tests`). Integration tests live in `crates/qcli-bin/tests/`.

## 🗺 Roadmap

- [x] Core queue with FIFO pop, pinning, and JSON persistence
- [x] CLI commands: `add`, `list`, `copy`, `pop`, `pin`, `unpin`
- [x] Platform abstractions: clipboard, app dirs, file locking
- [ ] Interactive TUI (`q` with no subcommand)
- [ ] AI-powered prompt upgrade via OpenAI / Anthropic / Codex
- [ ] Homebrew formula & prebuilt binaries
- [ ] Shell completions (bash, zsh, fish)
- [ ] Config file support (`~/.config/q/config.toml`)

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

**TL;DR:**
1. Fork & clone
2. Create a branch (`feat/my-feature` or `fix/my-bug`)
3. Make your changes, ensure `cargo test --workspace` passes
4. Submit a PR

## 📝 License

Distributed under the [MIT License](LICENSE).

## 🔗 Related

- [**q** (desktop)](https://github.com/2bb-dev/q) — The original Tauri desktop app with a visual UI.
