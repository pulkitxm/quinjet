# `quinjet pr artifacts`

Lists the artifacts a pull request's workflow runs uploaded, and saves one.

Usage:

```bash
quinjet pr artifacts <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
quinjet pr artifacts download <number> <name> [--into <dir>] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `<NAME>` | string | required for `download` | Artifact name, or a unique part of one. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--into <DIR>` | path | `.` | Directory to write the archive into. `download` only. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second workflow-run cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Listing

Artifacts belong to workflow runs, not to a pull request, so the listing is the
union across every run on the head commit, sorted by name:

```console
$ quinjet pr artifacts 42
ready         5 MiB  coverage                                 Docs
expired      512 B  old-logs                                 CI
ready         2 KiB  snapshots                                CI
```

A run's artifacts are cached under its id, its attempt and its state, so a
finished run's list is immutable and a running one's is on the 30 second clock.
A run whose artifacts cannot be read leaves a `note` line and does not fail the
listing. At most 200 artifacts are listed before `truncated` is set.

## Downloading

```console
$ quinjet pr artifacts download 42 snapshots --into ./ci
Saved ./ci/snapshots.zip
```

The name is resolved the way a check name is: exactly first, then as a unique
case-insensitive substring, exiting 3 with the list of names when it matches
nothing or more than one thing. An expired artifact exits 1 rather than writing
an empty file.

Three things about the write are deliberate:

- **The archive never passes through memory.** Every other GitHub read in
  Quinjet goes through a bounded stdout buffer, which is right for metadata and
  wrong for a zip that can be hundreds of megabytes. This one is streamed
  straight into the file.
- **It is written through a staging name.** The download goes to
  `<name>.zip.part` and is renamed only when `gh` exits successfully, so an
  interrupted download never leaves a truncated file that looks complete.
- **The artifact's name is never used as a path.** GitHub lets a workflow call
  an artifact almost anything, and that name is written by whoever can edit the
  workflow, which on a fork pull request is not necessarily someone you trust. A
  name containing `/`, `\`, `:`, or a control character, or one that is `.`,
  `..`, empty, or begins with `-`, is refused rather than sanitized:

  ```console
  $ quinjet pr artifacts download 42 escape
  error: artifact `../escape` has a name Quinjet will not write to disk
  ```

  Refusing rather than rewriting is the point: a rewritten name is a name the
  caller did not ask for, and silently writing to a different path is worse than
  not writing at all.

## `--json`

```json
{
  "headOid": "aaaa",
  "artifacts": [
    {
      "id": 9001,
      "name": "snapshots",
      "sizeInBytes": 2048,
      "expired": false,
      "expiresAt": "2026-09-20T00:00:00Z",
      "createdAt": "2026-08-21T01:00:00Z",
      "runId": 7701,
      "workflow": "CI",
      "downloadUrl": "https://api.github.com/repos/acme/project/actions/artifacts/9001/zip"
    }
  ],
  "truncated": false,
  "warnings": []
}
```

`download` prints `{"message": "Saved ..."}` rather than the listing.

## Examples

```bash
quinjet pr artifacts 42
quinjet pr artifacts 42 --json | jq -r '.artifacts[] | select(.expired | not) | .name'
quinjet pr artifacts download 42 snapshots
quinjet pr artifacts download 42 coverage --into /tmp/ci
```

Fetching every unexpired artifact:

```bash
quinjet pr artifacts "$PR" --json \
  | jq -r '.artifacts[] | select(.expired | not) | .name' \
  | while read -r name; do
      quinjet pr artifacts download "$PR" "$name" --into ./artifacts
    done
```

## Where to go next

- [`quinjet pr checks runs`](./checks-runs.md) for the runs these came from
- [`quinjet pr deployments`](./deployments.md) for what those runs deployed
- [All `quinjet` commands](../README.md)
