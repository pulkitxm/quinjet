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

function Assert-InstallFailure {
    param(
        [string] $RequestedVersion,
        [string] $Directory,
        [string] $Pattern,
        [string] $Log = (Join-Path $TestRoot "failed-install.log")
    )
    try {
        & $Installer -Version $RequestedVersion -BinDir $Directory -NoModifyPath *> $Log
        throw "installation unexpectedly succeeded"
    }
    catch {
        if ($_.Exception.Message -notlike $Pattern) { throw }
    }
}

function New-ExecutableFixture {
    param([string] $OutputAssembly)

    $compiler = Join-Path $TestRoot "compile-fixture.ps1"
    $compilerSource = @'
param([Parameter(Mandatory)][string] $OutputAssembly)

$source = @"
using System;
using System.IO;

public static class QuinjetInstallerFixture
{
    public static void Main(string[] arguments)
    {
        File.WriteAllText(
            Environment.GetEnvironmentVariable("QUINJET_TEST_COMPLETION_LOG"),
            string.Join(" ", arguments));
    }
}
"@

Add-Type -TypeDefinition $source -OutputAssembly $OutputAssembly -OutputType ConsoleApplication
'@
    [IO.File]::WriteAllText($compiler, $compilerSource)
    $windowsPowerShell = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
    & $windowsPowerShell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $compiler -OutputAssembly $OutputAssembly
    if ($LASTEXITCODE -ne 0) {
        throw "failed to compile the installer fixture"
    }
}

function Set-ReleaseFixture {
    param(
        [string] $Contents,
        [switch] $Executable,
        [switch] $InvalidChecksum
    )

    $asset = "quinjet-windows-x86_64.exe"
    $assetPath = Join-Path $Fixtures $asset
    if ($Executable) {
        New-ExecutableFixture -OutputAssembly $assetPath
    }
    else {
        [IO.File]::WriteAllText($assetPath, $Contents)
    }
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

        [Parameter()]
        [string] $OutFile,

        [Parameter()]
        [switch] $UseBasicParsing,

        [Parameter()]
        [string] $Method = "Get",

        [Parameter()]
        [int] $TimeoutSec,

        [Parameter()]
        [int] $MaximumRedirection
    )

    Assert-Equal -Expected 30 -Actual $TimeoutSec -Message "request timeout"
    Assert-Equal -Expected "SilentlyContinue" -Actual $ProgressPreference -Message "download progress"
    Add-Content -LiteralPath $global:QuinjetDownloadsLog -Value $Uri
    if ($Method -eq "Head") {
        Assert-Equal -Expected 5 -Actual $MaximumRedirection -Message "redirect limit"
        return [pscustomobject]@{
            BaseResponse = [pscustomobject]@{
                ResponseUri = [Uri] $global:QuinjetLatestUrl
                RequestMessage = [pscustomobject]@{ RequestUri = [Uri] $global:QuinjetLatestUrl }
            }
        }
    }
    if ($Uri -like "*/latest/download/*") {
        throw "asset download was not pinned"
    }
    $asset = [IO.Path]::GetFileName(([Uri] $Uri).AbsolutePath)
    Copy-Item -LiteralPath (Join-Path $global:QuinjetFixtures $asset) -Destination $OutFile
}

$global:QuinjetFixtures = $Fixtures
$global:QuinjetDownloadsLog = $DownloadsLog
$global:QuinjetLatestUrl = "https://github.com/pulkitxm/quinjet/releases/tag/v1.2.3"
$originalInstallDir = $env:QUINJET_INSTALL_DIR
$originalVersion = $env:QUINJET_VERSION
$originalNoModifyPath = $env:QUINJET_NO_MODIFY_PATH
$originalCompletionLog = $env:QUINJET_TEST_COMPLETION_LOG

try {
    Write-Host "test: installs and verifies a pinned Windows release"
    $binDir = Join-Path $TestRoot "successful-install\bin"
    $env:QUINJET_TEST_COMPLETION_LOG = Join-Path $TestRoot "completions.log"
    Set-ReleaseFixture -Executable
    $expectedHash = (Get-FileHash -LiteralPath (Join-Path $Fixtures "quinjet-windows-x86_64.exe") -Algorithm SHA256).Hash
    & $Installer -Version "1.2.3" -BinDir $binDir -NoModifyPath *> (Join-Path $TestRoot "successful-install.log")

    $installed = Join-Path $binDir "quinjet.exe"
    $actualHash = (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash
    Assert-Equal -Expected $expectedHash -Actual $actualHash -Message "installed binary hash"
    Assert-Equal -Expected "completions powershell --install --automatic" -Actual ([IO.File]::ReadAllText($env:QUINJET_TEST_COMPLETION_LOG)) -Message "completion installation arguments"
    Assert-Contains -Needle "https://github.com/pulkitxm/quinjet/releases/download/v1.2.3/quinjet-windows-x86_64.exe" -Path $DownloadsLog
    Assert-Contains -Needle "verified SHA-256 checksum" -Path (Join-Path $TestRoot "successful-install.log")

    Write-Host "test: resolves latest once and pins both downloads"
    $downloadCount = (Get-Content -LiteralPath $DownloadsLog).Count
    & $Installer -Version "latest" -BinDir (Join-Path $TestRoot "latest-install\bin") -NoModifyPath *> (Join-Path $TestRoot "latest-install.log")
    Assert-Equal -Expected ($downloadCount + 3) -Actual (Get-Content -LiteralPath $DownloadsLog).Count -Message "pinned latest request count"

    Write-Host "test: rejects invalid latest redirects before fetching assets"
    foreach ($latestUrl in @("https://example.com/releases/tag/v1.2.3", "https://github.com/pulkitxm/quinjet/releases/tag/v1.2.3-beta.1")) {
        $global:QuinjetLatestUrl = $latestUrl
        $downloadCount = (Get-Content -LiteralPath $DownloadsLog).Count
        Assert-InstallFailure -RequestedVersion "latest" -Directory (Join-Path $TestRoot "invalid-latest") -Pattern "invalid latest release*"
        Assert-Equal -Expected ($downloadCount + 1) -Actual (Get-Content -LiteralPath $DownloadsLog).Count -Message "invalid redirect request count"
    }
    $global:QuinjetLatestUrl = "https://github.com/pulkitxm/quinjet/releases/tag/v1.2.3"

    Write-Host "test: rejects invalid checksum records before fetching the binary"
    foreach ($checksumCase in @("duplicate", "missing", "malformed")) {
        Set-ReleaseFixture -Contents "new binary"
        $checksums = Join-Path $Fixtures "SHA256SUMS"
        switch ($checksumCase) {
            "duplicate" { Add-Content -LiteralPath $checksums -Value ([IO.File]::ReadAllText($checksums)) }
            "missing" { [IO.File]::WriteAllText($checksums, (("0" * 64) + "  other.exe`n")) }
            "malformed" { [IO.File]::WriteAllText($checksums, "invalid  quinjet-windows-x86_64.exe`n") }
        }
        $downloadCount = (Get-Content -LiteralPath $DownloadsLog).Count
        Assert-InstallFailure -RequestedVersion "1.2.3" -Directory $binDir -Pattern "the release checksum*"
        Assert-Equal -Expected ($downloadCount + 1) -Actual (Get-Content -LiteralPath $DownloadsLog).Count -Message "invalid checksum request count"
        Assert-Equal -Expected $expectedHash -Actual (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -Message "existing binary preserved"
    }

    Write-Host "test: rejects a checksum mismatch without replacing an installation"
    $binDir = Join-Path $TestRoot "bad-checksum\bin"
    New-Item -ItemType Directory -Path $binDir | Out-Null
    $installed = Join-Path $binDir "quinjet.exe"
    [IO.File]::WriteAllText($installed, "existing binary")
    Set-ReleaseFixture -Contents "tampered binary" -InvalidChecksum

    Assert-InstallFailure -RequestedVersion "latest" -Directory $binDir -Pattern "*checksum verification failed*" -Log (Join-Path $TestRoot "bad-checksum.log")
    Assert-Equal -Expected "existing binary" -Actual ([IO.File]::ReadAllText($installed)) -Message "existing installation"
    Assert-Contains -Needle "https://github.com/pulkitxm/quinjet/releases/download/v1.2.3/quinjet-windows-x86_64.exe" -Path $DownloadsLog

    Write-Host "test: rejects unsafe version values before downloading"
    $downloadCount = (Get-Content -LiteralPath $DownloadsLog).Count
    Assert-InstallFailure -RequestedVersion "v1/../../invalid" -Directory (Join-Path $TestRoot "invalid-version") -Pattern "*invalid release version*"
    Assert-Equal -Expected $downloadCount -Actual (Get-Content -LiteralPath $DownloadsLog).Count -Message "download count"

    Write-Host "All PowerShell installer tests passed."
}
finally {
    $env:QUINJET_INSTALL_DIR = $originalInstallDir
    $env:QUINJET_VERSION = $originalVersion
    $env:QUINJET_NO_MODIFY_PATH = $originalNoModifyPath
    $env:QUINJET_TEST_COMPLETION_LOG = $originalCompletionLog
    Remove-Item Function:\Invoke-WebRequest -Force -ErrorAction SilentlyContinue
    Remove-Variable QuinjetFixtures -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable QuinjetDownloadsLog -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable QuinjetLatestUrl -Scope Global -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $TestRoot -Recurse -Force -ErrorAction SilentlyContinue
}
