# `quinjet pr edit`

Changes one pull-request metadata field or relationship.

```bash
quinjet pr edit <number> <field> [value] [--repo <owner/name>] [--refresh] [--yes]
```

`<field>` is one of `title`, `body`, `base`, `add-assignee`, `remove-assignee`,
`add-label`, `remove-label`, `add-project`, `remove-project`, `add-reviewer`,
`remove-reviewer`, `milestone`, or `remove-milestone`. Every field except
`remove-milestone` needs a value. Relationship values may be comma-separated.

The confirmed form maps directly to one `gh pr edit` flag. Project edits need
the GitHub CLI `project` authorization scope.
