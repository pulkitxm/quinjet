# `quinjet pr open`

Hands a pull request's URL, or one selected check URL, to the desktop browser.
Inside an SSH session it prints the URL instead, because the desktop browser is
not on the machine running the command.

Usage:

```bash
quinjet pr open <number> [--repo <owner/name>] [--refresh] [--check <name>] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for the metadata rather than using the five-minute cache. |
| `--check <NAME>` | string | unset | Opens a matching check run instead of the pull request. Exact name wins; otherwise a unique case-insensitive substring is accepted. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints `{"message": "Opened <url>"}` instead of the sentence, or `{"message": "<url>"}` inside an SSH session. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

This is the one verb in the group with an effect outside the process, and it is
deliberately thin. It performs the same lookup every other `pr` verb performs,
takes the `url` field out of the result, and spawns the platform's opener with
that one argument. It spawns nothing when any of `SSH_CONNECTION`, `SSH_CLIENT`
or `SSH_TTY` is set: the browser then belongs to the machine at the other end of
the connection, so the verb writes the bare URL to stdout and exits 0. Most
terminals turn that URL into a clickable link, and the rest leave a line you can
copy.

| Platform | Opener |
| --- | --- |
| macOS | `open <url>` |
| Windows | `explorer <url>` |
| everything else | `xdg-open <url>` |

The choice is made at compile time from the target, not at runtime from what is
installed, so there is no fallback chain: a Linux machine without `xdg-open`
fails rather than trying `gio open` or a browser directly.

The child is spawned with stdin, stdout and stderr all attached to the null
device and is never waited on. That has two consequences worth knowing. Nothing
the browser or the opener writes can reach your terminal or your pipe, so the
sentence on stdout stays the only thing there. And the exit code only reports
whether the opener could be started: a browser that launches and then fails to
load the page, or an `xdg-open` that exits non-zero a moment later, is invisible
here. After a URL has been selected, the only failure this verb can report is a
missing or unexecutable opener:

```console
$ quinjet pr open 8
error: failed to hand https://github.com/pulkitxm/quinjet/pull/8 to xdg-open: No such file or directory (os error 2)
```

That exits 1. Everything else belongs to pull-request lookup or, with
`--check`, check selection. The [group page](./README.md) covers those exit
codes.

Without `--check`, the URL comes from GitHub rather than being constructed from
the number, so it is always canonical. With `--check`, Quinjet reads the check
list and applies the same exact-then-unique-substring selection as `pr logs`.
It opens the selected check's `link`. No match or an ambiguous match exits 3
with valid names in the hint; a selected check with no browser URL exits 4.

`--json` shape, one object with a single key. This is the standard shape for a
verb that acts rather than reads, described in
[the conventions](../conventions.md):

```json
{
  "message": "Opened https://github.com/pulkitxm/quinjet/pull/8"
}
```

The message is written after the spawn succeeds, so seeing it means the opener
started, not that a page rendered.

Examples:

```bash
quinjet pr open 8
quinjet pr open 8 --repo pulkitxm/quinjet
quinjet pr open 8 --check "Minimum supported Rust"
quinjet pr open 8 --json
quinjet pr open 8 -C ~/code/quinjet
```

```console
$ quinjet pr open 8
Opened https://github.com/pulkitxm/quinjet/pull/8
```

Over SSH the same call prints the URL and starts nothing:

```console
$ quinjet pr open 8
https://github.com/pulkitxm/quinjet/pull/8
```

On a headless machine or inside a container there may be no opener at all and no
SSH variables either, so prefer reading the URL and doing what you like with it:

```bash
quinjet pr view 8 --json | jq -r .url
```

## Where to go next

- [`quinjet pr view`](./view.md) for the metadata this verb takes the URL from
- [`quinjet pr checks`](./checks.md) for the list `--check` selects from
- [`quinjet pr`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
