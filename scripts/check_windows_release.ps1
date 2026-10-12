param(
    [Parameter(Mandatory = $true)]
    [string]$Binary
)

$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$dumpbin = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe'
if ($LASTEXITCODE -ne 0 -or -not $dumpbin) {
    throw 'Could not locate dumpbin'
}
$dependencies = & ($dumpbin | Select-Object -First 1) /DEPENDENTS $Binary
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect Windows release dependencies'
}
$imports = $dependencies | Select-String -Pattern '^\s+\S+\.dll\s*$' | ForEach-Object { $_.Line.Trim() }
if (-not $imports) {
    throw 'No Windows DLL imports were found'
}
$imports | Write-Output
if ($imports -match '^(VCRUNTIME|MSVCP|MSVCR|CONCRT|api-ms-win-crt)') {
    throw 'The portable release must not depend on the Visual C++ runtime DLLs'
}
$directory = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $directory | Out-Null
try {
    foreach ($name in 'quinjet', 'q') {
        $executable = Join-Path $directory "$name.exe"
        Copy-Item $Binary $executable
        & $executable --version
        if ($LASTEXITCODE -ne 0) {
            throw "$name failed the portable version check"
        }
        & $executable --help | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "$name failed the portable help check"
        }
    }
    Write-Output 'Both portable commands run successfully without Visual C++ runtime DLL imports'
} finally {
    Remove-Item -Recurse -Force $directory
}
