# `quinjet pr subscribe`

Subscribes the current viewer to pull request notifications.

```bash
quinjet pr subscribe <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form calls GitHub's `updateSubscription` GraphQL mutation with
state `SUBSCRIBED` and the exact pull request node identity.
