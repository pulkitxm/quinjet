# Ubuntu PPA

Launchpad builds Quinjet for Ubuntu 22.04, 24.04, 26.04, 26.10, and the
development release, on x86-64 and ARM64:

```bash
sudo add-apt-repository ppa:pulkitxm/quinjet
sudo apt install quinjet
```

`add-apt-repository` adds the PPA and its signing key in one step. The package
declares Git as a dependency and `gh` as a suggestion, and installs the same
files as the [apt repository](apt.md):

```text
/usr/bin/quinjet
/usr/bin/q
/usr/share/bash-completion/completions/quinjet
/usr/share/fish/vendor_completions.d/quinjet.fish
/usr/share/zsh/vendor-completions/_quinjet
/usr/share/man/man1/quinjet.1.gz
```

## Update

```bash
sudo apt update
sudo apt install --only-upgrade quinjet
```

`quinjet update` refuses to replace an apt-owned executable and prints that
command instead. New releases appear once Launchpad finishes building them
after the GitHub release.

## Remove

```bash
sudo apt remove quinjet
sudo add-apt-repository --remove ppa:pulkitxm/quinjet
```

## Releasing

Launchpad builds every package from source without network access. The
release workflow vendors the Linux crates into one orig tarball and renders
`debian/` for each series with `scripts/ppa_source.py`, which builds against
the newest versioned Rust toolchain that series ships. CI builds the 24.04
package from that source offline, installs it, and smoke tests it. On release,
the sources are signed with the key in the `LAUNCHPAD_SIGNING_KEY` secret of
the `launchpad` environment and uploaded with `dput`.
