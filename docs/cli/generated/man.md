# `quinjet man`

Prints Quinjet's manual page in roff on stdout, or writes one page per command
into a directory.

Usage:

```bash
quinjet man [--dir <DIR>] [--json]
```

Arguments: none.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--dir <DIR>` | path | unset | Writes one page per command into this directory and prints what it wrote, instead of printing the top page. The directory is created if it does not exist. |
| `--json` | flag | off | Prints the page, or the list of written paths, as one JSON object. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Like [`completions`](./completions.md), the pages are rendered from the command
tree this build carries, so they describe exactly the verbs and flags it has.
No repository is read, and `-C` is ignored.

Without `--dir` the output is the top-level page, in roff, on stdout:

```console
$ quinjet man | head -5
.ie \n(.g .ds Aq \(aq
.el .ds Aq '
.TH QUINJET 1  "quinjet 0.0.6"
.SH NAME
quinjet \- A fast, live, keyboard-first Git source-control interface for the terminal
```

Piping that into a pager renders it the way `man` would:

```bash
quinjet man | man -l -
```

## Writing every page

`--dir` writes the whole tree, one file per command, named the way `man`
expects to find a subcommand's page. Nested verbs are joined with hyphens, so
`quinjet branch create` becomes `quinjet-branch-create.1`:

```console
$ quinjet man --dir /tmp/quinjet-man
Wrote 43 pages to /tmp/quinjet-man
  /tmp/quinjet-man/quinjet.1
  /tmp/quinjet-man/quinjet-tui.1
  /tmp/quinjet-man/quinjet-status.1
  ...
```

To install them for your own account, write into the `man1` directory of a
path on `MANPATH`:

```bash
quinjet man --dir ~/.local/share/man/man1
man quinjet
man quinjet-branch-create
```

`~/.local/share/man` is searched by default on most Linux distributions and on
macOS. A system-wide install is the same command against
`/usr/local/share/man/man1`, which needs root.

Every page is rewritten on each run, so upgrading Quinjet and repeating the
command is all that is needed to bring installed pages up to date. Files in
the directory that Quinjet did not write are left alone.

`--json` shape without `--dir`, the page as one string:

```json
{
  "page": ".ie \\n(.g .ds Aq \\(aq\n.el .ds Aq '\n.TH QUINJET 1..."
}
```

`--json` shape with `--dir`, the paths in the order they were written:

```json
{
  "pages": [
    "/tmp/quinjet-man/quinjet.1",
    "/tmp/quinjet-man/quinjet-tui.1",
    "/tmp/quinjet-man/quinjet-status.1"
  ]
}
```

Examples:

```bash
quinjet man
quinjet man | man -l -
quinjet man --dir ~/.local/share/man/man1
quinjet man --dir ./build/man --json
```

## Where to go next

- [`quinjet completions`](./completions.md) for shell completion scripts
- [Generated references](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
