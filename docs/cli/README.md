# The `quinjet` command line

`quinjet` is one binary with two faces. Run it with no verb and it opens the
terminal interface: the Changes tab, the commit history, the pull-request pane
with its live checks and foldable Actions logs. Run it with a verb and it does
exactly one of those things and exits, on stdout, with no terminal to hold.

The two are not two implementations. Repository and GitHub data operations use
`cli::Command`, and the interface's Git worker executes those commands through
the same session a verb does. Browser opening uses one shared helper.
Presentation state such as focus, scrolling, folding, filtering, and mouse
capture remains specific to the terminal interface.

```bash
quinjet                          open the terminal interface here
quinjet tui ~/code/project       open it somewhere else
quinjet status                   what the Changes tab shows
quinjet diff --staged            what its diff pane shows for the index
quinjet pr checks 12 --watch     what the pull-request pane polls for
quinjet pr logs 12 clippy        one check run's steps and its log
quinjet completions bash         generate metadata without a repository
quinjet completions --install    install completions and the q shortcut
quinjet man --dir ./man1         generate all manual pages on demand
quinjet capabilities --json      inspect the installed command surface
quinjet update --check           check the latest stable release
```

## Start here

| Page | What it covers |
| --- | --- |
| [Getting started](./getting-started.md) | Installing, `-C`, the shape of a command, and the first five things worth running |
| [Conventions and contracts](./conventions.md) | The `--json` guarantee, stdout versus stderr, the exit-code table, what needs `git` and what needs `gh` |
| [The terminal interface](./tui.md) | The verb-less form, its flags, and which key on screen maps to which verb |

## Reading a repository

| Page | What it covers |
| --- | --- |
| [`quinjet status`, `diff`, `log`, `show`](./repository/README.md) | The working tree, the patch, the history, one commit |
| [`quinjet --remote`, `remote list`, `remote forget`](./remote/README.md) | Run on an SSH machine and inspect recent remote repositories |
| [`quinjet branch`](./branch/README.md) | Listing, switching, creating, renaming, deleting, and comparing without a checkout |
| [`quinjet stash`](./stash/README.md) | The whole stash workflow, including previewing one as a patch |
| [`quinjet worktree list`](./worktree/README.md) | Linked checkouts of the same repository |

## Changing a repository

| Page | What it covers |
| --- | --- |
| [`quinjet stage`, `unstage`, `discard`, `remove`, `commit`, `resolve`, `cherry-pick`, `revert`](./changes/README.md) | Top-level verbs that move the index, working tree, or `HEAD` |
| [`quinjet fetch`, `pull`, `push`, `sync`, `repos`](./remotes/README.md) | Talking to remotes, and which GitHub repositories this checkout points at |

## Pull requests

| Page | What it covers |
| --- | --- |
| [`quinjet pr`](./pull-request/README.md) | Metadata, files, conversation, checks, reviews, lifecycle, merge, metadata editing, and notifications |

## About Quinjet itself

| Page | What it covers |
| --- | --- |
| [`quinjet completions`, `man`](./generated/README.md) | Shell completion scripts and manual pages, generated from the command tree |
| [`quinjet capabilities`](./generated/capabilities.md) | Machine-readable discovery of commands, arguments, values, and output modes |
| [`quinjet update`](./update.md) | Checking releases and replacing the running executable after checksum verification |

## Guides

| Page | What it covers |
| --- | --- |
| [Installing with apt](../guides/apt.md) | Signed Debian and Ubuntu repository setup, upgrades, package contents, and removal |
| [Installing with Winget](../guides/winget.md) | Windows installation, upgrades, package inspection, and removal |
| [Installing with Homebrew](../guides/homebrew.md) | Tap installation on macOS and Linux, upgrades, package contents, and removal |
| [Watching CI from a script](../guides/watching-ci.md) | Blocking on checks, reading the verdict from an exit code, and pulling a failing job's log out with `jq` |
| [Automating Quinjet](../guides/automation.md) | Capability discovery, JSON contracts, non-interactive behavior, and completion setup |

## The short version

```bash
quinjet status                        branch, index, working tree
quinjet diff --staged                 the patch a commit would record
quinjet log -n 10                     the ten most recent commits
quinjet branch list --all             local and remote-tracking branches
quinjet stash list                    what is stashed
quinjet pr view 12                    a pull request's metadata
quinjet pr view 12 --watch            refresh the metadata until stopped
quinjet pr conversation 12 --watch    follow the conversation until stopped
quinjet pr checks 12 --watch          block until CI settles, exit non-zero if it did not go green
quinjet pr logs 12 clippy --watch     tail a running job's log
quinjet update --check                check whether a newer stable release exists
```

Every read takes `--json` and prints one document on stdout, except while
watching, where it prints one compact document per read. Errors, hints and
warnings go to stderr. Exit codes are part of the contract, so a script can
drive Quinjet without a terminal. See
[conventions and contracts](./conventions.md) for the details.
