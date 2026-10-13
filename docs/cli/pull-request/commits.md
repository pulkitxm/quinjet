# `quinjet pr commits`

Lists every commit reachable from a pull request's head, from the initial commit
through the pushed head. Local commits ahead of that head are excluded.

Usage:

```bash
quinjet pr commits <number> [--repo <owner/name>] [--refresh] [--introduced-only] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. `0` is rejected at runtime, a non-integer at parse time. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for metadata. A changed head produces a new immutable commit-list key. |
| `--introduced-only` | flag | off | Limits the answer to `base..head`, the commits introduced by the PR. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

After the standard pull-request lookup supplies the exact head OID, Quinjet
reads `git rev-list` and `git log` starting at that head in the opened repository.
This includes shared ancestors already on the base branch. It excludes local
commits beyond the pushed PR head and commits on unrelated branches.
Commit hashes, subjects, authors, and timestamps come from local Git objects,
including objects shared by linked worktrees. There is no commit-count limit.

Reads stream one record at a time, with a 2 MiB safety limit per record. The
ancestry scan checks shallow boundaries so incomplete history is never presented
as complete. Local results omit `authorLogin`, which is a GitHub account identity
rather than Git commit metadata.

If the head or required ancestry is missing, Quinjet fetches that exact head and
its complete history through Git into a disposable bare repository. It borrows
complete local object stores first and never changes the opened repository's
refs, index, checkout, or shallow boundaries. GitHub's PR commit connection is
not used for full history because it only lists the PR's introduced commits.

`--introduced-only` reads `base..head` locally. When that range is unavailable,
it uses GitHub's commit connection in complete 100-commit pages. Every page
must preserve the base OID, head OID, and total count, cursors must not repeat,
and commit OIDs must be unique. The stack member view uses this narrower range.

Complete head history is cached under `head-history-v1` plus the repository,
PR number, base OID, and head OID. Introduced-only snapshots use
`pull-request-commits-v2`, prefixed with `local-` for local reads. Separate keys
prevent introduced-only snapshots from replacing complete history. `--refresh`
bypasses the metadata cache; a force push supplies a new exact head and a new
history snapshot.

The text form prints one row per commit with its abbreviated OID, authored time,
author, and subject:

```console
$ quinjet pr commits 93
68ec58a6dd49  2026-08-20 19:14:21  Pulkit               feat: add stack discovery
af8d97525585  2026-08-21 08:42:09  Pulkit               fix: preserve exact range heads
```

`--json` prints one object:

```json
{
  "commits": [
    {
      "oid": "af8d975255850f0e8026339abdd75533e21ed40c",
      "abbreviatedOid": "af8d97525585",
      "subject": "fix: preserve exact range heads",
      "author": "Pulkit",
      "authorLogin": null,
      "authoredAt": "2026-08-21T08:42:09Z",
      "committedAt": "2026-08-21T08:42:09Z",
      "url": "https://github.com/pulkitxm/quinjet/commit/af8d975255850f0e8026339abdd75533e21ed40c"
    }
  ],
  "totalCommits": 1,
  "truncated": false,
  "baseOid": "68ec58a6dd49e6fb496243183d19154c27706e66",
  "headOid": "af8d975255850f0e8026339abdd75533e21ed40c",
  "fromCache": false
}
```

There is no `--watch` flag. The command is one exact snapshot suitable for a
script or for the terminal's selected-member detail. Run it again with
`--refresh` after a known force push.

Exit 0 prints the listing. Exit 1 means repository discovery, metadata lookup,
GitHub pagination, snapshot validation, or output parsing failed. Exit 2 is a
command-line usage error. Exit 3 means `--repo` did not match a discovered
repository.

## Where to go next

- [`quinjet pr`](./README.md) for repository selection and shared cache behavior
- [`quinjet pr conversation`](./conversation.md) for commits among review events
- [`quinjet pr diff`](./diff.md) for the cumulative patch at the current head
- [All `quinjet` commands](../README.md)
