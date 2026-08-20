# Homebrew

Quinjet is distributed through a personal tap rather than homebrew/core, so one
command installs it on macOS and on Linux:

```bash
brew install pulkitxm/tap/quinjet
```

There is no separate tap step. `brew install` resolves the fully qualified
`user/repository/token` form and taps
[pulkitxm/homebrew-tap](https://github.com/pulkitxm/homebrew-tap) for you before
it installs. The tap exists as its own repository because Homebrew only accepts
the short `brew tap <user>/<name>` form for repositories named
`homebrew-<name>`, and because cloning a few kilobytes of formula beats cloning
this one.

The formula downloads the release binary built for your platform, verifies its
published SHA-256 checksum, and installs:

```text
quinjet                the executable
q                      the same executable under its short name
completions            bash, zsh and fish scripts Homebrew loads for you
man 1 quinjet          the generated manual page
```

Everything lands under `brew --prefix`, so no shell profile is edited and
nothing is written to `~/.local`. Git is declared as a dependency and installed
with Quinjet when it is missing.

Prebuilt binaries exist for Apple Silicon, Intel Macs, and Linux on x86-64 and
aarch64. Windows is served by the [PowerShell
installer](../../README.md#install-script), not by Homebrew.

## Update

A Homebrew installation is upgraded by Homebrew:

```bash
brew update
brew upgrade quinjet
```

`quinjet update` refuses to run when Homebrew owns the executable, because
replacing a file inside the Cellar would leave Homebrew's records describing a
version that is no longer installed. It prints the `brew upgrade` command
instead. `quinjet update --check` still reports whether a newer release exists.

For the same reason Quinjet skips its first-run bootstrap under Homebrew: the
formula already installed the completions and the `q` shortcut, so nothing is
written into your home directory.

## Inspect

```bash
brew info quinjet          version, checksum, and what the formula installs
brew list quinjet          the paths Homebrew put on disk
brew outdated quinjet      whether a newer release exists
```

## Uninstall

```bash
brew uninstall quinjet
```

That removes the executable, the `q` shortcut, the completions and the manual
page. Quinjet's own state, the recent-project list and the markers that record
what a non-Homebrew installation put on your `PATH`, lives outside the prefix
and stays behind:

```bash
rm -rf ~/.local/state/quinjet
```

Stop tracking the tap entirely with:

```bash
brew untap pulkitxm/tap
```

## Releasing

Nothing about the formula is hand-edited. `extras/homebrew/quinjet.rb` is the
authored source and carries one placeholder per release-specific value. The
`publish` job in `.github/workflows/release.yml` renders it with
`scripts/homebrew_formula.py`, reading the version it just cut and the checksums
of the binaries it just published, then commits the result to the tap
repository. That repository is generated output; never edit it by hand.

Pushing to the tap needs a `TAP_PUSH_TOKEN` secret on this repository: a
fine-grained personal access token scoped to `pulkitxm/homebrew-tap` with read
and write access to contents. Without it the release fails at the mirror step,
after the crate and the GitHub release are already published, and the mirror can
be re-run once the token exists.

`make homebrew` renders the template locally with placeholder checksums, which
is what CI runs to catch a placeholder that no longer matches the script.
