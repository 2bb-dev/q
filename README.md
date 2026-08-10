<div align="center">
  <h1><b>q</b></h1>
  <p>A lightning-fast, minimalist native terminal queue.</p>

  [![Version](https://img.shields.io/github/v/release/2bb-dev/q?style=flat-square)](https://github.com/2bb-dev/q/releases)
  [![CI](https://img.shields.io/github/actions/workflow/status/2bb-dev/q/ci.yml?style=flat-square&label=CI)](https://github.com/2bb-dev/q/actions)
  [![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-blue?style=flat-square)]()
  [![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)
</div>

---

**q** is a native terminal tool for writing and managing queued prompts, tasks, and text snippets. Add items, pin the ones you reuse, and pop them off when you need them. Built in Rust with a small native binary and no language runtime.

## Features

- **Workspace tabs** -- Organize prompts into named project queues and switch between them with the mouse or keyboard.
- **Pop-on-Copy** -- Copy the newest prompt to your clipboard and remove it from its queue in one step.
- **Pinning** -- Keep frequently used prompts at the top. Pinned prompts copy without popping.
- **Pipe-friendly** -- Read from stdin, write to stdout, and emit JSON.
- **Cross-platform** -- macOS, Linux, and Windows.
- **Local-first** -- Queue data stays in a local JSON file.

## Install

Run the installer on macOS, Linux, or Git Bash on Windows:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/2bb-dev/q/main/install.sh | sh
```

The script downloads the appropriate binary from the [latest release](https://github.com/2bb-dev/q/releases/latest), verifies its SHA-256 checksum, and installs it to `~/.local/bin`. Set `Q_INSTALL_DIR` to choose another location:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/2bb-dev/q/main/install.sh | Q_INSTALL_DIR="$HOME/bin" sh
```

Verify the installation with `q --version`.

## Data

`q` stores its workspace in `queue.json` under the operating system's application-data directory for `q-cli`. Set `QCLI_APP_DIR` to use a custom directory:

```bash
QCLI_APP_DIR="$HOME/.q" q
```

Back up `queue.json` to preserve all tabs and prompts. Closing a tab permanently deletes the prompts in that tab.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development and contribution instructions.

## License

Distributed under the [MIT License](LICENSE).
