# `quinjet pr reviews`

Reads and writes GitHub pull-request review threads. The same command family
backs the review controls in the terminal interface.

Usage:

```bash
quinjet pr reviews show <number>
quinjet pr reviews comment <number> <path> --line <line> --side <left|right> --body <text>
quinjet pr reviews comment <number> <path> --file --body <text>
quinjet pr reviews reply <number> <thread-id> --body <text>
quinjet pr reviews edit <number> <comment-id> --body <text>
quinjet pr reviews delete <number> <comment-id> [--yes]
quinjet pr reviews submit <number> <--comment|--approve|--request-changes> --body <text>
quinjet pr reviews discard <number> [--yes]
quinjet pr reviews resolve <number> <thread-id>
quinjet pr reviews unresolve <number> <thread-id>
quinjet pr reviews progress <number> [--since <oid> | --since-review] [--all]
quinjet pr reviews next <number> [--files | --threads]
quinjet pr reviews viewed <number> [<path>...] [--all] [--unviewed] [--reset]
quinjet pr reviews visit <number>
```

Every verb accepts `--repo <owner/name>`, `--refresh`, `-C <DIR>`, and `--json`.
Any body can come from `--body <TEXT>` or `--body-file <PATH>`. Use
`--body-file -` to read it from standard input.

`show` returns the pull request's head commit, review decision, the current
viewer's pending review, and up to 500 review threads. Each thread includes its
path, old or new side, line range, resolved and outdated state, permissions,
and up to 100 comments. `truncated` and `commentsTruncated` say when those
bounds were reached.

Line comments use GitHub's blob coordinates. `--side right` addresses the new
file and `--side left` addresses the old file. A range also needs
`--start-line` and `--start-side`; neither can be supplied alone. `--file`
creates a file-level thread and cannot be combined with line coordinates.

The first comment creates a pending review. Later comments and replies join
that review. A force push makes a pending review stale when its stored commit
no longer matches the pull request head; Quinjet refuses to add or submit in
that state instead of silently anchoring feedback to another commit. Discard
the stale review before starting again.

`delete` and `discard` are previews without `--yes`. Their text output says
what would be removed and leaves GitHub unchanged. `--json` returns the same
current review snapshot as the other review mutations after a confirmed write.

Examples:

```bash
quinjet pr reviews show 42
quinjet pr reviews comment 42 src/lib.rs --line 18 --side right -b "Handle the empty case here."
quinjet pr reviews comment 42 README.md --file --body-file review.txt
printf '%s\n' "This is fixed now." | quinjet pr reviews reply 42 PRRT_kwDOAA --body-file -
quinjet pr reviews submit 42 --approve -b "Looks good to me."
quinjet pr reviews resolve 42 PRRT_kwDOAA
```

The implementation uses GitHub's GraphQL review-thread and review mutations,
including `addPullRequestReviewThread`, `addPullRequestReviewComment`, and
`submitPullRequestReview`. It does not use the deprecated integer diff
position. Requests run through `gh api graphql` on the pull request's GitHub
host, so GitHub Enterprise stays scoped to the repository selected by the
shared pull-request lookup.

## Terminal interface

Open a pull request, switch to Files with `Shift+F`, select one file, and move
focus into its diff. `j` and `k` select a reviewable line. The selected row is
highlighted and existing threads render immediately below their anchors.

| Key | Action |
| --- | --- |
| `c` | Add a line comment to the pending review. |
| `C` | Add a file-level comment. |
| `a` | Reply to the thread on the selected line. |
| `x` | Resolve or reopen the thread on the selected line. |
| Click a thread | Open its permission-aware reply, copy, browser, edit, delete, and state actions. |
| `Shift+V` | Choose comment, approve, or request changes and submit the review. |
| `Ctrl+Enter` | Save the current comment or submit modal. |

Review reads and writes use a dedicated worker lane, so a slow GitHub review
request cannot block diff loading, check logs, or the rest of the interface.

The last four verbs read and record review progress rather than writing to
GitHub. They keep a local note of which files you have read and which head you
last looked at, which is what makes "show me only what changed since" answerable
on a pull request that has moved. They are documented separately:

- [`quinjet pr reviews progress`](./review-progress.md)
- [`quinjet pr reviews next`](./review-next.md)
- [`quinjet pr reviews viewed`](./review-viewed.md)
- [`quinjet pr reviews visit`](./review-visit.md)

## Where to go next

- [`quinjet pr`](./README.md), the shared pull-request lookup and other verbs
- [`quinjet pr conversation`](./conversation.md), the chronological timeline
- [`quinjet tui`](../tui.md), the complete terminal interface
