# `quinjet pr gate`

Answers one question: can this pull request merge, and if not, what stops it.

Usage:

```bash
quinjet pr gate <number> [--repo <owner/name>] [--refresh] [--watch] [--interval <seconds>] [--no-exit-code] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 20 second gate cache for this read. Ignored under `--watch`, which always refreshes. |
| `--watch` | flag | off | Keeps reading until the verdict settles, then exits with it. |
| `--interval <SECONDS>` | integer of at least 2 | `5` | Seconds between reads while watching. Requires `--watch`. |
| `--no-exit-code` | flag | off | Always exit 0, whatever the verdict is. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout, or one compact object per read under `--watch`. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What it combines

Listing check runs answers part of the question. GitHub decides a merge against
rather more than that, and the parts live in different places: the head commit's
status-check rollup, the base branch's rules, the review decision, the review
threads, the merge state, and the merge queue. `pr gate` reads all of them and
reduces them to one word plus the reasons behind it.

The inputs, and where each comes from:

| Signal | Source |
| --- | --- |
| Required and optional checks | The head commit's `statusCheckRollup`, with each context's `isRequired(pullRequestNumber:)` |
| Required contexts that never reported | `baseRef.refUpdateRule.requiredStatusCheckContexts` minus what the rollup holds |
| Approvals, and whether they are current | `latestOpinionatedReviews`, comparing each review's commit against the head |
| Outstanding review requests | `reviewRequests` |
| Requested changes | `latestOpinionatedReviews` with state `CHANGES_REQUESTED` |
| Unresolved threads | `reviewThreads`, with `isResolved` and `isOutdated` |
| Whether resolution is required | `baseRef.refUpdateRule.requiresConversationResolution` |
| Approval count required | `baseRef.refUpdateRule.requiredApprovingReviewCount` |
| Merge conflicts | `mergeable` and `mergeStateStatus` |
| Base branch freshness | `mergeStateStatus`, plus one `compare` read for the exact count |
| Merge queue state | `mergeQueueEntry` |
| Deployment approvals | Rollup entries whose status is `WAITING` or whose conclusion is `ACTION_REQUIRED` |
| Linear history and signature rules | `baseRef.refUpdateRule` |

`refUpdateRule` is the field to know about. It reports the rules that actually
apply to the base branch, aggregated across classic branch protection and
rulesets, and it is readable without administrative permission. That is why the
gate can say "1 of 2 approvals are in place" rather than only "review required":
it knows the number.

## Cost

Two GitHub reads at most. One GraphQL query carries everything above except the
exact behind-count, and one `repos/<owner>/<name>/compare/<head>...<base>` read
supplies that. The comparison is skipped when the two object names are equal,
and its answer is cached forever, because both sides are immutable object names:
a moved base branch asks a different question rather than ageing an old answer.

The GraphQL response is cached for 20 seconds under
`pr-gate-v1\n<repository url>\n<number>\n<head oid>`. Twenty seconds because a
verdict changes when a check goes green, which the head commit does not move
for. The verdict itself is never cached: the cached record is the response, and
the blockers are recomputed from it, so a Quinjet upgrade that improves the
reasoning improves cached answers too.

## The verdict

One of five words, on the first line and in `verdict`:

| Verdict | Exit | Meaning |
| --- | --- | --- |
| `mergeable` | 0 | Nothing Quinjet can see stands in the way. |
| `blocked` | 1 | At least one blocker. They are listed under it. |
| `merged` | 0 | Already merged. |
| `closed` | 1 | Closed without merging. |
| `unknown` | 4 | GitHub has not computed mergeability yet. Read again. |

`--no-exit-code` makes every verdict exit 0, for a caller that would rather read
the JSON than branch on `$?`.

`unknown` is only ever reported when there is nothing else to say. A pull
request whose mergeability GitHub has not computed but whose CI has already
failed is `blocked`, because that answer does not depend on the undecided part.

The rule that matters most: when `mergeStateStatus` is `BLOCKED` and Quinjet
found no blocker of its own, it emits a `policy` blocker rather than reporting
`mergeable`. The gate never claims a merge is clear because it could not see the
reason it is not.

## Blockers

A blocker is something that will actually stop the merge. Each has a `kind`, a
one-line `summary`, and zero or more `details`:

| Kind | Raised when |
| --- | --- |
| `state` | The pull request is a draft, or closed. |
| `conflict` | The head branch conflicts with the base. |
| `ci` | A required check failed, or a required context never reported, or a required check has not finished. |
| `review` | A reviewer requested changes. |
| `approval` | Approvals are short of the required count, or the latest push is unapproved, or GitHub reports a review is still required. |
| `threads` | Threads are unresolved and the base branch requires resolution. |
| `branch` | The head is behind the base and the base requires it to be up to date. |
| `deployment` | A deployment is waiting for a human to approve it. |
| `policy` | GitHub blocks the merge for a rule Quinjet could not name. |
| `queue` | The merge queue reports the entry as unmergeable or locked. |

They are ordered by that table, so the first blocker is always the most
actionable one, and the order is stable across reads. `details` is capped at
eight entries plus a count of what was left out, so one blocker cannot flood the
answer.

Things that do not block the merge are notes rather than blockers, and they
appear under `warnings` and on `note` lines. Unresolved threads on a branch that
does not require resolution, a failing optional check, an approval that applies
to an older commit while enough current ones remain, a merge queue position, and
rules Quinjet could not read are all notes. Keeping the two apart is what makes
`blocked` worth trusting: the blockers list is exactly the set of things that
have to change.

## Text output

```console
$ quinjet pr gate 42
blocked  #42  Add feature
  CI: 1 required check failed
      windows / test failed
  approval: the latest push has not been approved
  threads: 1 unresolved thread
  branch: head is 4 commits behind main

checks    1 of 2 required passed, 1 failed
review    review_required, 0 of 1 approvals, 1 stale, 1 unresolved, requested from hubot
branch    main (4 behind), behind / mergeable
```

The blocker block is the answer; the three summary lines below it are the
numbers behind that answer. A `queue` line and an `auto` line appear when a
merge queue entry or auto-merge is set, and `note` lines carry everything under
`warnings`.

## `--json`

```json
{
  "schemaVersion": 1,
  "repository": "acme/project",
  "number": 42,
  "title": "Add feature",
  "url": "https://github.com/acme/project/pull/42",
  "state": "OPEN",
  "isDraft": false,
  "verdict": "blocked",
  "blockers": [
    {
      "kind": "ci",
      "summary": "1 required check failed",
      "details": ["windows / test failed"]
    }
  ],
  "checks": {
    "checks": [
      {
        "name": "test",
        "workflow": "windows",
        "state": "failed",
        "required": true,
        "url": "https://example.test/windows",
        "awaitingApproval": false
      }
    ],
    "requiredTotal": 2,
    "requiredPassed": 1,
    "requiredFailed": 1,
    "requiredPending": 0,
    "optionalFailed": 0,
    "missingRequired": [],
    "truncated": false
  },
  "review": {
    "decision": "REVIEW_REQUIRED",
    "reviews": [
      { "author": "octocat", "state": "APPROVED", "commitOid": "0000000", "stale": true }
    ],
    "approvals": 1,
    "currentApprovals": 0,
    "staleApprovals": 1,
    "changesRequestedBy": [],
    "requestedReviewers": ["hubot"],
    "requiredApprovals": 1,
    "requiresCodeOwnerReview": false,
    "requiresConversationResolution": true,
    "unresolvedThreads": 1,
    "outdatedUnresolvedThreads": 0,
    "threadsTruncated": false
  },
  "branch": {
    "baseRef": "main",
    "baseOid": "bbbb",
    "headOid": "aaaa",
    "mergeState": "BEHIND",
    "mergeable": "MERGEABLE",
    "behindBy": 4,
    "requiresLinearHistory": false,
    "requiresSignatures": false
  },
  "queue": null,
  "autoMerge": { "enabled": false, "method": "", "enabledBy": "" },
  "warnings": [],
  "fromCache": false
}
```

`schemaVersion` is the contract. It is `1` today, and it changes only when a key
is removed or its meaning changes; new keys are added without bumping it. A
consumer should read `verdict` and `blockers[].kind` and treat everything else
as detail, because those two are the parts a future version will keep.

`behindBy` is `null` rather than `0` when the comparison could not be read, so a
caller can tell "up to date" from "unknown".

## Truncation

The gate reads one page of 100 rollup contexts and one page of 100 review
threads. Crossing either sets `checks.truncated` or `review.threadsTruncated`
and adds a line to `warnings`. It never silently drops a requirement: a pull
request with more than 100 checks reports a partial count and says so, so a
caller can tell a clean gate from an incompletely read one.

The same applies to branch rules. When `refUpdateRule` cannot be read, the
required approval count is 0, required contexts are unknown, and `warnings`
carries a line saying the verdict is inferred from the pull request alone.

## `--watch`

Reads every `--interval` seconds, forcing a refresh each time, and stops as soon
as the verdict is not `unknown`. Then it exits with that verdict's code. Unlike
[`quinjet pr checks --watch`](./checks.md), it does not wait for CI to settle:
a pull request with a required check still running is `blocked` with a `ci`
blocker, which is a settled answer to "can this merge right now".

To wait for green, watch the checks and then read the gate:

```bash
timeout 30m quinjet pr checks "$PR" --watch --interval 30 || true
quinjet pr gate "$PR" --refresh
```

## Examples

```bash
quinjet pr gate 42
quinjet pr gate 42 --json
quinjet pr gate 42 --watch --interval 15
quinjet pr gate 42 --json | jq -r '.blockers[] | "\(.kind): \(.summary)"'
```

Gating a script on it reads naturally, because the exit code is the verdict:

```bash
#!/usr/bin/env bash
set -euo pipefail

if quinjet pr gate "$PR" --json > gate.json; then
  quinjet pr merge "$PR" --squash --yes
else
  case $? in
    1) jq -r '.blockers[] | "\(.kind): \(.summary)"' gate.json >&2; exit 1 ;;
    4) echo "GitHub has not decided yet; try again" >&2; exit 0 ;;
    *) exit 1 ;;
  esac
fi
```

## Where to go next

- [`quinjet stack gate`](../stack/gate.md) for the same verdict across a stack
- [`quinjet pr checks`](./checks.md) for the full check listing behind a `ci` blocker
- [`quinjet pr reviews`](./reviews.md) for the threads behind a `threads` blocker
- [`quinjet pr update-branch`](./update-branch.md) for a `branch` blocker
- [`quinjet pr`](./README.md), the rest of this group and its caching rules
- [Conventions and contracts](../conventions.md) for the shared exit-code table
- [All `quinjet` commands](../README.md)
