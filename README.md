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
- **Prompt history** -- Every prompt you add is remembered, even after it is popped or its tab is closed. Search it with `Cmd+/` (or `/` in the queue pane) and from the CLI with `q history`. Forget entries you would rather not keep with `^d` in the search overlay, `q history --forget <text>`, or `q history --clear`.
- **Live Markdown references** -- Queue `.md` and `.markdown` files without copying them. Copy, pop, preview, history, and search use the file's current contents.
- **Built-in editor** -- Press `e` to edit inline prompts or referenced Markdown files in a full-screen editor with conflict-safe saves.
- **Search in any language** -- Queries are transliterated to ASCII, so `uluchshit` finds `улучшить`, `cafe` finds `café`, and case, accents, and Unicode composition forms are all ignored.
- **Native mouse** -- Every TUI surface is clickable: tabs, prompts, the composer, right-click tab menus, and history search results, with wheel scrolling throughout.
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

## CLI examples

Every CLI add names its destination tab explicitly:

```bash
q add --tab 1 "review this patch"
q add --tab research ./notes.md
printf 'multiline prompt\n' | q add --tab research
```

A `.md` or `.markdown` positional argument is stored as a live external reference. Use `--text` to queue a Markdown-looking value literally:

```bash
q add --tab 1 --text "notes.md"
```

Copy reads without removing, pop reads and removes the queue item, and remove discards an item without reading it:

```bash
q copy --next --stdout --tab research
q pop --next --stdout --tab research
q remove <ID>
```

`q remove` never deletes an external file.

## Data

`q` stores its workspace in `queue.json` under the operating system's application-data directory for `q-cli`. Set `QCLI_APP_DIR` to use a custom directory:

```bash
QCLI_APP_DIR="$HOME/.q" q
```

Back up `queue.json` to preserve all tabs, inline prompts, and external-file references. Referenced Markdown files remain at their absolute paths and are not included in this backup. Moving or deleting one leaves a removable broken reference.

Closing a tab permanently deletes its queued items, but their sources stay searchable in prompt history, which keeps the 500 most recent sources around a 256 KiB target budget. Inline history retains text; external-file history retains a live path rather than a content snapshot. Use `q history --forget <text>` or `q history --clear` to remove history entries.

The built-in editor serializes `q` saves targeting the same resolved file and rejects observed content, file-identity (where exposed by the platform), or permission changes. Operating systems do not provide a portable atomic compare-and-replace against non-cooperating editors, so avoid saving the same Markdown file from another program at exactly the same time as `q`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development and contribution instructions.

## License

Distributed under the [MIT License](LICENSE).
