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
