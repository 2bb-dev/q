# q-cli

Terminal-native prompt queue for power users. Keyboard-first TUI (coming) + scriptable CLI (shipping).

## Install (development)

```sh
git clone git@github.com:2bb-dev/q-cli.git
cd q-cli
cargo install --path crates/qcli-bin
```

The binary is called `q`.

## Usage

```sh
q add "refactor the queue module"
q add --pin "always remember: prefer surgical diffs"
q add                          # reads from stdin
q list
q list --json
q copy --next                  # copy first prompt to system clipboard
q copy --next --stdout         # print to stdout (pipeable)
q copy <id> [--stdout]
q pop --next                   # copy + remove first unpinned prompt
q pop <id>
q pin <id>
q unpin <id>
```

Prompt ids accept an 8-char prefix or the full UUID.

## TUI

Launch with:

    q tui

Three panes: queue (left), composer (top right), details (bottom right).

### Key bindings

| Key | Action |
|---|---|
| `Tab` | Toggle focus between queue and composer |
| `j` / `k` or `↓` / `↑` | Move selection (queue pane) |
| `Enter` | Copy selected prompt; pop if unpinned |
| `y` | Copy without popping |
| `p` | Pin / unpin selected |
| `e` | Edit selected (loads into composer) |
| `J` / `K` or `Shift+↓` / `Shift+↑` | Reorder within pinned/unpinned group |
| `Ctrl+S` | Save composer text as new prompt |
| `Ctrl+U` | Upgrade composer via provider (requires config) |
| `q` (queue focus) / `Ctrl+C` | Quit |

## Data location

- macOS: `~/Library/Application Support/q-cli/queue.json`
- Linux: `$XDG_DATA_HOME/q-cli/queue.json` or `~/.local/share/q-cli/queue.json`
- Override anywhere: `QCLI_APP_DIR=/tmp/q-sandbox q ...`

## Architecture

- `qcli-core` — queue domain + persistence.
- `qcli-platform` — app dirs, file locking, system clipboard, image helpers.
- `qcli-providers` — OpenAI / Anthropic / Codex integrations (dormant; unlocked by future plans).
- `qcli-tui` — ratatui-based TUI shell (coming).
- `qcli-bin` — the `q` binary.

## Roadmap

See `docs/superpowers/plans/` for the next milestones: TUI shell, prompt upgrade, Codex auth.
