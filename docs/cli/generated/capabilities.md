# `quinjet capabilities`

Describes the command tree carried by the installed Quinjet executable.

Usage:

```bash
quinjet capabilities [--json]
```

The human form lists every command path and its one-line purpose. The JSON form
adds each command's arguments, short and long flags, required state, value names,
accepted enum values, and direct subcommands. It is generated from the same
built clap tree used for parsing, help, completions, and manual pages.

```console
$ quinjet capabilities
Quinjet 0.0.6 command capabilities (schema 1)

quinjet  A fast, live, keyboard-first Git source-control interface for the terminal
quinjet tui  Open the terminal interface
quinjet status  Show the working tree, the index and the branch
...

Use --json for arguments, values, and command relationships.
```

This command is side-effect free. It does not discover a repository, run `git`
or `gh`, read the network, or inspect user data. The global `-C` option is
accepted but ignored.

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
      "arguments": [
        {
          "id": "name",
          "help": "Branch to switch to",
          "short": null,
          "long": null,
          "required": true,
          "valueNames": ["BRANCH"],
          "possibleValues": []
        }
      ],
      "subcommands": []
    }
  ]
}
```

Global arguments appear on the commands that inherit them. Boolean flags expose
clap's `true` and `false` parser values, while user-entered identifiers have no
fixed `possibleValues`. The synthetic `help` command and generated `--help` and
`--version` flags are omitted because they are parser facilities rather than
Quinjet operations.

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
