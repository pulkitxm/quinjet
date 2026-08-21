# `quinjet pr unlock`

Unlocks a pull request conversation.

```bash
quinjet pr unlock <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form runs `gh pr unlock`. The terminal interface offers this only
when current metadata says the conversation is locked.
