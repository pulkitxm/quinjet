# Quinjet Architecture

## Design Goals

Quinjet optimizes for immediate input response, authoritative Git behavior, and predictable memory use. The terminal render path never spawns Git or performs filesystem traversal.

## Layers

```text
Terminal events
      │
      ▼
App state + intents ───────────────► Ratatui renderer
      │                                  ▲
      ▼                                  │ latest snapshot
bounded Git command queue                │
      │                                  │
      ▼                                  │
Git worker ── Git CLI ── parsed events ──┘
      ▲
      │
filesystem watcher (coalesced signal)
```

- `src/app.rs`: focus, selection, modal, command, pagination, and generation state.
- `src/git/mod.rs`: safe argv construction and user-facing Git operations.
- `src/git/status.rs`: byte-oriented porcelain-v2 status parser.
- `src/git/history.rs`: delimiter-safe history parser.
- `src/git/github.rs`: bounded `gh` discovery across fetch/push remotes, typed PR metadata, and explicitly targeted PR diffs.
- `src/git/diff.rs`: unified-diff model and syntax highlighting for working-tree, commit, and PR patches.
- `src/git/worker.rs`: the non-blocking UI/Git boundary.
- `src/watch.rs`: repository watcher and event-storm coalescing.
- `src/ui/`: viewport-only rendering, diff layouts, theme, and mouse hit map.

## Responsiveness Invariants

1. The UI thread mutates only in-memory state and renders visible rows.
2. Every preview, status, history, branch, and pull-request request carries a generation; stale replies are ignored.
3. The worker mailbox has fixed coalescing slots for status, history, branch, pull-request-list, and preview reads, so key repeat cannot allocate unbounded work; ordered user mutations are serialized by app state.
4. Watcher signals are lossy by design: one full status snapshot subsumes all preceding file events.
5. Git and PR diff output is capped at 8 MiB; history, remote discovery, and PR lists have explicit limits.
6. Git and GitHub CLI receive argv directly, never via a shell. Every PR request carries its canonical base-repository URL, avoiding ambient-remote ambiguity.
7. Read operations set `GIT_OPTIONAL_LOCKS=0`; `gh` runs with prompts, paging, color, and update checks disabled on the worker thread.

## VS Code Research

The ignored `extras/vscode` checkout was explored independently. Its central pattern is a provider-neutral SCM workbench driven by a Git-CLI-backed provider. Quinjet adopts the same high-level boundary in a smaller form: UI/app code works with typed snapshots while `git` owns repository semantics.

Important parity findings incorporated in the first implementation:

- Separate merge, staged, and worktree resource groups.
- Event-driven refresh with noise filtering and snapshot reconciliation.
- Git CLI delegation for hooks, credentials, config, refs, and index semantics.
- Paginated history and lazy commit preview.
- Side-by-side and unified syntax-highlighted diff surfaces.
- Context-sensitive operations, confirmations, keyboard help, and optional mouse support.

## Deliberate Next Steps

Full VS Code parity is an ongoing scope rather than a single release. Highest-priority follow-ups are multi-repository actors, non-UTF-8 path preservation, partial hunk staging, a true three-way merge editor, ref-filtered topology lanes, and configurable keymaps.
