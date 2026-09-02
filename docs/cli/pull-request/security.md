# `quinjet pr security`

Lists the security findings a pull request raises: the code-scanning alerts
open on its head branch, and the vulnerable dependencies it introduces.

Usage:

```bash
quinjet pr security <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Refreshes the pull-request metadata. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Two reads underneath. Code scanning is asked for the alerts open on the head
branch:

```text
gh api 'repos/<owner>/<name>/code-scanning/alerts?ref=refs/heads/<head>&state=open&per_page=100'
```

and the vulnerable dependencies come from the same dependency review
[`quinjet pr dependencies`](./dependencies.md) reads, so the two verbs never
disagree about what a pull request pulls in.

Alerts keep a 60 second cache under
`code-scanning-v1\n<repository url>\n<head ref>`, because unlike a dependency
comparison the alert list genuinely changes while a branch is being worked on.

## An unreadable source is never a clean one

Both reads can be refused. Code scanning is a paid feature on private
repositories, it can be switched off, and a token can lack the scope. The
dependency graph is the same.

A refusal is a warning, not a failure and not silence. The verb still prints
everything it could read, records what it could not under `warnings`, and never
counts an unreadable source as zero findings. That distinction is the whole
point of the verb: "no alerts" and "nobody would tell me about the alerts" must
not look the same to a script that gates on this.

## Exit codes

| Code | When |
| --- | --- |
| 0 | Nothing critical or high, including when a source was unreadable. |
| 1 | At least one critical or high finding, from either source. |

Note that this is the one exit code in the group that is a verdict rather than
a report of failure, so the listing is still written to stdout in full before
the process exits 1. It follows the same rule as
[`quinjet pr checks --exit-code`](./checks.md).

`--json` shape, one object:

```json
{
  "schemaVersion": 1,
  "headOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
  "alerts": [
    {
      "number": 7,
      "rule": "rust/command-injection",
      "severity": "critical",
      "description": "Shell argument built from user input",
      "path": "src/main.rs",
      "line": 12,
      "url": "https://github.com/acme/project/security/code-scanning/7"
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
  "critical": 1,
  "high": 1,
  "other": 0,
  "truncated": false,
  "warnings": []
}
```

`severity` is one of `critical`, `high`, `moderate`, `low`, `unknown`, and the
same scale covers both lists so the counts can be added. GitHub's code-scanning
words are folded into it: `error` reads as `high`, `warning` as `moderate`,
`note` as `low`. Findings sort by severity and then by location, and an alert
list longer than 200 is cut with `truncated` set.

Examples:

```bash
quinjet pr security 42
quinjet pr security 42 --json | jq -r '.alerts[] | "\(.path):\(.line) \(.rule)"'
quinjet pr security 42 || echo "do not merge yet"
```

```console
$ quinjet pr security 42
critical  src/main.rs:12                     Shell argument built from user input
high      left-pad 1.0.0                     Prototype pollution  fixed in 1.0.1

1 critical, 1 high, 0 other
```

## Where to go next

- [`quinjet pr dependencies`](./dependencies.md) for the whole dependency delta
- [`quinjet pr gate`](./gate.md) for the merge verdict itself
- [`quinjet pr feedback`](./feedback.md) for everything outstanding in one queue
- [`quinjet pr`](./README.md), the rest of this group and its caching rules
- [Conventions and contracts](../conventions.md) for the shared exit-code table
- [All `quinjet` commands](../README.md)
