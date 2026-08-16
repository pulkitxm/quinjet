# Automating Quinjet

Quinjet treats its command line as an API. A script or coding agent can discover
the installed surface, request structured results, distinguish usage mistakes
from missing data, and run every repository operation without opening the
terminal interface.

## Discover before invoking

Start with the installed binary rather than a cached command list:

```bash
quinjet capabilities --json > quinjet-capabilities.json
jq -r '.commands[].path' quinjet-capabilities.json
```

The document includes a schema version, the binary version, command paths,
arguments, help, required state, value names, enum values, and child commands.
It is generated from clap, so it cannot drift independently of parsing.

Human-readable discovery remains available through all common clap forms:

```bash
quinjet --help
quinjet help pr
quinjet help pr checks
quinjet pr checks --help
```

## Keep stdout parseable

Pass `--json` before or after a verb. A one-shot invocation emits exactly one
pretty JSON document on stdout and no progress output. A runtime failure emits
no partial document. Errors and remediation hints go to stderr.

```bash
if result=$(quinjet status --json 2>quinjet-error.txt); then
  jq '.branch.head, (.changes | length)' <<<"$result"
else
  code=$?
  printf 'quinjet exited %s\n' "$code" >&2
  sed -n '1,4p' quinjet-error.txt >&2
fi
```

`--watch --json` is the deliberate streaming exception. It emits compact JSON
Lines, one complete snapshot per refresh. A consumer should parse one line at a
time rather than wait for end of file.

```bash
quinjet pr checks 42 --watch --json | jq --unbuffered '.checks[] | [.name, .status]'
```

## Use exit codes before text

Exit status is the first branch in an automation flow:

| Code | Meaning | Typical response |
| --- | --- | --- |
| 0 | Success, including a safe preview that changed nothing | Parse stdout. |
| 1 | The operation or requested verdict failed | Read stderr or the completed watched result. |
| 2 | Invalid command syntax | Regenerate from `capabilities` or inspect `--help`. |
| 3 | A named branch, revision, stash, path, or check was not found | Use the suggested listing command. |
| 4 | A known check exists but its log is unavailable | Retry later or inspect `pr checks`. |

Do not classify a failure by matching English error text. Codes 3 and 4 are
stable categories, and code 2 always comes from clap before repository work.

## Destructive work is explicit

Commands that can discard history or content preview by default. The same argv
can be inspected first and confirmed second:

```bash
quinjet discard generated/
quinjet discard generated/ --yes

quinjet cherry-pick a1b2c3d
quinjet cherry-pick a1b2c3d --yes
```

Quinjet never treats redirected stdin, CI, or an agent caller as consent. It
does not prompt, open an editor, or wait for credentials. Required values must
be present in argv, and Git and GitHub subprocesses run non-interactively.

## Progress stays out of protocols

A person running a one-shot command with interactive stderr may see an animated
status line while Git, GitHub, or the updater is working. The indicator is
cleared before the result appears. It is disabled when stderr is redirected,
under `--json`, and during watches. TTY detection changes presentation only,
never the selected operation or returned data.

## Install completions for people

The same argument metadata drives shell completion for bash, zsh, fish, elvish,
and PowerShell. Paths are identified separately from revisions, branch names,
stash references, check names, and numeric values.

```bash
quinjet completions bash > ~/.local/share/bash-completion/completions/quinjet
quinjet completions zsh > ~/.zfunc/_quinjet
quinjet completions fish > ~/.config/fish/completions/quinjet.fish
```

`quinjet completion` is a visible alias for users familiar with tools that use
the singular spelling.

## Design references

The command behavior follows established guidance rather than a private agent
protocol:

- [Command Line Interface Guidelines](https://clig.dev/) for stdout and stderr separation, examples-first help, TTY-aware progress, and terminal documentation
- [clap derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/) for generated help, validation, and command-tree assertions
- [`clap_complete`](https://docs.rs/clap_complete/latest/clap_complete/) for completions generated from the parser definition
- [Cargo metadata compatibility](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html#compatibility) for explicit machine-readable format versions
- [Terraform machine-readable UI](https://developer.hashicorp.com/terraform/internals/machine-readable-ui) for separating one-shot documents from event streams

## Where to go next

- [Conventions and contracts](../cli/conventions.md) for the full JSON and exit-code guarantees
- [`quinjet capabilities`](../cli/generated/capabilities.md) for the discovery schema
- [Watching CI from a script](./watching-ci.md) for a complete blocking CI workflow
