# Quinjet

[![CI](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml/badge.svg)](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/quinjet.svg)](https://crates.io/crates/quinjet)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, live, keyboard-first Git source-control interface for the terminal, written in Rust.

Quinjet discovers the containing Git repository from any nested directory, watches it for changes, and combines a VS Code-inspired changes list with syntax-highlighted diffs and commit history. Navigation and rendering remain independent from Git subprocess latency.

## Features

- Live working-tree, index, conflict, branch, and ahead/behind refresh
- Scrollable staged, unstaged, untracked, renamed, deleted, and conflict groups
- One-key file staging/unstaging with immediate authoritative refresh
- Syntax highlighting for TypeScript/TSX, Rust, Python, Go, JavaScript, and hundreds of other formats
- Unified and draggable side-by-side diff panes
- Compact change hunks by default; `t` expands the selected file to full context
- Paginated commit history with one commit-details card and a titled diff pane for every changed file
- Commit, amend, stash, fetch, pull, push, sync, cherry-pick, and revert
- Branch switching, creation, deletion, and creation at a selected commit
- Natural mouse scrolling, clickable rows, and draggable pane dividers
- Keyboard-first filtering, command palette, modal text editing, and accessibility help
- Coalesced background Git work, stale-result rejection, and bounded diff output

## Installation

From crates.io:

```bash
cargo install quinjet
```

From the latest source:

```bash
cargo install --git https://github.com/pulkitxm/quinjet --locked
```

Git and a terminal with true-color support are recommended.

## Usage

Run from any directory inside a Git repository:

```bash
quinjet
```

Quinjet asks Git for the top-level repository, so running it from `project/apps/web/src` still displays every change in `project`. You can also provide a path explicitly:

```bash
quinjet /path/to/project
```

Mouse capture can be disabled without losing functionality:

```bash
quinjet --no-mouse
```

## Keyboard

The UI intentionally stays uncluttered; press `?` for the complete shortcut reference.

| Key | Action |
|---|---|
| `j` / `k`, arrows | Move through every file/commit or scroll the preview |
| Mouse wheel | Naturally scroll the pane under the pointer |
| `Tab` / `Enter` | Toggle sidebar/preview focus |
| `z` | Hide/show the sidebar |
| `e` | Collapse/expand every file diff |
| `1` / `2` | Changes/history |
| `s` or `Space` | Toggle stage/unstage for the selected file |
| `a` / `U` | Stage all/unstage all |
| `c` | Open commit editor; `Ctrl+Enter` submits |
| `t` | Toggle compact hunks/full-file context |
| `v` | Toggle unified/side-by-side diff |
| `x` | Discard selected change after confirmation |
| `/` | Filter the active list |
| `[` / `]` | Previous/next diff hunk |
| `:` or `Ctrl+P` | Command palette |
| `?` | Shortcut help |
| `q` | Quit |

Drag the divider between the file list and preview to resize the main panes. In side-by-side mode, drag the center divider to resize old/new sides.

Text fields support Unicode-safe editing plus familiar terminal and macOS motions: Option/Ctrl+Arrow moves by word, Option/Ctrl+Delete removes a word, Command+Arrow moves to a line boundary, Command+Delete removes to a line boundary, and Ctrl+A/E/B/F/W/U/K are supported.

## Performance and Safety

- Rendering and key handling never invoke Git directly.
- A coalescing worker mailbox replaces obsolete read requests.
- Filesystem event storms collapse into authoritative status snapshots.
- Preview requests carry generations so stale replies are ignored.
- History is paginated and diff output is bounded.
- Git receives argument arrays directly, never shell-concatenated commands.
- Destructive operations are confirmed where appropriate.
- Hooks, credentials, signing, filters, and repository semantics remain delegated to the installed Git CLI.

See [ARCHITECTURE.md](ARCHITECTURE.md) for implementation details.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md), the [Code of Conduct](CODE_OF_CONDUCT.md), and [Security Policy](SECURITY.md) before opening a pull request or report.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --locked
```

## License

Quinjet is available under the [MIT License](LICENSE).
