# `quinjet pr suggestions`

Lists the suggested changes reviewers left, and applies them to the working
tree.

Usage:

```bash
quinjet pr suggestions <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
quinjet pr suggestions apply <number> <--all | <suggestion-id>> [--message <text>] [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `<SUGGESTION_ID>` | string | one of the two required for `apply` | A review comment id, or a unique prefix of one. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Apply every suggestion that can be applied. |
| `--message <TEXT>` | string | unset | Record the result as one commit with this message. |
| `--yes` | flag | off | Confirm; without it the command reports what it would change. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the metadata cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Listing

A suggestion is a ` ```suggestion ` block in a review comment, and GitHub applies
it to the thread's line range. One comment can carry more than one; each becomes
its own row, keyed by the comment id plus its position:

```console
$ quinjet pr suggestions 42
COMMENT_1                feature.txt:1                +1 -1  @hubot  ready
COMMENT_2                src/lib.rs:12-14             +2 -3  @hubot  a later commit moved the code it was written against

1 ready to apply, 1 blocked
```

`ready` means the pull request's own state allows it. Three states do not:

| Blocker | Meaning |
| --- | --- |
| `outdated` | A later commit moved the code it was written against, so its line numbers no longer mean anything. |
| `resolved` | Its thread is resolved already. |
| `no-line-range` | GitHub reported no line range for the thread. |

Those come from GitHub. Problems in your checkout are reported by the plan
instead, because they depend on which commit you have out.

## Applying

```console
$ quinjet pr suggestions apply 42 COMMENT_1
Would apply 1 suggestion(s) across 1 file(s), +1 -1
  feature.txt  +1 -1
Pass --yes to write them.
```

```console
$ quinjet pr suggestions apply 42 --all --message "fix: address review" --yes
Applied 2 suggestion(s) across 2 file(s), +4 -3 and committed them
```

Without `--message` the files are left changed and unstaged, which is the right
default when you want to look before committing. With it, exactly the files the
plan touched are staged and recorded, so an unrelated edit elsewhere in the tree
stays out of the commit.

## What it refuses, and why

A suggestion's line numbers only mean something against the commit it was
written for, so applying one to a different tree would edit the wrong lines
quietly. Four checks stand in the way, all before anything is written:

- **The worktree must be at the pull request's head.** Otherwise the file is at
  a different revision and the line range points somewhere else:

  ```console
  $ quinjet pr suggestions apply 42 COMMENT_1 --yes
  error: this worktree is at 5c1234693375 but the pull request's head is 3180ef896154; check the branch out first
  ```

  This is checked before the plan is worked out, so the answer names the branch
  to check out rather than reporting that there is nothing to apply.

- **The files must have no uncommitted changes.** Writing over a local edit
  would lose work that has never been committed anywhere:

  ```console
  $ quinjet pr suggestions apply 42 COMMENT_1 --yes
  error: these files have uncommitted changes: feature.txt
  ```

- **Two suggestions must not overlap.** There is no order to apply them in that
  is obviously what the reviewers meant, so both are skipped with the reason
  rather than one silently winning.

- **The range must be inside the file.** A suggestion past the end of the file
  is skipped and says how long the file actually is.

Every skip is reported. A suggestion that quietly did not apply is worse than
one that says why:

```console
$ quinjet pr suggestions apply 42 --all
Would apply 1 suggestion(s) across 1 file(s), +1 -1
  feature.txt  +1 -1
  skipped src/lib.rs:12-14: a later commit moved the code it was written against
Pass --yes to write them.
```

Files are written all or none: a partial application would leave a tree nobody
asked for, so a failure part-way restores what was already written.

## Writing one

[`quinjet pr reviews suggest`](./review-suggest.md) composes the block for you,
so you do not have to remember the fence.

## `--json`

```json
{
  "schemaVersion": 1,
  "number": 42,
  "headOid": "aaaa",
  "suggestions": [
    {
      "id": "COMMENT_1",
      "threadId": "THREAD_1",
      "author": "hubot",
      "path": "feature.txt",
      "startLine": 1,
      "endLine": 1,
      "replacement": "from pull request, renamed",
      "comment": "Please rename this file",
      "url": "https://github.com/acme/project/pull/42",
      "outdated": false,
      "resolved": false,
      "blocker": null
    }
  ],
  "applicable": 1,
  "blocked": 0,
  "truncated": false,
  "warnings": []
}
```

`replacement` is the block's contents without its fence and without a trailing
newline. An empty `replacement` deletes the lines.

## Examples

```bash
quinjet pr suggestions 42
quinjet pr suggestions apply 42 COMMENT_1 --yes
quinjet pr suggestions apply 42 --all --message "fix: address review" --yes
quinjet pr suggestions 42 --json | jq -r '.suggestions[] | select(.blocker == null) | .id'
```

Applying everything and resolving the threads it came from:

```bash
quinjet pr suggestions apply "$PR" --all --message "fix: apply suggestions" --yes
quinjet pr suggestions "$PR" --json \
  | jq -r '.suggestions[] | select(.blocker == null) | .threadId' \
  | sort -u \
  | while read -r thread; do
      quinjet pr reviews resolve "$PR" "$thread"
    done
```

## Where to go next

- [`quinjet pr reviews suggest`](./review-suggest.md) for writing one
- [`quinjet pr feedback`](./feedback.md) for everything outstanding, not only these
- [`quinjet pr reviews`](./reviews.md) for replying and resolving
- [All `quinjet` commands](../README.md)
