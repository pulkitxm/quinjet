# `quinjet pr commits`

Lists one pull request's commits in chronological order, from the oldest commit
still retained in the response through the current head.

Usage:

```bash
quinjet pr commits <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
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
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

After the standard pull-request lookup supplies the exact base and head OIDs,
Quinjet reads GitHub's pull-request commit connection newest-page first:

```text
repository(owner: $owner, name: $name) {
  pullRequest(number: $number) {
    baseRefOid
    headRefOid
    commits(last: 100, before: $before) { ... }
  }
}
```

Each page is capped by the normal 2 MiB GitHub response limit. Quinjet follows
at most five backward cursors, retaining at most 500 commits, then reverses the
pages so the answer reads from oldest to newest. If the pull request has more
than 500 commits, `truncated` is true and the text form names how many of the
newest commits are shown.

Every page must report the base OID, head OID, and total count from the original
metadata snapshot. The newest retained commit must be that head OID, cursors
must not repeat, and commit OIDs must be unique. A force push during pagination
therefore fails visibly with a request to retry instead of returning a mixed
history.

The completed result is cached forever under
`pull-request-commits-v1\n<repository url>\n<number>\n<base oid>\n<head oid>`.
Both boundary commits are part of the key, so a force push or base change asks a
different question. `--refresh` bypasses the five-minute metadata cache; it
does not discard an immutable commit list for an unchanged snapshot.

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
      "authorLogin": "pulkitxm",
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
