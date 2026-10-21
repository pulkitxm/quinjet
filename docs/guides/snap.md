# Snap

The Snap Store publishes Quinjet for x86-64 and ARM64. On Ubuntu, snapd is
already installed, so one command is enough:

```bash
sudo snap install quinjet --classic
```

Other distributions can install snapd once from their own package manager and
then use the same command.

Quinjet uses classic confinement because it drives your own Git installation.
It runs the host `git` and `gh` executables and lets Git run your hooks,
editor, credential helpers, and signing programs, so Git is not bundled.
Install it from your distribution when it is missing:

```bash
sudo apt install git
```

Snapd links bash completion for `quinjet` automatically. Add the `q` shortcut
used by the other installation methods with:

```bash
sudo snap alias quinjet q
```

## Update

Snapd refreshes installed snaps in the background several times a day. To
update immediately:

```bash
sudo snap refresh quinjet
```

`quinjet update` refuses to replace a snap-owned executable and prints that
command instead. `quinjet update --check` still checks the newest release
without changing the installation.

## Inspect

```bash
snap info quinjet
snap list quinjet
snap aliases quinjet
```

## Remove

```bash
sudo snap remove quinjet
```

## Releasing

The release workflow packs a snap for each architecture from the same static
binaries that the apt repository, Homebrew, and the install script use. Each
snap is installed and smoke tested on a native Ubuntu 24.04 runner, which
checks the completion link, Git access in a scratch repository, and the
snap-owned update path, then uploads it to the stable channel. The upload
credentials live only in the `SNAPCRAFT_STORE_CREDENTIALS` secret of the
`snap-store` environment.
