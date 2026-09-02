# `quinjet pr checks annotations`

Lists the annotations a pull request's check runs placed on its lines, and says
which of them the pull request's own patch actually shows.

Usage:

```bash
quinjet pr checks annotations <number> [--severity <level>] [--check <name>] [--file <path>] [--in-diff] [--group <by>] [--full] [--exit-code] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--severity <LEVEL>` | `failure`, `warning`, `notice` | unset | Only annotations at this level. |
| `--check <NAME>` | string | unset | Only check runs whose name contains this, case-insensitively. |
| `--file <PATH>` | path | unset | Only annotations under this path. `--path` is taken by the global repository option. |
| `--in-diff` | flag | off | Only annotations on lines the pull request's patch shows. |
| `--group <BY>` | `file`, `check`, `severity` | `file` | How to group the listing. |
| `--full` | flag | off | Print each annotation's whole message and raw details. |
| `--exit-code` | flag | off | Exit 1 when any listed annotation is a failure. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second check-run cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## The layer between a check and a log

[`quinjet pr checks`](./checks.md) says a run failed.
[`quinjet pr logs`](./logs.md) prints everything it wrote. Annotations are the
structured middle: a path, a line range, a level, a title, and a message, which
is exactly the shape an editor draws in a gutter and a reader wants first.

```console
$ quinjet pr checks annotations 42

README.md
  i  notice   2        Spell Checker  [outside diff]  (Spell check)

feature.txt
  x  failure  1        use a slice  (Clippy)
  !  warning  90-92    This block is long  [outside hunks]  (Clippy)

1 failure, 1 warning, 1 notice · 1 on changed lines, 2 elsewhere
```

The glyph column is `x` failure, `!` warning, `i` notice. A row never repeats
what its grouping heading already said, so `--group check` prints the whole
`path:line` instead of a bare line number and drops the trailing check name.

## Placement

The bracketed word is the part that is not in the GitHub API. A check run
annotates the repository, not the pull request, so an annotation can point
somewhere the pull request never touched. Quinjet decides which, and says so
rather than dropping either kind:

| Placement | JSON | Meaning |
| --- | --- | --- |
| (none) | `in-diff` | The path and line are both inside the pull request's patch. |
| `[outside hunks]` | `outside-hunks` | The pull request changes the file, but not at that line. |
| `[outside diff]` | `outside-diff` | The pull request does not touch the file at all. |
| `[unplaced]` | `unknown` | No patch was loaded for the file, so this is unresolved. |

`outside hunks` is the one worth knowing about: a linter that runs over the whole
repository reports pre-existing findings alongside new ones, and only the first
kind is this pull request's problem. `--in-diff` keeps just the annotations a
reviewer can act on here.

Placement is decided against the pull request's own patch, by loading the
patches of the annotated paths the pull request changes and comparing each
annotation's line range with the new-side lines those patches render. Only
annotated paths are loaded, in batches of 16, so a 200 file pull request with
three annotations reads three files. An annotation on a file the pull request
never touched is answered without loading anything.

## Cost

One request lists the head commit's check runs, and each run that reports a
non-zero annotation count costs one more. A run reporting no annotations is
never asked for them, which is what keeps a green pull request to a single
request.

The check-run list is cached for 30 seconds under
`check-runs-v1\n<repository url>\n<head oid>`, the same clock the check listing
uses, because which runs have annotations changes as runs finish. A run's
annotations are cached under its id, its reported count, and its status, so a
run that publishes another annotation asks a different question rather than
ageing an old answer.

At most 32 annotated check runs are read and at most 500 annotations are listed.
Crossing either sets `truncated` and adds a line to `warnings`; a run whose
annotations cannot be read leaves a warning and does not fail the command.

## Ordering

Severity, then path, then start line, then end line, then check name. The order
does not depend on which check run answered first, so the same pull request
lists the same way on every read, and "the next annotation" means something
stable to a client walking the list.

Filters narrow the rows and the counts together, so the summary line always
describes what is printed. `--exit-code` reads the filtered counts too:
`--severity notice --exit-code` exits 0 on a pull request whose Clippy run
failed, because no failure survived the filter.

## `--json`

```json
{
  "schemaVersion": 1,
  "headOid": "aaaa",
  "annotations": [
    {
      "check": "Clippy",
      "checkRunId": 123456,
      "checkUrl": "https://github.com/acme/project/runs/123456",
      "path": "feature.txt",
      "startLine": 1,
      "endLine": 1,
      "startColumn": 3,
      "endColumn": 9,
      "severity": "failure",
      "title": "use a slice",
      "message": "This vector is never resized",
      "rawDetails": "consider &[T]",
      "url": "https://example.test/a1",
      "placement": "in-diff"
    }
  ],
  "counts": {
    "failure": 1,
    "warning": 1,
    "notice": 1,
    "inDiff": 1,
    "outsideDiff": 2
  },
  "truncated": false,
  "fromCache": false,
  "warnings": []
}
```

`check` is the check run's name exactly as [`quinjet pr logs`](./logs.md)
accepts it, which is the jump from an annotation to the output that produced it.
`checkUrl` is the run's page in a browser. A missing column is `null` rather
than `0`, because column zero does not exist.

## Examples

```bash
quinjet pr checks annotations 42
quinjet pr checks annotations 42 --severity failure
quinjet pr checks annotations 42 --in-diff --group check
quinjet pr checks annotations 42 --check clippy --full
```

Failing a script only on findings this pull request introduced:

```bash
quinjet pr checks annotations "$PR" --in-diff --severity failure --exit-code
```

Reading the log behind the first failing annotation:

```bash
check=$(quinjet pr checks annotations "$PR" --severity failure --json \
  | jq -r '.annotations[0].check // empty')
[ -n "$check" ] && quinjet pr logs "$PR" "$check"
```

Feeding an editor's quickfix list:

```bash
quinjet pr checks annotations "$PR" --json \
  | jq -r '.annotations[] | "\(.path):\(.startLine):\(.startColumn // 1): \(.severity): \(.title)"'
```

## Where to go next

- [`quinjet pr checks`](./checks.md) for the run listing these come from
- [`quinjet pr logs`](./logs.md) for the output behind an annotation
- [`quinjet pr gate`](./gate.md) for whether those failures block the merge
- [`quinjet pr`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
