# Quinjet Architecture

## Design Goals

Quinjet optimizes for immediate input response, authoritative Git behavior, and predictable memory use. The terminal render path never spawns Git or performs filesystem traversal.

## Layers

```text
argv
  ├── no verb ──► Terminal events
  │                     │
  │                     ▼
  │       App state + intents ──► Ratatui renderer
  │                     │                   ▲
  │                     ▼                   │ snapshot
  │       bounded Git command queue         │
  │                     │                   │
  │                     ▼                   │
  │                Git worker ──────────────┘
  │                     │
  └── repository verb ──┤
                        ▼
                  cli::Command
                        │
                        ▼
              cli::Session::execute ──► Outcome
                        │                 │
                        ▼                 ▼
           Git CLI / GitHub CLI      text or JSON on stdout
                        ▲
                        │
       filesystem watcher, webhook listener

metadata verb ──► generated references / verified updater ──► text or JSON
```

- `src/cli/command.rs`: the one command vocabulary. `Command` names every operation, `Outcome` names every answer, and `Session` owns the repository plus the two reusable diff workspaces.
- `src/cli/mod.rs`: the subcommand tree, its dispatch ahead of terminal setup, the exit-code taxonomy, and the `--json` emitter.
- `src/cli/render.rs`: plain-text renderings of every outcome, with no terminal, no color, and no width assumption.
- `src/cli/update.rs`: latest-release lookup, target selection, checksum verification, and replacement of the running executable.
- `src/cli/watch.rs`: the non-interactive refresh loop, repainting for a terminal and streaming compact JSON for a pipe.
- `src/app.rs`: focus, selection, modal, command, lazy-file, live-check, progress, and generation state.
- `src/git/mod.rs`: safe argv construction, reusable index-first local diff workspaces, branch/stash workflows, branch-scoped history reads, and user-facing Git operations.
- `src/git/status.rs`: byte-oriented porcelain-v2 status parser.
- `src/git/history.rs`: delimiter-safe history parser.
- `src/git/github/mod.rs`: on-demand repository-scoped PR lookup, a reusable local PR workspace, and per-file and batched Git diff generation; it has no repository-wide PR-list operation.
- `src/git/github/checks.rs`: check runs, their GitHub Actions job steps, and raw run logs reduced to timestamped, severity-tagged lines.
- `src/git/github/conversation.rs`: the pull-request timeline and inline review comments flattened into one ordered thread.
- `src/git/diff.rs`: unified-diff model and syntax highlighting for working-tree, commit, and PR patches.
- `src/git/worker.rs`: the non-blocking UI/command boundary. It performs no Git work of its own; it routes each request through `cli::Session` and labels the answer with the generation its caller asked under.
- `src/watch.rs`: repository watcher and event-storm coalescing.
- `src/webhook.rs`: optional loopback listener that turns a forwarded GitHub delivery into a refresh signal.
- `src/ui/`: viewport-only rendering, diff layouts, theme, and mouse hit map.

## Responsiveness Invariants

1. The UI thread mutates only in-memory state and renders visible rows.
1a. There is one command vocabulary for user-visible repository and GitHub operations. The terminal and subcommands execute `cli::Command` through `cli::Session`; presentation state such as focus, scrolling, folding, filtering, and mouse capture stays in the app. The worker adds only a generation tag and a lane; it constructs no argument list. One macro declaration generates the exhaustive route match and one fixture per `GitOperation` variant. Subcommand dispatch happens before the interactive-terminal check and before any terminal mode is entered, so metadata verbs such as `completions`, `man`, and `update` run without repository discovery and a piped invocation never claims the terminal.
2. Every preview, status, history, branch, and pull-request request carries a generation; stale replies are ignored.
3. Fixed coalescing mailboxes isolate status/history, GitHub metadata (lookup, checks, conversation, check logs), local change/commit/branch/stash previews, and potentially network-bound PR previews. Background diff prefetch occupies its own mailbox slot behind the preview slot, so a queued batch can never displace the preview a reader is waiting for. Key repeat remains constant-space, slow GitHub work cannot block tab switches, and ordered user mutations are serialized by app state. No GitHub command is queued at startup or merely by opening the PR tab.
4. Watcher signals are lossy by design: one full status snapshot subsumes all preceding file events.
5. Git/PR patch output is capped at 8 MiB per read. Exact PR/check metadata is capped at 2 MiB, changed-file indexes at 8 MiB / 16,384 entries, conversations at 500 entries, check logs at 8 MiB / 200,000 lines, repository discovery at 16 identities, and adaptive selected-PR history fetches at 4,096 commits. Background prefetch stops after 400 files.
6. Potentially large local and PR subprocess output is read through capped pipes. Crossing a cap kills the child rather than first allocating all output and truncating afterward. Syntax grammar parsing stops at 512 KiB per patch or 32 KiB per row, and collapsed cached patches are not cloned into the combined document, so post-processing remains bounded after Git returns.
7. Git and GitHub CLI receive argv directly, never via a shell. Every explicit PR lookup carries its canonical base-repository identity after lazy remote discovery, avoiding ambient-remote ambiguity.
8. Working-tree groups, commits, branch comparisons, and stashes use one coalesced local workspace. A bounded name/status index produces collapsed headers first; the first path is prefetched silently, and later expansions run path-scoped Git commands whose documents remain cached for that workspace. Periodic status snapshots do not rebuild an unchanged comparison.
8a. Every index also reads `git diff --numstat` over the same range, so a header shows its real `+n -n` before that file has a patch. A file's totals never depend on whether its patch has loaded.
9. PR patches first use immutable base/head OIDs already present in the opened repository, which makes local-branch PR previews network-free. Missing objects fall back to fixed refs in one bare workspace under the cache/temp root: base and GitHub PR-head refs are shallow/partial fetched, the merge base is deepened adaptively, and the workspace remains alive while files are selected. Each selection runs a path-scoped Git diff; `Drop` removes the workspace. The opened repository receives no checkout, branch, ref, index, or worktree mutation.
10. The PR file tree is built from a bounded `git diff --name-status -z` index and renders only viewport rows. Its default all-files document reuses that index with collapsed headers and cached path documents; selecting a tree file enters single-file mode, while selecting the Files tab restores the index. Files and directories are selectable, directory rows expose real collapse/expand controls, and hidden descendants never trigger diff work. File-selection requests are generation-tagged and coalesced; there are no changed-file pages.
10a. Remaining patches arrive through batched background reads keyed to the prepared workspace rather than to a preview generation, so they can never invalidate a reader's own request. One Git invocation answers for a batch of paths and the combined patch is split back apart at its `diff --git` boundaries.
11. PR checks, the conversation, and metadata refresh on one adaptive poll: 5 seconds while a check is running, 20 seconds once they settle, 2 minutes from another view. The tick is a ceiling, not a schedule: every stream keeps its own floor (20 s for metadata and the conversation, 8 s for a growing log) and a finished run's log is never re-read, so the fast cadence costs one request rather than five. A stream that coalesces into an in-flight request stays due instead of being skipped. A forwarded webhook bypasses every floor. Each read is issued and validated independently, refresh failures preserve the previous snapshot, and a poll never clears the loaded pull request, its section, or its cursors. A changed head OID reindexes only the diff. An opt-in loopback webhook listener runs the same refresh immediately; a delivery is a signal, never a source of displayed data.
10b. The pull-request pane composes rows from app state rather than from a diff document, so `End` asks for the end and lets the draw clamp to whatever that pane holds. Each row declares whether it can outgrow the pane: wrapped prose and headers stay fixed, while code, log output, diff context and single-line values scroll horizontally in display columns. Anchoring the header keeps authorship on screen while a comment's code is read sideways.
11a. Check logs are read from the GitHub Actions job endpoint and attached to steps by comparing whole-second instants, because runner lines are sub-second while the steps API reports seconds. Output before the first step or after the last is kept separate, where provisioning and teardown failures live. The endpoint serves whatever a running job has written so far and answers 404 only before its blob exists, so repeating the read is what tails a run; step status and elapsed time come from the jobs API and stay live even while that blob is still missing. The view follows the newest output only while the reader is already at the end.
12. Cached content is split by whether its key already names it. Entries whose key contains their identity are immutable and never expire: a finished run's steps and log keyed by job, a changed-file listing and each file's patch keyed by the merge-base and head commits, a conversation keyed by the stamp GitHub moves on any activity. A new head or a new comment therefore asks a different question rather than aging an old answer, so a stale read is impossible and only eviction applies. Only genuinely time-varying reads keep a clock: repository identity for a day, pull-request metadata for five minutes, the check list for thirty seconds. A run still in progress is never cached, because re-reading it is what tails it.
12a. Every entry is written atomically under the per-user Quinjet cache root with private Unix modes, bounded to 128 MiB / 2,048 entries pruned oldest first, with a 1 MiB ceiling per file patch so one file cannot crowd out a pull request, and relocatable through `QUINJET_CACHE_DIR`. Patches are cached; credentials never are. Because patches are repository content at rest outside the repository, the cache root stays owner-only.
12b. Opening a pull request warms every settled run's log in the background, on the metadata lane behind every interactive read, capped at 32 runs and issued once per set of settled runs. The view carries one reload mark while any pull-request read is in flight and says `cached` when what is on screen came from disk.
13. Read operations set `GIT_OPTIONAL_LOCKS=0`; `gh` runs with prompts, paging, color, and update checks disabled on the worker thread.
14. A session owns the prepared workspaces, which is what makes one layer serve both faces. The terminal keeps a session per worker lane and pays for a prepared pull request once; a subcommand builds a session, prepares, reads, and drops it, so a repeated invocation pays the fetch again and relies on the immutable per-file caches instead. Mutations are serialized by app state inside the terminal only; two concurrent processes can race on the index exactly as two `git` invocations would.
15. The command line never repaints or streams progress into a pipe. Human output is written once per invocation, `--json` is one document, and `--watch` is the single deliberate exception: one compact document per read under `--json`, a repainted screen on a terminal, and appended frames when redirected.
16. Terminal setup becomes recoverable immediately after raw mode is enabled. If any later setup step fails, a rollback guard restores from that first successful mutation. In abort builds the panic hook restores from any thread because destructors do not run. In unwind builds it restores only for a panic on the terminal-owning thread, so a worker panic cannot tear down a terminal whose event loop is still running.
17. Completion scripts and manual pages are metadata reads built directly from clap before repository discovery. Completions support bash, zsh, fish, elvish, and PowerShell. Manual generation fully builds one clap tree, then renders the root and nested commands from it so nested pages retain their full command path and inherited global options.
18. Self-update resolves one stable GitHub release before downloading anything else, pins its checksum and target binary to that tag, verifies SHA-256 before staging beside the running executable, and delegates cross-platform replacement to `self-replace`. An older or equal release never replaces the binary, and any network, checksum, staging, or replacement failure leaves the running path intact.

## VS Code Research

The ignored `extras/vscode` checkout was explored independently. Its central pattern is a provider-neutral SCM workbench driven by a Git-CLI-backed provider. Quinjet adopts the same high-level boundary in a smaller form: UI/app code works with typed snapshots while `git` owns repository semantics.

Important parity findings incorporated in the first implementation:

- Separate merge, staged, and worktree resource groups with per-file and per-group actions.
- Stash creation variants, listing, preview, apply, pop, drop, and clear flows.
- Read-only current-branch comparison against local or remote-tracking branches.
- Event-driven refresh with noise filtering and snapshot reconciliation.
- Git CLI delegation for hooks, credentials, config, refs, and index semantics.
- Paginated history for an explicit branch revision and lazy commit preview.
- Side-by-side and unified syntax-highlighted diff surfaces.
- Context-sensitive operations, confirmations, keyboard help, and optional mouse support.

## Deliberate Next Steps

Full VS Code parity is an ongoing scope rather than a single release. Highest-priority follow-ups are multi-repository actors, non-UTF-8 path preservation, partial hunk staging, a true three-way merge editor, ref-filtered topology lanes, and configurable keymaps.
