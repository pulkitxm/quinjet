# Quinjet in Edith

Edith ships Quinjet as a native extension while Quinjet remains the Git, diff,
pull request, and live workspace review engine. Edith owns the surrounding app
experience: machines, projects, folders, worktrees, tabs, terminal selection,
themes, and managed-session lifecycle.

This page records the complete integration contract and the behavior that is
enabled only when Quinjet is launched by Edith.

## Ownership boundary

| Area | Owner | Contract |
| --- | --- | --- |
| Git status, history, diffs, pull requests, and reviews | Quinjet | The normal Quinjet terminal interface runs against the selected worktree. |
| Extension registration and installation | Edith | Quinjet appears in Extensions and can be installed from `pulkitxm/tap/quinjet`. |
| Project, folder, machine, worktree, and tab UI | Edith | Native SwiftUI replaces Quinjet's project picker for managed sessions. |
| Local repository metadata | Quinjet CLI | `quinjet project list --json` and `quinjet worktree list --json` provide structured data. |
| SSH execution | Quinjet and Edith | Quinjet owns remote command forwarding. Edith supplies its machine target and existing SSH control socket. |
| Embedded terminal | Edith | Edith starts Quinjet in its terminal view and receives Quinjet host events. |
| External terminal | Edith and cmux | Edith creates, focuses, replaces, and closes the cmux workspace used by a tab. |
| Theme and appearance | Edith | Edith stores the selection, passes Quinjet flags, and matches the embedded terminal palette. |

## What the user gets

### Native extension shell

- Quinjet is registered as an optional Edith extension with a dedicated main
  navigation page and window route.
- Edith checks whether the `quinjet` executable is installed and exposes the
  Homebrew installation path when it is missing.
- The first screen is Edith's project picker. The empty Quinjet project picker
  is not shown inside a managed terminal.
- Every open review has an Edith tab that retains its machine, project,
  worktree, terminal configuration, and terminal session.

### Local projects and folders

Edith loads local recent projects with:

```bash
quinjet project list --json
```

The response groups a repository by its common Git directory and includes all
linked worktrees. Edith shows project cards, supports search by project name,
branch, or path, and opens the current worktree by default. The worktree count
is a bordered button with a beige hover treatment. It opens a native popover so
one linked worktree can be selected explicitly.

The local picker also has a native macOS folder chooser. After a folder is
selected, Edith validates it and loads its worktrees with:

```bash
quinjet -C /path/to/project worktree list --json
```

Bare and prunable worktrees are not offered because they cannot be opened as a
live workspace.

### SSH machines and recent folders

The machine strip contains the host Mac and every machine known to Edith. Each
machine shows its connection state. Selecting an SSH machine starts or reuses
its Edith machine session.

Remote recent projects are assembled in two stages:

1. Edith reads `quinjet remote list --json` on the host.
2. It keeps accessible, absolute folders for the selected SSH target.
3. For each folder, it asks the remote Quinjet process for its worktrees.
4. Duplicate repositories are collapsed by their worktree paths.

The worktree request reuses Edith's active SSH connection:

```bash
quinjet \
  --remote user@machine \
  --ssh-control-path /path/to/edith.sock \
  -C /remote/project \
  worktree list \
  --json
```

This makes recent SSH folders project-selectable instead of displaying machine
names without a usable repository path.

### Live remote folder browser

Every SSH machine also has a Browse folders view. This browser is backed by
Edith's live `MachineSession`, not by a cached local filesystem snapshot.

- It resolves the remote home directory after the machine connects.
- Typing an absolute path refreshes matching subfolders after a short debounce.
- Directories and symlinks are navigable, and directories sort before files.
- `Tab` completes the selected or first matching directory.
- Up and down move through the current directory and its entries.
- `Return` enters a directory or opens the current directory as a project.
- `Command+Z` returns to the previous directory.
- Parent and refresh buttons support the same navigation without the keyboard.

Opening a directory still goes through Quinjet's remote worktree command, so
the selected path becomes the exact repository model and worktree that Edith
launches.

### Edith tabs and worktrees

- The plus button creates a new Edith project-picker tab.
- Selecting another worktree from the worktree button replaces the project in
  the current Edith tab.
- Selecting a project in a new picker tab opens it in that tab.
- A new tab requested from a remote Quinjet session keeps the same machine
  selected.
- Changing worktrees preserves the Edith tab identity while restarting Quinjet
  against the chosen worktree.
- The final Edith tab cannot be closed, preventing an empty extension page.
- Closing a non-final tab stops its embedded terminal and closes its managed
  cmux workspace when one exists.

## Terminal selection

Edith offers two renderers in the Quinjet toolbar.

### Embedded

The embedded renderer starts Quinjet inside Edith's terminal view. A local
launch has this effective shape:

```bash
quinjet \
  --client edith \
  -C /path/to/worktree \
  tui \
  --theme gruvbox \
  --appearance dark
```

An SSH launch adds the selected machine and Edith control socket:

```bash
quinjet \
  --client edith \
  --remote user@machine \
  --ssh-control-path /path/to/edith.sock \
  -C /remote/worktree \
  tui \
  --theme gruvbox \
  --appearance dark
```

Edith supplies `TERM=xterm-256color` and `COLORTERM=truecolor`. Before each
launch it creates a fresh terminal view, clears prior mouse-tracking state, and
registers the Quinjet host-event channel. If the process unexpectedly exits,
the project remains selected and Edith shows a restart action instead of an
empty terminal.

### cmux

The cmux option is enabled only when Edith finds a supported cmux executable in
the application bundle. Edith opens Quinjet in a new cmux tab, records the cmux
workspace identifier, and can later focus, replace, or close that workspace.

The Edith tab remains visible with the selected project, path, theme, a Show in
cmux action, and a Use embedded terminal action. Reopening the project replaces
the prior cmux workspace rather than leaving unmanaged duplicate sessions.

cmux launches intentionally omit `--client edith`. The Quinjet process is no
longer inside Edith's terminal, so there is no Edith host-event receiver for
managed shortcut delegation. Local and SSH repository selection, theme flags,
and appearance flags are still passed normally.

## Themes and persistence

The toolbar exposes all thirteen Quinjet themes:

`quinjet`, `catppuccin`, `dracula`, `everforest`, `gruvbox`, `nord`, `one`,
`rose-pine`, `solarized`, `tokyo-night`, `ayu`, `monokai`, and `github`.

Edith stores the terminal renderer and theme in shared app defaults. Appearance
follows Edith's current light or dark color scheme. Changing the renderer,
theme, or appearance relaunches the selected project with the new configuration
while retaining its machine, worktree list, and Edith tab.

For embedded sessions, Edith also changes the terminal background, foreground,
and caret palette to match the selected Quinjet theme. For cmux sessions, the
same theme and appearance are passed to Quinjet and reflected in the Edith
placeholder.

## Managed shortcut protocol

`--client edith` activates an opt-in host protocol over OSC code `6973`.
Quinjet writes the sequence to its terminal, and Edith's embedded terminal
intercepts the payload instead of displaying it.

| Quinjet input | OSC payload | Edith action |
| --- | --- | --- |
| `w` | `quinjet;open-worktree` | Refresh and open the native worktree popover for the current Edith tab. |
| `N` (`Shift+N`) | `quinjet;open-new-tab` | Create and select a new Edith project-picker tab on the current machine. |

In a standalone Quinjet session, these keys keep their native behavior. `w`
opens Quinjet's current-tab project picker, and `N` opens its new-tab project
picker.

## Managed-session exceptions

The following exceptions exist only when `--client edith` is present:

- Quinjet does not open its own project or worktree modal for `w` or `N`.
- `q` does not quit the managed Quinjet process.
- `Ctrl+W` does not close a Quinjet repository tab.
- Quit is removed from the command palette and help list.
- Repository-tab close icons and the right-click close menu are hidden.
- Close-all and final-tab close requests are rejected by the workspace and the
  terminal loop.

These safeguards keep Edith as the lifecycle owner and prevent a quit or close
shortcut from leaving a blank terminal behind.

## SSH transport contract

- `--remote <SSH_TARGET>` accepts any target understood by `ssh`.
- `--ssh-control-path <PATH>` adds `ssh -S <PATH>` on the host and is removed
  before arguments are forwarded to the remote process.
- `--client edith` is forwarded to the remote Quinjet process, so embedded
  remote sessions keep the same managed behavior as local sessions.
- Interactive remote sessions allocate a terminal. Non-interactive JSON
  commands preserve stdout, stderr, exit status, and machine reachability
  errors.
- Successful remote commands record the target and folder in Quinjet's host
  recent state, which feeds Edith's recent SSH project list.
- Git and filesystem watching run on the selected machine, so live remote
  changes are observed where they happen.

The remote machine must have both `quinjet` and `git` on `PATH`.

## Why standalone Quinjet is unchanged

The integration is additive and opt-in:

- `--client` is unset by default. The only supported client value is `edith`.
- Native Quinjet project pickers, project tabs, quit commands, and close actions
  remain available without the flag.
- `--ssh-control-path` is optional. Ordinary SSH launches create their normal
  SSH connection when it is absent.
- `project list --json`, `remote list --json`, and `worktree list --json` are
  general CLI contracts. They do not require Edith and do not mutate a
  repository.
- Theme and appearance flags are existing Quinjet terminal options and continue
  to work for direct launches.
- cmux sessions launched by Edith deliberately run as ordinary Quinjet sessions
  because they are outside the managed embedded terminal.

## Failure behavior

| Failure | Result |
| --- | --- |
| Quinjet is not installed | Edith points to the Extensions installation flow. |
| A recent repository is missing or invalid | Quinjet omits it from structured project output. |
| An SSH machine cannot connect | Edith keeps the picker open and shows the machine error. |
| A remote recent folder is inaccessible | Edith excludes it and still offers live folder browsing. |
| A selected folder is not an open worktree | Edith shows the validation failure without replacing the active project. |
| Quinjet exits in the embedded terminal | Edith retains the tab and offers Restart. |
| A cmux workspace disappears | Show in cmux reports the focus failure and the project stays selected. |
| A stored terminal renderer is unavailable | Edith falls back to the embedded renderer. |

## Current scope

- Edith supports the embedded terminal and cmux renderer.
- Managed OSC shortcuts require the embedded renderer.
- SSH recent-folder discovery accepts absolute Unix paths.
- Theme changes restart the selected Quinjet process with new launch flags.
- Edith owns project and machine navigation. Quinjet continues to own all Git
  and GitHub review functionality after a worktree is open.

## Merged implementation pull requests

Quinjet:

- [#79: run Quinjet over SSH](https://github.com/pulkitxm/quinjet/pull/79)
- [#80: add Edith managed client integration](https://github.com/pulkitxm/quinjet/pull/80)
- [#81: show live repository activity](https://github.com/pulkitxm/quinjet/pull/81)
- [#82: unify modal keyboard and mouse interactions](https://github.com/pulkitxm/quinjet/pull/82)
- [#83: integrate Edith machine sessions](https://github.com/pulkitxm/quinjet/pull/83)
- [#84: unify project tabs across machines](https://github.com/pulkitxm/quinjet/pull/84)

Edith:

- [#345: embed Quinjet as a native extension](https://github.com/pulkitxm/edith/pull/345)
- [#346: add Quinjet machine projects](https://github.com/pulkitxm/edith/pull/346)
