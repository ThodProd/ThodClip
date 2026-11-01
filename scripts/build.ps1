<#
  Builds ThodClip and packs it for ThodHUB.

  Example:
    .\scripts\build.ps1
    .\scripts\build.ps1 -Version 1.0.2 -Notes "Что изменилось"
#>
[CmdletBinding()]
param(
    [string]$Version = '',
    [string]$Configuration = 'Release',
    [string]$OutputDirectory = '',
    [string]$Notes = '',
    [switch]$SkipPackage
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $root 'hub\manifest.json'
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

if (-not $Version) { $Version = $manifest.version }
if ($Version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw "Version must look like 1.2.3, got '$Version'."
}
if (-not $Notes) { $Notes = "ThodClip $Version — история буфера обмена и архив скриншотов." }

if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root 'artifacts' }
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
$stage = Join-Path $OutputDirectory "stage-$Version"

Write-Host "ThodClip $Version"
Write-Host "Workspace: $root"

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

# --- worker (self-contained) -------------------------------------------------
$workerProject = Join-Path $root 'src\ThodClip.Worker\ThodClip.Worker.csproj'
$workerOut = Join-Path $stage 'worker'
Write-Host 'Publishing worker (win-x64, self-contained) ...'
& dotnet publish $workerProject `
    -c $Configuration `
    -r win-x64 `
    --self-contained true `
    -p:Version=$Version `
    -p:DebugType=None `
    -p:DebugSymbols=false `
    -p:PublishSingleFile=false `
    -o $workerOut
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed with exit code $LASTEXITCODE." }

# --- UI ---------------------------------------------------------------------
$uiOut = Join-Path $stage 'ui'
New-Item -ItemType Directory -Path $uiOut -Force | Out-Null
foreach ($file in 'index.html', 'styles.css', 'app.js', 'thodhub.js') {
    Copy-Item -LiteralPath (Join-Path $root "ui\$file") -Destination $uiOut -Force
}

# --- manifest ---------------------------------------------------------------
$manifest.version = $Version
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText((Join-Path $stage 'manifest.json'), ($manifest | ConvertTo-Json -Depth 10), $utf8NoBom)

# --- bundled docs / licenses ------------------------------------------------
foreach ($file in 'README.md', 'THIRD-PARTY-NOTICES.md', 'LICENSE') {
    Copy-Item -LiteralPath (Join-Path $root $file) -Destination $stage -Force
}
$licensesOut = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Path $licensesOut -Force | Out-Null
foreach ($file in 'MIT.txt', 'Apache-2.0.txt', 'SQLite-blessing.txt') {
    Copy-Item -LiteralPath (Join-Path $root "licenses\$file") -Destination $licensesOut -Force
}

$assetsOut = Join-Path $stage 'assets'
New-Item -ItemType Directory -Path $assetsOut -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'assets\thodclip.svg') -Destination $assetsOut -Force

# --- package ----------------------------------------------------------------
if (-not $SkipPackage) {
    $packageScript = Join-Path $root 'scripts\package-module.ps1'
    $zip = & $packageScript `
        -PackageDirectory $stage `
        -Repository 'ThodProd/ThodClip' `
        -OutputDirectory $OutputDirectory `
        -Notes $Notes
    if (-not $zip -or -not (Test-Path -LiteralPath $zip)) { throw 'Packaging failed: ZIP was not produced.' }

    $releasePath = Join-Path $OutputDirectory 'thodhub-release.json'
    $release = Get-Content -LiteralPath $releasePath -Raw | ConvertFrom-Json
    $actualHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    $actualSize = (Get-Item -LiteralPath $zip).Length
    if ($release.sha256 -ne $actualHash -or [long]$release.size -ne $actualSize) {
        throw 'Packaging verification failed: SHA-256/size do not match thodhub-release.json.'
    }
    Write-Host ''
    Write-Host "Package : $zip"
    Write-Host "SHA-256 : $actualHash"
    Write-Host "Size    : $actualSize bytes"
    Write-Host "Release : $releasePath"
} else {
    Write-Host "Staged  : $stage"
}
