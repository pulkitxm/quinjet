# `quinjet update`

Checks GitHub's latest stable Quinjet release and, when it is newer, replaces
the executable that is currently running.

Usage:

```bash
quinjet update [--check] [--json]
```

Options:

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `--check` | flag | off | Reports whether a newer stable release exists without downloading or replacing a binary. |
| `--json` | flag | off | Prints one result object instead of a sentence. Global. |
| `-C, --path <DIR>` | path | `.` | Accepted as a global option but unused because updating is not a repository operation. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

A Homebrew installation is the one case `update` refuses to touch. When the
resolved executable sits inside a Homebrew Cellar, `update` prints that Homebrew
owns the executable, hints `brew upgrade quinjet`, and exits 4 without any
network request. Replacing a file inside the Cellar would leave Homebrew's
records describing a version that is no longer installed. `update --check` is
unaffected and still reports whether a newer release exists. See
[the Homebrew guide](../guides/homebrew.md).

Beyond that, `update` does not try to infer whether the binary came from `cargo install`,
cargo-binstall, `install.sh`, `install.ps1`, or a copied release artifact. Those
methods can all use custom directories, and the existing installers do not
persist provenance. The reliable identity is `std::env::current_exe()`,
canonicalized so a `q` launcher symlink is not the destination. `q update` and
`quinjet update` therefore replace the same real executable and leave the
launcher in place. A custom Cargo root or installer directory needs no special
flag.

The update sequence is fixed:

1. Read the latest published release from GitHub and parse its `vX.Y.Z` tag as a stable semantic version.
2. Stop successfully when that version is equal to or older than the compiled version.
3. Select the published asset for the running operating system and architecture.
4. Build checksum and binary URLs from the resolved tag, never from a moving `latest/download` URL.
5. Download the release's `SHA256SUMS`, require one exact valid entry for the selected asset, and download the binary with a 32 MiB ceiling.
6. Verify SHA-256 before creating any staged executable.
7. Resolve the running path through any `q` launcher symlink, stage beside the real destination, and replace that file while preserving its permissions. Unix replacement is a copy plus rename; Windows uses `self-replace`.
8. Start the newly installed executable in automatic completion-maintenance mode so an active, still-installed shell script receives the new command tree before success is reported.

Every network request has a 30 second timeout. Release metadata is limited to
1 MiB and the checksum document to 64 KiB. On Linux and macOS the updater uses
`curl`, falling back to `wget`; on Windows it uses PowerShell. The downloader is
given argument arrays rather than a shell-built command. No `gh` authentication
is used or required, so the request is subject to GitHub's normal unauthenticated
release API limits.

## Supported releases

| Runtime | Asset |
| --- | --- |
| Linux x86-64 | `quinjet-linux-x86_64` |
| Linux AArch64 | `quinjet-linux-aarch64` |
| macOS x86-64 | `quinjet-macos-x86_64` |
| macOS Apple Silicon, including an Intel build running under Rosetta | `quinjet-macos-aarch64` |
| Windows x86-64 or Windows ARM64 using x64 emulation | `quinjet-windows-x86_64.exe` |

An unsupported operating system or architecture fails before downloading the
checksum or binary. Linux GNU and musl installations both move to the published
static musl asset.

## Output

An installation that is current, including one newer than GitHub's latest
stable release:

```console
$ quinjet update
Quinjet 1.2.3 is up to date
```

A check with a newer release:

```console
$ quinjet update --check
Quinjet 1.3.0 is available (current 1.2.3)
```

A completed update:

```console
$ quinjet update
Updated Quinjet from 1.2.3 to 1.3.0
```

The shell-integration refresh is silent. On the first installation it writes
the generated script, any marked completion integration, and a `q` launcher on
`PATH`. On an update it rewrites only completion scripts that
still exist. The installed-once state described by
[`quinjet completions`](./generated/completions.md) prevents an update from
restoring anything the user removed.

The JSON form has one stable object. `asset` is `null` when no newer release
exists; `status` is `up_to_date`, `available`, or `updated`:

```json
{
  "status": "available",
  "currentVersion": "1.2.3",
  "latestVersion": "1.3.0",
  "asset": "quinjet-linux-x86_64"
}
```

## Failures and safety

Success exits 0. A network timeout or HTTP failure, invalid release metadata,
unsupported target, oversized response, missing or duplicate checksum,
checksum mismatch, unwritable installation directory, replacement failure, or
completion refresh failure prints an error on stderr and exits 1. A completion
failure happens after the verified binary replacement, so the error says that
the executable was updated and names the completion problem.

The existing executable is not touched until the version is newer, every
download has completed, and the checksum matches. Staging and replacement
failures clean up the temporary file and preserve the old executable. The
explicit `update` command is the confirmation, so there is no `--yes` flag.
The startup completion check also runs when the current version is already up
to date. It repairs a stale existing script without another download, but
leaves a removed script, marked completion block, or `q` launcher removed.

Cargo's installation tracker is not rewritten. The executable on disk is the
new release, but Cargo may continue to record the version it originally
installed until the next `cargo install --force` or cargo-binstall operation.

## Where to go next

- [Getting started](./getting-started.md) for every supported installation method
- [Conventions and contracts](./conventions.md) for JSON, stdout, stderr, and exit codes
- [All `quinjet` commands](./README.md)
