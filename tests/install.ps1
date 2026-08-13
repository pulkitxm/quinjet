$ErrorActionPreference = "Stop"
Set-StrictMode -Version 3.0

$Root = Split-Path -Parent $PSScriptRoot
$Installer = Join-Path $Root "install.ps1"
$TestRoot = Join-Path ([IO.Path]::GetTempPath()) ("quinjet-installer-tests-" + [Guid]::NewGuid().ToString("N"))
$Fixtures = Join-Path $TestRoot "fixtures"
$DownloadsLog = Join-Path $TestRoot "downloads.log"

function Assert-Equal {
    param(
        [object] $Expected,
        [object] $Actual,
        [string] $Message
    )
    if ($Expected -ne $Actual) {
        throw "${Message}: expected '$Expected', got '$Actual'"
    }
}

function Assert-Contains {
    param(
        [string] $Needle,
        [string] $Path
    )
    if (-not (Select-String -LiteralPath $Path -SimpleMatch $Needle -Quiet)) {
        throw "expected '$Needle' in $Path"
    }
}

function Set-ReleaseFixture {
    param(
        [string] $Contents,
        [switch] $InvalidChecksum
    )

    $asset = "quinjet-windows-x86_64.exe"
    $assetPath = Join-Path $Fixtures $asset
    [IO.File]::WriteAllText($assetPath, $Contents)
    $hash = if ($InvalidChecksum) { "0" * 64 } else { (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash }
    [IO.File]::WriteAllText((Join-Path $Fixtures "SHA256SUMS"), "$hash  dist/$asset`n")
}

New-Item -ItemType Directory -Path $Fixtures | Out-Null
[IO.File]::WriteAllText($DownloadsLog, "")

function global:Invoke-WebRequest {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string] $Uri,

        [Parameter(Mandatory)]
        [string] $OutFile,

        [Parameter()]
        [switch] $UseBasicParsing
    )

    Add-Content -LiteralPath $global:QuinjetDownloadsLog -Value $Uri
    $asset = [IO.Path]::GetFileName(([Uri] $Uri).AbsolutePath)
    Copy-Item -LiteralPath (Join-Path $global:QuinjetFixtures $asset) -Destination $OutFile
}

$global:QuinjetFixtures = $Fixtures
$global:QuinjetDownloadsLog = $DownloadsLog
$originalInstallDir = $env:QUINJET_INSTALL_DIR
$originalVersion = $env:QUINJET_VERSION
$originalNoModifyPath = $env:QUINJET_NO_MODIFY_PATH

try {
    Write-Host "test: installs and verifies a pinned Windows release"
    $binDir = Join-Path $TestRoot "successful-install\bin"
    Set-ReleaseFixture -Contents "Windows binary"
    & $Installer -Version "1.2.3" -BinDir $binDir -NoModifyPath *> (Join-Path $TestRoot "successful-install.log")

    $installed = Join-Path $binDir "quinjet.exe"
    Assert-Equal -Expected "Windows binary" -Actual ([IO.File]::ReadAllText($installed)) -Message "installed binary contents"
    Assert-Contains -Needle "https://github.com/pulkitxm/quinjet/releases/download/v1.2.3/quinjet-windows-x86_64.exe" -Path $DownloadsLog
    Assert-Contains -Needle "verified SHA-256 checksum" -Path (Join-Path $TestRoot "successful-install.log")

    Write-Host "test: rejects a checksum mismatch without replacing an installation"
    $binDir = Join-Path $TestRoot "bad-checksum\bin"
    New-Item -ItemType Directory -Path $binDir | Out-Null
    $installed = Join-Path $binDir "quinjet.exe"
    [IO.File]::WriteAllText($installed, "existing binary")
    Set-ReleaseFixture -Contents "tampered binary" -InvalidChecksum

    $failedAsExpected = $false
    try {
        & $Installer -Version "latest" -BinDir $binDir -NoModifyPath *> (Join-Path $TestRoot "bad-checksum.log")
    }
    catch {
        if ($_.Exception.Message -like "*checksum verification failed*") {
            $failedAsExpected = $true
        }
        else {
            throw
        }
    }
    if (-not $failedAsExpected) {
        throw "checksum mismatch unexpectedly succeeded"
    }
    Assert-Equal -Expected "existing binary" -Actual ([IO.File]::ReadAllText($installed)) -Message "existing installation"
    Assert-Contains -Needle "https://github.com/pulkitxm/quinjet/releases/latest/download/quinjet-windows-x86_64.exe" -Path $DownloadsLog

    Write-Host "test: rejects unsafe version values before downloading"
    $downloadCount = (Get-Content -LiteralPath $DownloadsLog).Count
    $failedAsExpected = $false
    try {
        & $Installer -Version "v1/../../invalid" -BinDir (Join-Path $TestRoot "invalid-version") -NoModifyPath
    }
    catch {
        if ($_.Exception.Message -like "*invalid release version*") {
            $failedAsExpected = $true
        }
        else {
            throw
        }
    }
    if (-not $failedAsExpected) {
        throw "invalid release version unexpectedly succeeded"
    }
    Assert-Equal -Expected $downloadCount -Actual (Get-Content -LiteralPath $DownloadsLog).Count -Message "download count"

    Write-Host "All PowerShell installer tests passed."
}
finally {
    $env:QUINJET_INSTALL_DIR = $originalInstallDir
    $env:QUINJET_VERSION = $originalVersion
    $env:QUINJET_NO_MODIFY_PATH = $originalNoModifyPath
    Remove-Item Function:\Invoke-WebRequest -Force -ErrorAction SilentlyContinue
    Remove-Variable QuinjetFixtures -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable QuinjetDownloadsLog -Scope Global -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $TestRoot -Recurse -Force -ErrorAction SilentlyContinue
}
