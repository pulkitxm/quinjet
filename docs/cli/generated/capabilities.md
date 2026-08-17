# `quinjet capabilities`

Describes the command tree carried by the installed Quinjet executable.

Usage:

```bash
quinjet capabilities [--json]
```

The human form lists every command path and its one-line purpose. The JSON form
adds each command's complete usage, arguments, short and long flags, actions,
value arity, defaults, accepted enum values, argument groups, and direct subcommands. It is
generated from the same built clap tree used for parsing, help, completions, and
manual pages. The usage string carries required positional and option groups in
clap's own notation. Detailed semantic constraints remain in each command's
help and reference page.

```console
$ quinjet capabilities
Quinjet 0.0.6 command capabilities (schema 1)

quinjet  A fast, live, keyboard-first Git source-control interface for the terminal
quinjet tui  Open the terminal interface
quinjet status  Show the working tree, the index and the branch
...

Use --json for arguments, values, and command relationships.
```

This command does not discover a repository, run `git` or `gh`, or read the
network. Like every invocation of an installed release binary, startup may
perform first-time shell integration or refresh an existing completion script
when its version marker is stale. Removed completion or `q` integration is
remembered and stays removed. The global `-C` option is accepted but ignored.

## JSON contract

`schemaVersion` versions this discovery document independently of the binary.
Consumers should request `--json`, require a schema version they understand,
and ignore fields they do not use.

```json
{
  "schemaVersion": 1,
  "version": "0.0.6",
  "outputModes": ["text", "json"],
  "commands": [
    {
      "path": "quinjet branch switch",
      "about": "Switch to a branch",
      "usage": "Usage: quinjet branch switch [OPTIONS] <BRANCH>",
      "arguments": [
        {
          "id": "name",
          "help": "Branch to switch to",
          "short": null,
          "long": null,
          "positional": true,
          "required": true,
          "action": "set",
          "minValues": 1,
          "maxValues": 1,
          "valueNames": ["BRANCH"],
          "possibleValues": [],
          "defaultValues": []
        }
      ],
      "groups": [],
      "subcommands": []
    }
  ]
}
```

Global arguments appear on the commands that inherit them. Presence-only flags
use `set_true` or `set_false` and accept zero values. User-entered identifiers
have no fixed `possibleValues`. Required one-of groups identify alternatives
such as paths or `--all`, and whether more than one member may be present. The synthetic `help` command and generated
`--help` and `--version` flags are omitted because they are parser facilities
rather than Quinjet operations.

Examples:

```bash
quinjet capabilities
quinjet capabilities --json | jq -r '.commands[].path'
quinjet capabilities --json | jq '.commands[] | select(.path == "quinjet pr checks")'
```

## Where to go next

- [Automating Quinjet](../../guides/automation.md) for structured output and failure handling
- [`quinjet completions`](./completions.md) for interactive shell discovery
- [`quinjet man`](./man.md) for offline terminal documentation
- [Generated references](./README.md), the rest of this group
