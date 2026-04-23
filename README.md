<div align="center">
  <h1><b>q</b></h1>
  <p>A lightning-fast, minimalist native terminal queue.</p>

  [![Version](https://img.shields.io/github/v/release/2bb-dev/q?style=flat-square)](https://github.com/2bb-dev/q/releases)
  [![CI](https://img.shields.io/github/actions/workflow/status/2bb-dev/q/ci.yml?style=flat-square&label=CI)](https://github.com/2bb-dev/q/actions)
  [![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-blue?style=flat-square)]()
  [![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)
</div>

---

**q** is a native terminal tool for writing and managing queued prompts, tasks, and text snippets. Add items, pin the ones you reuse, and pop them off when you need them. Built in Rust. Fast startup, tiny binary, no runtime dependencies.

## Features

- **Pop-on-Copy** -- Copy a prompt to your clipboard and remove it from the queue in one step. FIFO order.
- **Pinning** -- Lock frequently-used prompts to the top. Pinned prompts copy without popping.
- **Pipe-friendly** -- Read from stdin, write to stdout, emit JSON.
- **Cross-platform** -- macOS, Linux, and Windows.

## Install

```bash
git clone https://github.com/2bb-dev/q.git
cd q
cargo install --path crates/qcli-bin
```

## License

Distributed under the [MIT License](LICENSE).
