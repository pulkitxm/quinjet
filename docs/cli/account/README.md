# `quinjet account`

Select a GitHub identity once for each repository. Company and personal
repositories retain independent commit names, emails and HTTPS credentials.

```bash
quinjet account list
quinjet account switch agayushh
quinjet -C ~/code/company-project account switch ayushflickly
quinjet account list --json
```

Log in to each account first with `gh auth login`. In the terminal interface,
press `Ctrl+G`, then `↑` / `↓` to choose a saved login from the account list and `Enter` to use it.
The list shows all saved GitHub.com accounts and marks the current repository
account. Use `Home` / `End` to jump to either end, or `Esc` to cancel.
The command palette (`Ctrl+P` or `:`) also provides
**Switch GitHub Account for This Repository…**.

The bottom-right status bar shows only the selected repository username.
It updates after switching accounts and when repository status
refreshes. An unconfigured repository shows **Account unset**.

Switching verifies the selected credential against GitHub before changing the
repository. It uses the account's public display name and GitHub's ID-based
noreply email, so a private company email cannot become the fallback identity.
The settings apply to this repository and its linked worktrees. Other
repositories and the global Git configuration keep their existing settings.

HTTPS Git operations use a repository credential helper that retrieves the
selected account's token from GitHub CLI on demand. Tokens are never saved in
Git configuration. Quinjet's GitHub API commands also use the pinned account,
even after another application changes the globally active GitHub CLI account.
The switch does not change the globally active GitHub CLI account.

SSH remotes continue to use your SSH keys and host aliases. Use HTTPS remotes
for this account selection to control push authentication. Switching also turns
off commit signing locally to avoid carrying over a company signing key;
configure a signing key belonging to the selected account before re-enabling it.

New commits use the selected identity. Quinjet refuses conflicting author or
committer overrides, and refuses to amend a commit authored by another identity.
Existing history is preserved. Cherry-pick, revert, rebase and external Git
commands follow Git's own author-preservation and configuration rules.

The command requires a local repository and GitHub CLI. A failed credential or
network check leaves the repository settings unchanged. A repository config
lock prevents a switch from overwriting a concurrent Git configuration edit.
