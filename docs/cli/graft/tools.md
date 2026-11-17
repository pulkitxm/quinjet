# `quinjet graft tools`

Lists the tools that registered gateways expose.

```bash
quinjet graft tools
quinjet graft tools review --json
```

```text
review  http://localhost:7400/mcp
  summarize_diff(branch*, diff*)  Summarize a patch for a reviewer.
  triage(check_log*, pull_request*, focus?)  Explain a failing check and what to do next.
```

A `*` marks a parameter `graft call` fills from the repository, and the output
ends with the table of context parameters. A gateway that cannot be reached is
listed as unavailable with the reason, and the command exits with code 4.
