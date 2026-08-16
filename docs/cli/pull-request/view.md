# `quinjet pr view`

Prints one pull request's metadata and its description.

Usage:

```bash
quinjet pr view <number> [--repo <owner/name>] [--refresh] [--watch] [--interval <seconds>] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. `0` is rejected at runtime, a non-integer at parse time. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. Matched case-insensitively against `owner/name`, or against the tail of the repository URL. |
| `--refresh` | flag | off | Asks GitHub again instead of answering from the five-minute metadata cache. |
| `--watch` | flag | off | Keeps refreshing the pull-request metadata until stopped. |
| `--interval <SECONDS>` | integer of at least 2 | `5` | Seconds between reads while watching. Requires `--watch`; lower values are usage errors. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the text block. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

This is the lookup and nothing else, so it is the cheapest way to check that
`gh`, the remotes and the number all line up before running a verb that also has
to fetch commits. Underneath it runs exactly one command:

```text
gh pr view <number> --repo <canonical-url> \
  --json number,title,body,author,state,isDraft,createdAt,updatedAt,url,baseRefName,baseRefOid,headRefName,headRefOid,headRepository,isCrossRepository,additions,deletions,changedFiles \
  --jq '[(.number|tostring), .title, (.body // ""), (.author.login // "ghost"), ...] | @tsv'
```

The answer is one tab-separated line of 18 fields, cached under
`pull-request-v3\n<repository url>\n<number>` for five minutes. Tabs, newlines,
carriage returns and backslashes inside the body arrive escaped and are put back
before the record is parsed, which is what lets a multi-line description survive
a one-line format.

The state line is a small piece of interpretation rather than a raw field. A
draft prints `DRAFT` in place of its real state, so `OPEN`, `MERGED`, `CLOSED`
and `DRAFT` are the four things that line can say. `Source` and `Destination`
are labels, not refs: a same-repository pull request shows the bare head branch,
a fork shows `owner/name:branch`, a GitHub Enterprise fork shows
`host/owner/name:branch`, and a pull request whose fork has been deleted shows
`deleted fork:<branch>`. An author GitHub no longer knows prints as `ghost`.

Warnings from repository discovery and from a stale cache fall out on stderr
before the block is written, so a redirect keeps stdout clean:

```console
$ quinjet pr view 8 > pr.txt
warning: GitHub is unavailable; showing stale cached metadata for #8
```

The description is printed verbatim after a blank line, with trailing whitespace
removed and nothing else done to it. Markdown is not rendered, links are not
shortened, and a body over 256 KiB is cut at a character boundary and ends in
`…`. A pull request with an empty body prints no blank line and no body at all.

Without `--watch`, `--json` emits the complete `PullRequestSnapshot`, not the
inner pull request alone. Its fields are `repositories`, `selectedRepository`,
`pullRequest`, `warnings`, `exactNumber`, and `fromCache`. `baseOid` and
`headOid` inside `pullRequest` are the commits
[`quinjet pr files`](./files.md) and [`quinjet pr diff`](./diff.md) work from,
`headRepository` is `null` when the fork has been deleted, and `headRemotes`
lists the local remote names that point at the head repository, which is empty
for a fork you have not added as a remote:

```json
{
  "repositories": [{
    "nameWithOwner": "pulkitxm/quinjet",
    "url": "https://github.com/pulkitxm/quinjet",
    "remotes": ["origin"]
  }],
  "selectedRepository": {
    "nameWithOwner": "pulkitxm/quinjet",
    "url": "https://github.com/pulkitxm/quinjet",
    "remotes": ["origin"]
  },
  "pullRequest": {
    "number": 8,
    "title": "Read pull requests, watch their checks, and index diffs up front",
    "description": "Adds a pull-request pane holding the description, conversation and check logs.",
    "author": "pulkitxm",
    "state": "MERGED",
    "isDraft": false,
    "createdAt": "2026-08-14T19:51:15Z",
    "updatedAt": "2026-08-15T13:19:57Z",
    "url": "https://github.com/pulkitxm/quinjet/pull/8",
    "baseRef": "main",
    "baseOid": "5451c8cc4376a6ea6d8f54043aef5749e262f193",
    "headRef": "feat/pr-conversation-live-checks",
    "headOid": "df8b3a85ed92b0b1b8f11daf2e67ce0431a22d44",
    "baseRepository": {
      "nameWithOwner": "pulkitxm/quinjet",
      "url": "https://github.com/pulkitxm/quinjet",
      "remotes": ["origin"]
    },
    "headRepository": "pulkitxm/quinjet",
    "headRemotes": ["origin"],
    "isCrossRepository": false,
    "additions": 6284,
    "deletions": 602,
    "changedFiles": 14
  },
  "warnings": [],
  "exactNumber": 8,
  "fromCache": false
}
```

`repositories` is the discovered repository list, `selectedRepository` is the
one used for this lookup, `warnings` mirrors non-fatal lookup warnings,
`exactNumber` is the resolved requested number, and `fromCache` says whether
metadata came from a fresh or stale cache entry. Note that `state` in JSON is
the real state, upper-cased, and `isDraft` is
separate. Only the text form collapses the two into `DRAFT`. `updatedAt` is the
stamp [`quinjet pr conversation`](./conversation.md) keys its cache on, so it
moves whenever anything at all happens in the thread.

Examples:

```bash
quinjet pr view 8
quinjet pr view 8 --json
quinjet pr view 8 --refresh
quinjet pr view 8 --watch --interval 10
quinjet pr view 8 --repo pulkitxm/quinjet
quinjet pr view 8 -C ~/code/quinjet --json
```

```console
$ quinjet pr view 8
#8  Read pull requests, watch their checks, and index diffs up front
MERGED · @pulkitxm · opened 2026-08-14T19:51:15Z · updated 2026-08-15T13:19:57Z
Source       feat/pr-conversation-live-checks
Destination  pulkitxm/quinjet:main
Changes      14 files, +6284 -602
URL          https://github.com/pulkitxm/quinjet/pull/8

Adds a pull-request pane holding the description, conversation and check logs, keeps it live, and resolves every changed file's line counts while the index is built.
```

```console
$ quinjet pr view 8 --repo bogus/thing
error: no remote of this checkout points at `bogus/thing`
hint: run `quinjet repos` for the repositories it can see
```

```console
$ quinjet pr view 99999
error: unable to load pull request: GraphQL: Could not resolve to a PullRequest with the number of 99999. (repository.pullRequest)
```

The first of those exits 3, because a name was wrong. The second exits 1,
because `gh` was asked a sensible question and refused it.

Under `--watch`, every reading forces a metadata refresh. Text repaints on a
terminal and appends when redirected; JSON is one compact
`PullRequestSnapshot` per reading. The watch has no settled state and runs until
stopped.

## Where to go next

- [`quinjet pr`](./README.md), the rest of this group and its shared lookup
- [`quinjet pr files`](./files.md) for what the pull request changes
- [All `quinjet` commands](../README.md)
