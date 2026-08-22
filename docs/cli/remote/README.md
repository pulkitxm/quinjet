# Remote repositories

Quinjet can run its complete terminal or command-line interface on an SSH
machine. The remote machine needs `quinjet` and `git` on `PATH`.

```bash
quinjet --remote tuf-wired --folder ~/src/project
quinjet --remote tuf-wired --folder ~/src/project status --watch
```

`--remote` accepts any target understood by `ssh`, including aliases from
`~/.ssh/config`. `--folder` is an alias of `--path` and selects the repository
on the remote machine. Every other argument is forwarded to the remote Quinjet
process.

Interactive sessions allocate an SSH terminal. Non-interactive verbs preserve
stdout, stderr, JSON, watch output, and the remote command's exit status. An SSH
transport failure exits with code 4.

Successful sessions are stored locally as recent SSH repositories. Use
[`quinjet remote list`](./list.md) to check which machines are currently
reachable and [`quinjet remote forget`](./forget.md) to remove an entry.

The terminal interface and `status --watch` execute on the remote machine, so
its filesystem watcher observes remote edits directly. The normal periodic
refresh remains the fallback when the watcher misses an event.
