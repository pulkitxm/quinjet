# `quinjet pr lock`

Locks a pull request conversation.

```bash
quinjet pr lock <number> [--reason <off-topic|resolved|spam|too-heated>] [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form runs `gh pr lock`. A reason is optional. GitHub enforces the
repository permission required to lock conversations.
