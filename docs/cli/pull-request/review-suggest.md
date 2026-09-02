# `quinjet pr reviews suggest`

Adds a pending review comment proposing an exact replacement for a range of
lines.

Usage:

```bash
quinjet pr reviews suggest <number> <path> --line <line> [--start-line <line>] [--note <text>] <--body <text> | --body-file <path>> [--repo <owner/name>] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `<PATH>` | path | required | Repository-relative path to suggest a change to. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--line <LINE>` | integer | required | Last line the suggestion replaces, on the new side. |
| `--start-line <LINE>` | integer | unset | First line, for a multi-line suggestion. |
| `--note <TEXT>` | string | empty | Prose printed above the suggestion block. |
| `-b, --body <TEXT>` | string | one of the two required | The replacement text. |
| `--body-file <PATH>` | path | one of the two required | Read the replacement from a file, or standard input with `-`. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What it writes

The same thing a reviewer writes by hand, without having to remember the fence.
The body becomes a suggestion block, with `--note` above it:

```text
Use the shorter form
<blank line>
<three backticks>suggestion
let value = &[1];
<three backticks>
```

with `<three backticks>` written literally.

It is added to your pending review exactly as
[`quinjet pr reviews comment`](./reviews.md) would, so it does not reach the
author until you submit. Everything about pending reviews there applies here:
the review is created on demand, and a pending review targeting an older head
commit is refused until you submit or discard it.

`--body-file -` reads the replacement from standard input, which is how you
suggest what a formatter produced:

```bash
sed -n '18,22p' src/lib.rs | rustfmt | quinjet pr reviews suggest 42 src/lib.rs \
  --line 22 --start-line 18 --note "rustfmt disagrees here" --body-file -
```

## What it refuses

A replacement containing a fenced code block would close the suggestion fence
early, and everything after it would land in the comment as prose rather than as
the change you meant:

```console
$ quinjet pr reviews suggest 42 src/lib.rs --line 8 --body '```rust'
error: a suggestion cannot contain a fenced code block
```

That is a refusal rather than an escape, because a rewritten suggestion is not
the one you wrote, and posting it would put words in your mouth on somebody
else's pull request.

`--start-line` after `--line` is refused too: GitHub numbers a range from its
first line, and a reversed one would silently comment somewhere else.

## Examples

```bash
quinjet pr reviews suggest 42 src/lib.rs --line 18 -b "let value = &[1];"
quinjet pr reviews suggest 42 src/lib.rs --line 22 --start-line 18 --body-file fix.txt
quinjet pr reviews suggest 42 README.md --line 4 --note "typo" -b "Quinjet"
quinjet pr reviews submit 42 --request-changes -b "A few small things."
```

## Where to go next

- [`quinjet pr suggestions`](./suggestions.md) for reading and applying them
- [`quinjet pr reviews`](./reviews.md) for the rest of the pending-review flow
- [All `quinjet` commands](../README.md)
