# Install Quinjet from GitHub Releases.
[CmdletBinding()]
param(
    [Parameter()]
    [string] $Version,

    [Parameter()]
    [string] $BinDir,

    [Parameter()]
    [switch] $NoModifyPath
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol =
    [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repository = "pulkitxm/quinjet"
$ReleasesUrl = "https://github.com/$Repository/releases"

function Write-Info {
    param([string] $Message)
    Write-Output "info: $Message"
}

function Get-BooleanEnvironmentValue {
    param(
        [string] $Name,
        [bool] $DefaultValue
    )

    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $DefaultValue
    }

    switch ($value.ToLowerInvariant()) {
        { $_ -in @("0", "false", "no") } { return $false }
        { $_ -in @("1", "true", "yes") } { return $true }
        default { throw "$Name must be 0 or 1" }
    }
}

function Test-PathEntry {
    param(
        [string] $PathValue,
        [string] $Entry
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    $normalizedEntry = $Entry.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    foreach ($candidate in $PathValue.Split([IO.Path]::PathSeparator)) {
        $normalizedCandidate = $candidate.Trim().TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
        if ($normalizedCandidate.Equals($normalizedEntry, [StringComparison]::OrdinalIgnoreCase)) {
            return $true
        }
    }
    return $false
}

function Invoke-Download {
    param(
        [string] $Uri,
        [string] $OutFile
    )

    try {
        Invoke-WebRequest -Uri $Uri -OutFile $OutFile -UseBasicParsing
    }
    catch {
        throw "failed to download ${Uri}: $($_.Exception.Message)"
    }
}

function Install-Quinjet {
    param(
        [string] $RequestedVersion,
        [string] $RequestedBinDir,
        [switch] $SkipPathUpdate
    )

    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw "install.ps1 only supports Windows; use install.sh on Linux or macOS"
    }

    if ([string]::IsNullOrWhiteSpace($RequestedVersion)) {
        $RequestedVersion = if ([string]::IsNullOrWhiteSpace($env:QUINJET_VERSION)) {
            "latest"
        }
        else {
            $env:QUINJET_VERSION
        }
    }

    if ([string]::IsNullOrWhiteSpace($RequestedBinDir)) {
        if (-not [string]::IsNullOrWhiteSpace($env:QUINJET_INSTALL_DIR)) {
            $RequestedBinDir = $env:QUINJET_INSTALL_DIR
        }
        elseif (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
            $RequestedBinDir = Join-Path $env:LOCALAPPDATA "Programs\Quinjet\bin"
        }
        elseif (-not [string]::IsNullOrWhiteSpace($HOME)) {
            $RequestedBinDir = Join-Path $HOME ".local\bin"
        }
        else {
            throw "could not determine an installation directory; set QUINJET_INSTALL_DIR"
        }
    }

    $skipPathUpdate = $SkipPathUpdate.IsPresent -or
        (Get-BooleanEnvironmentValue -Name "QUINJET_NO_MODIFY_PATH" -DefaultValue $false)

    $architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    switch ($architecture) {
        "X64" { $asset = "quinjet-windows-x86_64.exe" }
        "Arm64" {
            $asset = "quinjet-windows-x86_64.exe"
            Write-Warning "a native Windows ARM64 build is not available; installing the x64 build"
        }
        default { throw "Quinjet does not publish a Windows release for architecture '$architecture'" }
    }

    if ($RequestedVersion -eq "latest") {
        $releaseUrl = "$ReleasesUrl/latest/download"
        $versionLabel = "latest"
    }
    else {
        $releaseTag = if ($RequestedVersion.StartsWith("v")) { $RequestedVersion } else { "v$RequestedVersion" }
        if ($releaseTag -notmatch '^v[0-9][0-9A-Za-z._+-]*$') {
            throw "invalid release version: $RequestedVersion"
        }
        $releaseUrl = "$ReleasesUrl/download/$releaseTag"
        $versionLabel = $releaseTag
    }

    $tempDir = Join-Path ([IO.Path]::GetTempPath()) ("quinjet-install-" + [Guid]::NewGuid().ToString("N"))
    $downloadPath = Join-Path $tempDir $asset
    $checksumsPath = Join-Path $tempDir "SHA256SUMS"
    $stagedBinary = $null

    New-Item -ItemType Directory -Path $tempDir | Out-Null
    try {
        Write-Info "detected Windows $architecture"
        Write-Info "downloading Quinjet $versionLabel"
        Invoke-Download -Uri "$releaseUrl/SHA256SUMS" -OutFile $checksumsPath
        Invoke-Download -Uri "$releaseUrl/$asset" -OutFile $downloadPath

        $escapedAsset = [Regex]::Escape($asset)
        $checksumPattern = "^(?<hash>[0-9A-Fa-f]{64})\s+\*?(?:dist/)?${escapedAsset}$"
        $checksumMatch = $null
        foreach ($line in Get-Content -LiteralPath $checksumsPath) {
            $match = [Regex]::Match($line.Trim(), $checksumPattern)
            if ($match.Success) {
                $checksumMatch = $match
                break
            }
        }
        if ($null -eq $checksumMatch) {
            throw "the release checksum for $asset is missing or invalid"
        }

        $expectedChecksum = $checksumMatch.Groups["hash"].Value
        $actualChecksum = (Get-FileHash -LiteralPath $downloadPath -Algorithm SHA256).Hash
        if (-not $expectedChecksum.Equals($actualChecksum, [StringComparison]::OrdinalIgnoreCase)) {
            throw "checksum verification failed for $asset"
        }
        Write-Info "verified SHA-256 checksum"

        New-Item -ItemType Directory -Force -Path $RequestedBinDir | Out-Null
        $destination = Join-Path $RequestedBinDir "quinjet.exe"
        $stagedBinary = Join-Path $RequestedBinDir (".quinjet-install-" + [Guid]::NewGuid().ToString("N") + ".exe")
        Copy-Item -LiteralPath $downloadPath -Destination $stagedBinary
        Move-Item -LiteralPath $stagedBinary -Destination $destination -Force
        $stagedBinary = $null

        Write-Output "`nQuinjet was installed to $destination"

        $processPath = [Environment]::GetEnvironmentVariable("Path", "Process")
        if (-not (Test-PathEntry -PathValue $processPath -Entry $RequestedBinDir)) {
            if ($skipPathUpdate) {
                Write-Warning "$RequestedBinDir is not on PATH"
                Write-Output "Add it before using Quinjet:"
                Write-Output "  `$env:Path = `"$RequestedBinDir;`$env:Path`""
            }
            else {
                $pathUpdateFailed = $false
                $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
                if (-not (Test-PathEntry -PathValue $userPath -Entry $RequestedBinDir)) {
                    $updatedPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
                        $RequestedBinDir
                    }
                    else {
                        "$RequestedBinDir$([IO.Path]::PathSeparator)$userPath"
                    }
                    try {
                        [Environment]::SetEnvironmentVariable("Path", $updatedPath, "User")
                        Write-Info "added $RequestedBinDir to your user PATH"
                    }
                    catch {
                        $pathUpdateFailed = $true
                        Write-Warning "could not update your user PATH: $($_.Exception.Message)"
                    }
                }
                if ($pathUpdateFailed) {
                    Write-Output "Add it before using Quinjet:"
                    Write-Output "  `$env:Path = `"$RequestedBinDir;`$env:Path`""
                }
                else {
                    Write-Warning "restart your terminal before running quinjet"
                }
            }
        }

        if ($null -eq (Get-Command git -ErrorAction SilentlyContinue)) {
            Write-Warning "Git is required at runtime but was not found on PATH"
            Write-Warning "install Git from https://git-scm.com/download/win"
        }
    }
    finally {
        if ($null -ne $stagedBinary) {
            Remove-Item -LiteralPath $stagedBinary -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Install-Quinjet -RequestedVersion $Version -RequestedBinDir $BinDir -SkipPathUpdate:$NoModifyPath
