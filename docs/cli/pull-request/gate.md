# `quinjet pr gate`

`quinjet pr gate <number>` gives one deterministic merge verdict and explains
every blocker Quinjet can establish from pull-request metadata, checks, and
review threads. The text form starts with `pass`, `pending`, or `blocked`, then
groups each reason under a stable category such as `ci`, `review`, `approval`,
`branch`, `conflict`, `queue`, or `ruleset`.

The command exits 0 when the gate passes, 1 when it is blocked, and 2 while its
answer is pending. `--watch` refreshes GitHub data until the verdict passes or
becomes blocked. `--interval <seconds>` changes the refresh period and has a
minimum of two seconds.

`--json` emits the verdict, pull-request number, immutable head OID, blocker and
pending arrays, cache provenance, and review truncation state. Scripts should
use the structured `category` and `summary` fields rather than parse text.

```console
$ quinjet pr gate 42
blocked
  ci: windows / test is failure
  review: 2 unresolved review threads
  branch: head is behind the base branch
```

```console
$ quinjet pr gate 42 --watch --interval 10 --json
```

The aggregate `BLOCKED` merge state is reported as a ruleset blocker. GitHub
does not expose every underlying repository rule through the existing
pull-request query, so this command does not invent a more specific reason.
