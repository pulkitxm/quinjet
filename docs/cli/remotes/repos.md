# `quinjet repos`

Lists the GitHub repositories this checkout's remotes resolve to, with the
remote names that reach each one.

Usage:

```bash
quinjet repos [--refresh] [-C <DIR>] [--json]
```

Arguments: none. `quinjet repos` takes no positional argument; use `-C` to point
it at another checkout.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--refresh` | flag | off | Reads the remotes again instead of answering from the cached repository identity. Skips the cache read, not the cache write. |
| `-C, --path <DIR>` | path | `.` | Repository to run against. Global, so it may appear before or after the verb. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the table. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What it does underneath

The discovery runs in five steps, and the first three cost nothing but local
Git calls.

**One, list the remotes.** `git -C <root> -c core.quotepath=false remote` gives
the configured remote names. At most 32 are inspected.

**Two, read every URL of every remote.** For each name, both directions are
read, fetch first and then push:

```bash
git -C <root> -c core.quotepath=false remote get-url --all <name>
git -C <root> -c core.quotepath=false remote get-url --push --all <name>
```

`--all` matters: a remote with several `url` or `pushurl` entries contributes
all of them, so one remote can end up pointing at two repositories. A remote
whose fetch URL cannot be read adds the warning
``Unable to read URL for remote `<name>` `` and is skipped; a push URL that
cannot be read is skipped silently, because a remote without a separate pushurl
is normal. At most 64 name-and-URL pairs are collected in total.

**Three, strip credentials and group.** Every URL is rewritten before it is used
for anything. For a URL with a scheme, everything up to and including the last
`@` in the authority is dropped, and any `?query` or `#fragment` is cut from both
the authority and the path. So a remote whose authority carries a `user:token@`
prefix arrives as plain `https://github.com/o/r.git`. An scp-style address such as
`git@github.com:o/r.git` is rewritten to `ssh://github.com/o/r.git`. URLs that
are equal after this rewriting are grouped, and their remote names are collected
together. No token ever reaches `gh`, the cache key, the terminal or the JSON.

**Four, derive or ask.** Each distinct URL becomes a repository identity, by one
of two routes:

- Locally, with no network at all, when the sanitized URL has an `http`, `https`
  or `ssh` scheme, its host is exactly `github.com`, and its path is exactly two
  non-empty components. The `.git` suffix is stripped and the scheme is
  canonicalised to `https`, so `git@github.com:pulkitxm/quinjet.git` yields
  `pulkitxm/quinjet` at `https://github.com/pulkitxm/quinjet`.
- Otherwise through `gh`, which validates the host rather than letting Quinjet
  guess at it. This is the path every GitHub Enterprise remote takes, and every
  URL with a third path component, a non-standard scheme, or a local filesystem
  path:

```bash
gh repo view <sanitized-url> --json nameWithOwner,url --template '{{.nameWithOwner}}{{"\t"}}{{.url}}{{"\n"}}'
```

`gh` runs with the repository as its working directory and with
`GH_PROMPT_DISABLED=1`, `GH_PAGER=cat`, `GH_NO_UPDATE_NOTIFIER=1` and
`NO_COLOR=1`. A URL `gh` refuses adds the warning
``remote `<name>` is not available through gh: <error>`` and the next URL is
tried; one bad remote does not fail the command.

**Five, fall back, merge and sort.** If no URL produced a repository, Quinjet
runs the same `gh repo view` with no URL, letting `gh` infer a repository from
the directory and `GH_REPO`. Such an entry has no remote names and is printed as
`inferred`. Identities are then merged by their URL, compared without case and
without a trailing slash, so an `https` remote and an `ssh` remote pointing at
the same repository become one row with two remote names. The list is sorted
with the repository that owns a remote called `origin` first, and the rest by
display name without case. That first row is the repository
[`quinjet pr`](../pull-request/README.md) uses when no `--repo` is given.

## Caps and their warnings

Every cap has its own sentence, so a shortened list never looks complete:

| Cap | Warning when it is reached |
| --- | --- |
| 32 remotes | `Only the first 32 Git remotes were inspected` |
| 64 fetch and push URL entries | `Only the first 64 configured fetch/push URL entries were inspected` |
| 32 distinct URLs | `Only the first 32 distinct fetch/push remote URLs were inspected` |
| 16 repositories | `Only the first 16 distinct GitHub repositories were loaded` |

"First" means first in sorted order, not the order in `.git/config`. Remotes are
walked in the order `git remote` prints them, which is alphabetical, and
distinct URLs are walked in lexicographic order of the sanitized URL. The
repository cap is checked after each URL is resolved, and it only adds its
warning when there was actually another URL left to look at.

These caps apply to `quinjet pr` too. Every `pr` verb runs this same discovery
to find the repository a number belongs to, so a checkout with more than 16
GitHub repositories can hide the one you meant. Pass `--repo owner/name` to
name it directly.

## Caching

A repository identity that came from `gh` is cached for one day, under the key
`repository` followed by the sanitized URL, or `repository`, `inferred`, the
worktree root and `$GH_REPO` for the inferred fallback. `--refresh` skips
reading that entry and asks `gh` again; the fresh answer is written back either
way.

`--refresh` does nothing for a `github.com` remote, because that identity is
derived locally and is never cached or asked about. In a checkout whose only
remote is an ordinary GitHub URL, `quinjet repos --refresh` performs exactly the
same work as `quinjet repos`: two Git calls and no network.

If `gh` fails and a cached entry exists, the cached answer is used and the
command still exits 0, with `Using stale cached GitHub identity for remote
<name>` in the warnings, or `Using a stale cached inferred GitHub repository`
for the fallback. The command only fails when there is nothing to fall back to.

## Warnings are on stdout here

`repos` prints its warnings as trailing `warning:` lines in the listing itself,
and repeats them in the `warnings` array under `--json`. This is the exception
to the rule in [conventions and contracts](../conventions.md): the `pr` verbs
print the same kind of warning on stderr. Redirecting `quinjet repos` to a file
therefore captures the warnings with the table.

`--json` shape, an object with two keys: `repositories` is an array in the
printed order, and `warnings` is an array of plain sentences that is empty when
nothing went wrong. `remotes` is the sorted, de-duplicated list of remote names
that reach the repository, and it is empty for an inferred entry. `url` is the
canonical repository URL with any trailing slash removed, and it carries the
host, which is what keeps an Enterprise repository distinct from a `github.com`
one with the same owner and name.

```json
{
  "repositories": [
    {
      "nameWithOwner": "pulkitxm/quinjet",
      "url": "https://github.com/pulkitxm/quinjet",
      "remotes": [
        "origin"
      ]
    }
  ],
  "warnings": []
}
```

Examples:

```bash
quinjet repos
quinjet repos --json
quinjet repos --refresh
quinjet repos -C ~/code/project --json
quinjet repos --json | jq -r '.repositories[0].nameWithOwner'
```

```console
$ quinjet repos
pulkitxm/quinjet                         remote origin https://github.com/pulkitxm/quinjet
```

The first column is the display name, padded to 40 characters, and it gains a
host prefix when the host is not `github.com`, so an Enterprise repository
prints as `github.example.com/team/service`. The second column is
`remote <names>` with the names comma-separated, or the single word `inferred`.
The third is the URL. A repository reached by two remotes prints one row:

```console
$ quinjet repos
pulkitxm/quinjet                         remote origin, upstream https://github.com/pulkitxm/quinjet
```

When nothing resolves at all, the command fails rather than printing an empty
table:

```console
$ quinjet repos
error: No Git remotes are configured; GitHub CLI could not infer a repository: gh repo view failed: no git remotes found
```

The first half of that sentence is `No configured fetch or push remote resolves
to GitHub` instead when remotes exist but none of them is a GitHub repository.

## Where to go next

- [`quinjet fetch`, `pull`, `push`, `sync`, `repos`](./README.md), the rest of
  this group
- [`quinjet pr`](../pull-request/README.md), which starts from this listing and
  takes `--repo owner/name` to override it
- [Conventions and contracts](../conventions.md) for the cache root, the
  `--refresh` rule and the shared size caps
- [All `quinjet` commands](../README.md)
