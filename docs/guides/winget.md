# Winget

Windows Package Manager installs Quinjet from Microsoft's community source:

```powershell
winget install Pulkitxm.Quinjet
```

The package installs both command names:

```text
quinjet
q
```

Git is a package dependency and is installed when missing. The Pull Requests
view additionally needs an authenticated GitHub CLI installation, which remains
optional:

```powershell
winget install GitHub.cli
gh auth login
```

The current package is x86-64. Windows on ARM can run it through x64 emulation.
The first Quinjet invocation installs or refreshes PowerShell completion in the
same way as a PowerShell-script installation.

## Update

```powershell
winget upgrade Pulkitxm.Quinjet
```

`quinjet update` refuses to replace a Winget-owned executable because doing so
would leave Winget's installed-version record behind. It prints the Winget
upgrade command instead. `quinjet update --check` remains available.

## Inspect

```powershell
winget show Pulkitxm.Quinjet
winget list --id Pulkitxm.Quinjet
```

## Remove

```powershell
winget uninstall Pulkitxm.Quinjet
```

## Releasing

The release workflow packages the Windows binary twice inside
`quinjet-windows-x86_64.zip`, once as `quinjet.exe` and once as `q.exe`. The
multi-file manifest maps those files to the two portable command aliases,
declares Git as a dependency, and pins the archive's SHA-256 checksum. Rendered
manifests are published in `quinjet-winget-manifests.zip` with every GitHub
release so the Microsoft submission matches the released bytes exactly.

`packaging/winget/templates` contains the authored manifest set.
`scripts/winget_manifest.py` fills in the release version, date, and checksum,
and rejects missing or malformed release values before publishing.

The **Publish to WinGet** workflow submits each published stable release to
`microsoft/winget-pkgs`. It verifies the manifest bundle and Windows archive
against that release's `SHA256SUMS`, checks their version and installer metadata,
and opens one pull request for that version. Repeated runs reuse an open
submission or stop when the version is already merged. Microsoft validates and
merges the submission before it appears in the WinGet catalog.

### Publishing credential

Set the `WINGET_TOKEN` secret in the `pukbot-production` GitHub environment.
Use a GitHub user token with `public_repo` and `workflow` scopes. It needs access
to `pulkitxm/winget-pkgs` and permission to dispatch the Operation workflow in
`pulkitxm/pukbot`. An installation token cannot open a pull request in
Microsoft's repository where the app is not installed.

Fork synchronization, branches, commits, and cross-fork pull requests all use
Pukbot. GitHub CLI provides authentication and read-only release inspection.
The workflow installs a pinned, checksum-verified Pukbot release with fork
synchronization and owner-qualified pull request support.

### Retry a submission

Run **Publish to WinGet** from the Actions tab with a stable release tag, or leave
the tag empty to submit the latest release. The same operation is available
locally with an authenticated GitHub CLI and Pukbot v0.3.32 or later:

```bash
bash scripts/submit_winget.sh v0.0.70 --dry-run
bash scripts/submit_winget.sh v0.0.70
```

The dry run verifies the release and prints the planned changes. It does not
write to GitHub.
