# Edith integration

Edith embeds Quinjet for local and SSH repositories while owning project,
worktree, tab, terminal, and theme selection.

## Added

- `quinjet project list --json` exposes recent projects and worktrees to Edith.
- `--remote` runs Quinjet on a selected SSH machine, and
  `--ssh-control-path` reuses Edith's SSH session.
- `--client edith` sends worktree and new-tab shortcuts to Edith over the host
  terminal channel.

## Edith-only behavior

With `--client edith`, Quinjet delegates `W` and `Shift+N`, hides close and quit
actions, and keeps the managed terminal session alive. The flag is opt-in, so
standalone Quinjet and ordinary SSH launches are unchanged.
