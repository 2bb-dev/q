# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Core queue domain model with FIFO ordering and pinning
- JSON persistence via `qcli-core::storage`
- CLI commands: `add`, `list`, `copy`, `pop`, `pin`, `unpin`
- Platform abstractions: clipboard (text + image), app dirs, file locking
- Pipe-friendly I/O: stdin input, stdout output, JSON output
- LLM provider scaffolding: OpenAI, Anthropic, Codex (dormant)
- CI workflow: fmt, clippy, test on push and PR
