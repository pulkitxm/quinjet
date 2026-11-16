# `quinjet graft tools`

Lists the tools that registered gateways expose.

```bash
quinjet graft tools
quinjet graft tools agent --json
```

```text
agent  http://localhost:7390/mcp
  draft_commit_message(branch*, recent_commits*, staged_diff*)  Draft a commit message for the staged change.
  explain_check_failure(check_log*, pull_request*)  Explain why a pull request's continuous integration check failed.
```

A `*` marks a parameter `graft call` fills from the repository, and the output
ends with the table of context parameters. A gateway that cannot be reached is
listed as unavailable with the reason, and the command exits with code 4.
