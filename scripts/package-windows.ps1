#Requires -Version 5.1
<#
.SYNOPSIS
  Offline packaging for Windows: portable zip, SHA256SUMS.txt, and (if installed) the NSIS setup.exe.
  Run from anywhere after `cargo build --release -p jide`.
#>
[CmdletBinding()]
param(
    [string]$Version
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\jide.exe'
$dist = Join-Path $root 'dist'

if (-not $Version) {
    $manifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
    if ($manifest -match '(?ms)\[workspace\.package\].*?^\s*version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        throw 'Could not read [workspace.package] version from Cargo.toml; pass -Version.'
    }
}

if (-not (Test-Path $exe)) {
    throw "Missing $exe. Run: cargo build --release -p jide"
}

New-Item -ItemType Directory -Force -Path $dist | Out-Null
$staging = Join-Path $dist "JustIDE-$Version-x64-portable"
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Force -Path $staging | Out-Null

Copy-Item $exe $staging
Set-Content -Path (Join-Path $staging 'portable.flag') -Value '' -NoNewline
Copy-Item (Join-Path $root 'README.md') $staging
Copy-Item (Join-Path $root 'docs\USER-GUIDE.md') $staging
$license = Join-Path $root 'LICENSE'
if (Test-Path $license) {
    Copy-Item $license $staging
} else {
    Write-Warning 'LICENSE not found at repo root; it will be missing from the artifacts.'
}

$zip = Join-Path $dist "JustIDE-$Version-x64-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip
Remove-Item -Recurse -Force $staging
Write-Host "Created $zip"

$makensis = Get-Command makensis.exe -ErrorAction SilentlyContinue
if (-not $makensis) {
    foreach ($dir in @("${env:ProgramFiles(x86)}\NSIS", "$env:ProgramFiles\NSIS")) {
        $candidate = Join-Path $dir 'makensis.exe'
        if (Test-Path $candidate) { $makensis = Get-Item $candidate; break }
    }
}

if ($makensis) {
    $path = if ($makensis.PSObject.Properties['Source']) { $makensis.Source } else { $makensis.FullName }
    & $path "/DVERSION=$Version" (Join-Path $root 'installer\just-ide.nsi')
    if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }
    Write-Host "Created JustIDE-$Version-x64-setup.exe"
} else {
    Write-Warning 'makensis.exe not found; skipping the installer. Install NSIS to build setup.exe.'
}

$sums = Get-ChildItem -Path $dist -File |
    Where-Object { $_.Name -match '^JustIDE-.*\.(zip|exe)$' } |
    Sort-Object Name |
    ForEach-Object {
        $hash = (Get-FileHash -Algorithm SHA256 -Path $_.FullName).Hash.ToLowerInvariant()
        "$hash  $($_.Name)"
    }
$sumsPath = Join-Path $dist 'SHA256SUMS.txt'
[System.IO.File]::WriteAllText($sumsPath, (($sums -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Host "Created $sumsPath"
$sums | ForEach-Object { Write-Host $_ }
