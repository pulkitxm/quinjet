# `quinjet pr dependencies`

Lists what a pull request does to the dependency graph: what it adds, what it
removes, what it moves to a different version, and where a license changed
underneath a package that stayed.

Usage:

```bash
quinjet pr dependencies <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Refreshes the pull-request metadata. The comparison itself is never stale, so this only affects which commits are compared. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Underneath, this is GitHub's dependency review of the two commits:

```text
gh api repos/<owner>/<name>/dependency-graph/compare/<base oid>...<head oid>
```

Both sides of that comparison are object names, so the answer to this exact
question can never change. The response is cached under
`dependency-review-v1\n<repository url>\n<base oid>\n<head oid>` with no clock
at all: a new push asks a different question and gets a different cache entry,
and an old one is still correct if you go back to it.

## Why an upgrade is one row and not two

GitHub reports a version bump as a removal of the old version and an addition
of the new one. Read literally that is two rows a reviewer has to pair up by
eye, and on a lockfile refresh there can be a hundred of them.

Quinjet pairs them itself. A removal and an addition are the same package when
the ecosystem, the name and the manifest all match, and when they do the pair
becomes one `changed` row carrying both versions and both licenses. That is
also what makes a license change visible: a package that went from MIT to
Apache-2.0 at the same time as a version bump is one row saying exactly that,
rather than two rows whose licenses happen to differ.

Pairing is per manifest deliberately. The same package leaving `a/Cargo.lock`
and arriving in `b/Cargo.lock` is a move between two manifests, not an upgrade,
and it stays two rows.

Rows sort by change kind and then by ecosystem and name, so the same pull
request produces the same order on every read, and additions are read before
removals because an addition is the one that can carry a new advisory.

`--json` shape, one object:

```json
{
  "schemaVersion": 1,
  "baseOid": "b8c6f4e2a1d09f7b3c5e2a1d09f7b3c5e2a1d09f",
  "headOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
  "changes": [
    {
      "change": "changed",
      "ecosystem": "cargo",
      "name": "serde",
      "version": "1.1.0",
      "previousVersion": "1.0.0",
      "manifest": "Cargo.lock",
      "scope": "runtime",
      "license": "Apache-2.0",
      "previousLicense": "MIT",
      "vulnerabilities": 0
    }
  ],
  "vulnerabilities": [
    {
      "package": "left-pad",
      "version": "1.0.0",
      "severity": "high",
      "advisory": "GHSA-1234",
      "summary": "Prototype pollution",
      "firstPatchedVersion": "1.0.1"
    }
  ],
  "added": 1,
  "removed": 0,
  "changed": 1,
  "licenseChanges": 1,
  "truncated": false,
  "warnings": []
}
```

`change` is one of `added`, `removed`, `changed`. `scope` is `runtime`, `dev`
or `unknown`. `previousVersion` and `previousLicense` are empty strings unless
the row is a pairing, and `firstPatchedVersion` is empty when the advisory has
no fixed release yet. A comparison wider than 500 changes is cut and sets
`truncated`.

Examples:

```bash
quinjet pr dependencies 42
quinjet pr dependencies 42 --json | jq -r '.changes[] | select(.change == "added") | .name'
quinjet pr dependencies 42 --json | jq -e '.vulnerabilities | length == 0'
```

```console
$ quinjet pr dependencies 42
changed  runtime   cargo:serde                        1.0.0 -> 1.1.0  license MIT -> Apache-2.0
added    dev       npm:left-pad                       1.0.0

1 added, 0 removed, 1 changed, 1 license change(s)

high      left-pad 1.0.0                     Prototype pollution  fixed in 1.0.1
a dependency this pull request introduces has a known serious advisory
```

The verb exits 0 whatever it finds. A pull request whose dependencies you want
to gate on belongs in [`quinjet pr security`](./security.md), which exits 1 on
a critical or high finding.

A repository without the dependency graph enabled, or a token without the
scope, makes this verb fail rather than report nothing: an empty answer and an
unavailable one are different, and this verb refuses to blur them.

## Where to go next

- [`quinjet pr security`](./security.md) for what the findings mean for merging
- [`quinjet pr context`](./context.md), which carries these changes into a bundle
- [`quinjet pr gate`](./gate.md) for the merge verdict itself
- [`quinjet pr`](./README.md), the rest of this group and its caching rules
- [Conventions and contracts](../conventions.md) for the shared exit-code table
- [All `quinjet` commands](../README.md)
