# `quinjet pr conversation`

Prints a pull request's whole thread: the description, every comment, every
review and its inline replies, the pushed commits, and the lifecycle events
between them.

Usage:

```bash
quinjet pr conversation <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for the metadata. The conversation itself is keyed by `updatedAt` and can never be stale, so this only helps when the metadata cache is holding an old stamp. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The thread is assembled from two endpoints rather than one, because neither is
complete on its own:

```bash
gh api --paginate repos/<owner>/<name>/issues/<n>/timeline?per_page=100 --jq '<flattener>'
gh api --paginate repos/<owner>/<name>/pulls/<n>/comments?per_page=100 --jq '<flattener>'
```

The timeline carries comments, reviews, commits, force pushes and lifecycle
events. It also carries inline review comments, but only as `line-commented`
groupings and only for some pull requests, so the pulls comments endpoint is
read as well and its records are merged in. Each `--jq` program flattens a
heterogeneous event into one tab-separated record of exactly 8 fields, which is
why an event type GitHub adds tomorrow still arrives with an actor, a timestamp
and a URL rather than breaking the parse.

Merging is by `html_url`: a review comment already present from the timeline is
skipped when the comments endpoint reports it again, and only entries with a
non-empty URL can dedupe, so two events that both have no URL are both kept. The
opening post is synthesized locally from the metadata rather than read from
either endpoint, which is why its body is the description and its `reference` is
the head commit. Everything is then sorted by `timestamp`, which is an RFC 3339
string compared as text; that is correct because GitHub always emits UTC with
the same width.

Events GitHub records but never renders in its own conversation are dropped in
the query rather than after it, so they never cross the wire: `subscribed`,
`unsubscribed`, `mentioned`, `referenced`, `milestoned`, `demilestoned`,
`user_blocked`, `connected`, `disconnected`, `transferred`, `pinned`,
`unpinned`, `locked`, `unlocked`, `marked_as_duplicate`,
`unmarked_as_duplicate`, `comment_deleted`, `deployed` and
`deployment_environment_changed`, plus the two automatic base change events.

Both halves are cached forever under
`conversation-timeline-v1\n<url>\n<number>\n<updatedAt>` and
`conversation-comments-v1\n<url>\n<number>\n<updatedAt>`. GitHub moves
`updatedAt` on any activity in the thread, so the key names this exact
conversation: an unchanged thread is answered from disk without a request, and a
new comment asks a different question rather than ageing an old answer out.
Before falling back to a full paginated read, Quinjet tries a single-page
conditional request carrying `If-None-Match`, which GitHub answers `304 Not
Modified` for an unchanged thread at no cost against the rate limit. A response
spanning more than one page has no single validator and is read in full.

The thread is capped at 500 entries. When it is longer, the oldest are dropped
and the opening post is put back at the front, so the description survives even
on a thread with a thousand events, and the text form says so:

```json
[the conversation reached Quinjet's entry cap and older entries were dropped]
```

A body is capped at 64 KiB and a quoted diff hunk at 8 KiB, both cut at a
character boundary and ended with `…`.

Only five kinds carry prose: `opened`, `comment`, `review`, `review-comment` and
`commit`. Everything else is a header line and nothing more, which is why a
label event prints as a single line. A review with no body prints its verdict in
`detail` and no text.

`--json` shape, one object with an `entries` array. `kind` is a lower-case
hyphenated enum. `detail` is a short qualifier whose meaning depends on the
kind: a review verdict such as `APPROVED`, a label name, a `path:line` anchor
for an inline comment, an abbreviated commit id, the old title for a rename, the
reviewer or assignee for a request, or `<head> into <base>` for the opening
post. `reference` is the stable identity of the underlying object: a commit OID,
a review id, or the post-rename title. `context` is currently only a review
comment's quoted diff hunk. `fromCache` is true when nothing had to be
transferred, either because the thread was already held for this stamp or
because GitHub confirmed it had not changed:

```json
{
  "entries": [
    {
      "kind": "opened",
      "actor": "pulkitxm",
      "timestamp": "2026-08-14T19:51:15Z",
      "detail": "feat/pr-conversation-live-checks into main",
      "body": "Adds a pull-request pane holding the description, conversation and check logs.",
      "url": "https://github.com/pulkitxm/quinjet/pull/8",
      "reference": "df8b3a85ed92b0b1b8f11daf2e67ce0431a22d44",
      "context": ""
    },
    {
      "kind": "labeled",
      "actor": "github-actions[bot]",
      "timestamp": "2026-08-14T19:51:24Z",
      "detail": "rust",
      "body": "",
      "url": "",
      "reference": "",
      "context": ""
    },
    {
      "kind": "review-comment",
      "actor": "reviewer",
      "timestamp": "2026-08-15T09:12:04Z",
      "detail": "src/git/mod.rs:42",
      "body": "Extract this into its own function.",
      "url": "https://github.com/pulkitxm/quinjet/pull/8#discussion_r1",
      "reference": "99",
      "context": "@@ -40,3 +40,6 @@"
    }
  ],
  "truncated": false,
  "fromCache": true
}
```

The full set of `kind` values is `opened`, `comment`, `review`,
`review-comment`, `commit`, `force-push`, `merged`, `closed`, `reopened`,
`labeled`, `unlabeled`, `renamed`, `ready-for-review`, `converted-to-draft`,
`review-requested`, `review-request-removed`, `assigned`, `unassigned`,
`cross-referenced`, `head-ref-deleted`, `head-ref-restored`, `base-ref-changed`
and `other`. Anything GitHub introduces later arrives as `other` with its actor
and timestamp intact.

Note that the top level here is an object, not an array, unlike
`quinjet log` and `quinjet branch list`. The entries live under `entries` so
that `truncated` and `fromCache` have somewhere to go.

Examples:

```bash
quinjet pr conversation 8
quinjet pr conversation 8 --json
quinjet pr conversation 8 --json | jq -r '.entries[] | select(.kind == "comment") | .actor'
quinjet pr conversation 8 --repo pulkitxm/quinjet
```

```console
$ quinjet pr conversation 8

@pulkitxm opened this feat/pr-conversation-live-checks into main  (2026-08-14T19:51:15Z)
  Adds a pull-request pane holding the description, conversation and check logs, keeps it live, and resolves every changed file's line counts while the index is built.

@github-actions[bot] added the label rust  (2026-08-14T19:51:24Z)

@Pulkit pushed 542ff8c  (2026-08-14T19:54:51Z)
  feat: refresh instantly from forwarded GitHub webhooks
  
  Polling is the floor for liveness, not the ceiling. An opt-in loopback
  listener lets `gh webhook forward` push deliveries straight into the
  session, turning a check-run or comment event into an immediate refresh
  instead of a wait for the next poll.

@pulkitxm force-pushed  (2026-08-14T21:38:37Z)

@pulkitxm merged this  (2026-08-15T13:19:41Z)

@pulkitxm closed this  (2026-08-15T13:19:41Z)

@pulkitxm deleted the head branch  (2026-08-15T13:19:57Z)
```

Two things about that output are worth knowing. A commit entry's actor is the
Git author name from the commit, not a GitHub login, so `@Pulkit` and
`@pulkitxm` in the same thread are the same person under two identities.
A body is printed indented by two spaces with its own line breaks preserved, and
a quoted hunk is printed above it prefixed with two spaces, a pipe and a space, so a review comment
shows the code it is about before the comment itself.

## Where to go next

- [`quinjet pr`](./README.md), the rest of this group and the shared lookup
- [`quinjet pr checks`](./checks.md) for the CI half of the same pull request
- [All `quinjet` commands](../README.md)
