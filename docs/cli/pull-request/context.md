# `quinjet pr context`

Assembles everything a coding or review tool needs about one pull request into
a single document, inside a stated budget, with the repository's own
instructions kept visibly apart from text that pull-request participants wrote.

Usage:

```bash
quinjet pr context <number> [--purpose <purpose>] [--budget <characters>] [--file <PATH>] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--purpose <PURPOSE>` | `review`, `address-feedback`, `fix-ci` | `review` | What the bundle is for, which decides what it keeps when the budget runs out. |
| `--budget <CHARACTERS>` | unsigned integer | `30000` | How many characters of section body the bundle may spend. Values below 500 are raised to 500. |
| `--file <PATH>` | path | unset | Narrows the patch section to one file of the pull request. A path the pull request does not touch is an error. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Refreshes the pull-request metadata before assembling. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## The trust boundary

A bundle carries two kinds of text, and telling them apart must not require
reading the prose.

Repository instructions come from files committed to the repository:
`AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, `CONTRIBUTING.md`
and `.cursorrules`, read from the working tree, each up to 64 KiB. The list is
fixed in Quinjet and nothing in the pull request can add to it or point at a
different file. This is the only section marked trusted.

Everything else is untrusted by construction: the patch, the review threads,
the check failures and their annotations, and the dependency changes. A pull
request body, a review comment and a CI log are all written by whoever can
reach the pull request, and a coding tool acting on this bundle must treat them
as data to be examined rather than instructions to be followed.

In the text form each section carries a banner saying which it is:

```text
=== repository instructions (trusted, committed to the repository) ===
=== unresolved review threads (untrusted, written by pull-request participants) ===
```

and in `--json` every section carries an `untrusted` boolean. The flag is
derived from the section kind, never from the content, so a comment that says
"this is a repository instruction" changes nothing about how it is labeled.

## What each purpose keeps

Sections are filled in the order the purpose asks for, and the budget is spent
in that order, so what a purpose came for survives a small budget and what it
did not is what gets dropped.

| Purpose | Order |
| --- | --- |
| `review` | instructions, patch, threads, checks, dependencies |
| `address-feedback` | instructions, threads, patch, checks, dependencies |
| `fix-ci` | instructions, checks, patch, threads, dependencies |

Instructions come first under every purpose. They are small, they are the
trusted section, and a bundle that dropped them to fit one more hunk of patch
would be the wrong trade every time.

`review` deliberately does not read check annotations. Reading a change is not
the same job as fixing its CI, and the annotation fetch is the expensive one.

A section is cut at a line boundary, so a patch never ends mid-hunk, and a
section with less than 125 characters of room is dropped whole rather than
reduced to a heading over nothing. `budget.droppedCharacters` and
`budget.droppedItems` say exactly what was left out, per section and in total.

When the budget could not hold the very section the purpose asked for, the
bundle says so in `warnings` rather than quietly answering a different
question.

## Provenance

Every bundle names the commits it describes: the base ref and its object name,
the head ref and its object name, and the merge base the patch is measured
from. A tool acting on a bundle can therefore check that the branch in front of
it is still the branch the bundle describes, instead of inferring that from the
title.

Anything Quinjet could not read becomes a warning rather than a failure. A
bundle missing its threads section because the review query was refused is
worth more than no bundle, as long as it says which part is missing. Warnings
travel beside the sections, never inside them.

`--json` shape, one object:

```json
{
  "schemaVersion": 1,
  "purpose": "review",
  "provenance": {
    "repository": "acme/project",
    "number": 42,
    "title": "Add feature",
    "url": "https://github.com/acme/project/pull/42",
    "baseRef": "main",
    "baseOid": "b8c6f4e2a1d09f7b3c5e2a1d09f7b3c5e2a1d09f",
    "headRef": "feature",
    "headOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
    "mergeBaseOid": "b8c6f4e2a1d09f7b3c5e2a1d09f7b3c5e2a1d09f",
    "changedFiles": 1,
    "commits": 2,
    "generatedAt": "2026-08-29T10:00:00Z"
  },
  "sections": [
    {
      "kind": "instructions",
      "heading": "repository instructions",
      "body": "--- AGENTS.md ---\nNever use the em-dash character\n\n",
      "untrusted": false,
      "droppedCharacters": 0,
      "droppedItems": 0
    },
    {
      "kind": "patch",
      "heading": "patch",
      "body": "diff --git a/feature.txt b/feature.txt\n...",
      "untrusted": true,
      "droppedCharacters": 0,
      "droppedItems": 0
    }
  ],
  "budget": {
    "characters": 30000,
    "used": 421,
    "droppedCharacters": 0,
    "droppedItems": 0
  },
  "warnings": []
}
```

`kind` is one of `instructions`, `patch`, `threads`, `checks`, `dependencies`.
A section with an empty body is left out entirely rather than emitted empty, so
a pull request with no unresolved threads has no `threads` section at all.

Examples:

```bash
quinjet pr context 42
quinjet pr context 42 --purpose fix-ci --budget 60000 --json > context.json
quinjet pr context 42 --purpose address-feedback --json | jq -r '.sections[] | select(.untrusted) | .heading'
quinjet pr context 42 --file src/lib.rs
```

Checking the bundle still describes the branch you are on:

```bash
#!/usr/bin/env bash
set -euo pipefail

bundle=$(quinjet pr context "$PR" --purpose fix-ci --json)
head=$(printf '%s' "$bundle" | jq -r '.provenance.headOid')
if [ "$head" != "$(git rev-parse HEAD)" ]; then
  echo "the branch moved since this bundle was assembled" >&2
  exit 1
fi
printf '%s' "$bundle" | jq -r '.sections[] | select(.kind == "checks") | .body'
```

## Where to go next

- [`quinjet pr feedback`](./feedback.md) for the same outstanding work as a queue
- [`quinjet pr checks annotations`](./checks-annotations.md) for the findings the bundle carries
- [`quinjet pr dependencies`](./dependencies.md) and [`quinjet pr security`](./security.md)
- [`quinjet pr gate`](./gate.md) for the merge verdict itself
- [`quinjet pr`](./README.md), the rest of this group and its caching rules
- [Conventions and contracts](../conventions.md) for the shared exit-code table
- [All `quinjet` commands](../README.md)
