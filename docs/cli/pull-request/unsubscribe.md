# `quinjet pr unsubscribe`

Unsubscribes the current viewer from pull request notifications.

```bash
quinjet pr unsubscribe <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form calls GitHub's `updateSubscription` GraphQL mutation with
state `UNSUBSCRIBED`. GitHub preserves repository-level notification settings.
