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
- `src/git/mod.rs`: safe argv construction, branch-scoped history reads, and user-facing Git operations.
- `src/git/status.rs`: byte-oriented porcelain-v2 status parser.
- `src/git/history.rs`: delimiter-safe history parser.
- `src/git/github.rs`: bounded/cached cursor batches for all PR states, repository-scoped lookup, and disposable local PR fetch/diff generation.
- `src/git/diff.rs`: unified-diff model and syntax highlighting for working-tree, commit, and PR patches.
- `src/git/worker.rs`: the non-blocking UI/Git boundary.
- `src/watch.rs`: repository watcher and event-storm coalescing.
- `src/ui/`: viewport-only rendering, diff layouts, theme, and mouse hit map.

## Responsiveness Invariants

1. The UI thread mutates only in-memory state and renders visible rows.
2. Every preview, status, history, branch, and pull-request request carries a generation; stale replies are ignored.
3. Fixed coalescing mailboxes isolate status/history, GitHub metadata prefetch, local change/commit previews, and potentially network-bound PR previews. Key repeat remains constant-space, slow GitHub work cannot block tab switches, and ordered user mutations are serialized by app state.
4. Watcher signals are lossy by design: one full status snapshot subsumes all preceding file events.
5. Git/PR patch output is capped at 8 MiB. PR metadata is capped at 2 MiB, GraphQL batches at 50 records, the progressive snapshot at 10,000 PRs, local pages at 25 rows, changed paths at 2 MiB / 4,096 entries, file pages at 20 paths, repository discovery at 16 identities, and adaptive history fetches at 4,096 commits. PR cards distinguish current-page line counts computed from the selected patch from whole-PR totals reported by GitHub.
6. Potentially large PR subprocess output is read through capped pipes. Crossing a cap kills the child rather than first allocating all output and truncating afterward.
7. Git and GitHub CLI receive argv directly, never via a shell. Every list/lookup request carries its canonical base-repository identity, avoiding ambient-remote ambiguity.
8. PR patches first use immutable base/head OIDs already present in the opened repository, which makes local-branch PR previews network-free. Missing objects fall back to fixed refs in a unique bare repository under the cache/temp root: base and GitHub PR-head refs are shallow/partial fetched, the merge base is deepened adaptively, selected paths are diffed, and `Drop` removes the repository. The opened repository receives no checkout, branch, ref, index, or worktree mutation.
9. Successful raw `gh` metadata responses are atomically cached under the per-user Quinjet cache root. TTL reads, stale-on-error fallback, private Unix modes, a 32 MiB / 256-entry prune limit, and `QUINJET_CACHE_DIR` keep it useful and bounded. Diffs and credentials are never cached.
10. Read operations set `GIT_OPTIONAL_LOCKS=0`; `gh` runs with prompts, paging, color, and update checks disabled on the worker thread.

## VS Code Research

The ignored `extras/vscode` checkout was explored independently. Its central pattern is a provider-neutral SCM workbench driven by a Git-CLI-backed provider. Quinjet adopts the same high-level boundary in a smaller form: UI/app code works with typed snapshots while `git` owns repository semantics.

Important parity findings incorporated in the first implementation:

- Separate merge, staged, and worktree resource groups.
- Event-driven refresh with noise filtering and snapshot reconciliation.
- Git CLI delegation for hooks, credentials, config, refs, and index semantics.
- Paginated history for an explicit branch revision and lazy commit preview.
- Side-by-side and unified syntax-highlighted diff surfaces.
- Context-sensitive operations, confirmations, keyboard help, and optional mouse support.

## Deliberate Next Steps

Full VS Code parity is an ongoing scope rather than a single release. Highest-priority follow-ups are multi-repository actors, non-UTF-8 path preservation, partial hunk staging, a true three-way merge editor, ref-filtered topology lanes, and configurable keymaps.
