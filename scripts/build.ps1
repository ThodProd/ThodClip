<#
  Builds ThodClip and packs it for ThodHUB.

  Example:
    .\scripts\build.ps1
    .\scripts\build.ps1 -Version 1.1.2 -Notes "Что изменилось"
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

if ($Version -ne $manifest.version) { throw "Version $Version does not match the manifest ($($manifest.version))." }
$versionLine = Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"$'
if ($versionLine.Matches[0].Groups[1].Value -ne $Version) { throw 'Cargo workspace version does not match the manifest.' }

foreach ($tool in 'cargo', 'rustc', 'wasm-bindgen') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        $cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
        $env:Path = "$cargoBin;$env:Path"
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "'$tool' is required. See README.md." }
    }
}
$hostTarget = (& rustc -vV | Select-String '^host: (.*)$').Matches[0].Groups[1].Value
if ($hostTarget -notin 'x86_64-pc-windows-msvc', 'x86_64-pc-windows-gnu') { throw "Windows x64 toolchain required. Found $hostTarget." }

Write-Host "ThodClip $Version"
Write-Host "Workspace: $root"

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
if (Test-Path -LiteralPath $stage) {
    $outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
    $stageItem = Get-Item -LiteralPath $stage -Force
    $prefix = $outputPath.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not $stageItem.FullName.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -or
        ($stageItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Invalid staging directory: $stage"
    }
    Remove-Item -LiteralPath $stageItem.FullName -Recurse -Force
}
$workerOut = Join-Path $stage 'worker'
$uiOut = Join-Path $stage 'ui'
New-Item -ItemType Directory -Path $workerOut, $uiOut -Force | Out-Null

Push-Location -LiteralPath $root
try {
    $metadata = & cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve Cargo target directory.' }
    $target = $metadata.target_directory
    $profile = if ($Configuration -eq 'Debug') { 'debug' } else { 'release' }
    [string[]]$profileArgs = @(if ($profile -eq 'release') { '--release' })

    Write-Host "Building worker ($hostTarget) ..."
    & cargo build --locked -p thodclip-worker --target $hostTarget @profileArgs
    if ($LASTEXITCODE -ne 0) { throw 'Worker build failed.' }
    Copy-Item -LiteralPath (Join-Path $target "$hostTarget/$profile/thodclip-worker.exe") -Destination (Join-Path $workerOut 'ThodClip.Worker.exe')

    Write-Host 'Building embedded UI (WebAssembly) ...'
    & cargo build --locked -p thodclip-frontend --target wasm32-unknown-unknown @profileArgs
    if ($LASTEXITCODE -ne 0) { throw 'UI build failed.' }
    $wasmDir = Join-Path $target 'thodclip-bindings'
    & wasm-bindgen --target no-modules --no-typescript --out-name thodclip --out-dir $wasmDir (Join-Path $target "wasm32-unknown-unknown/$profile/thodclip_frontend.wasm")
    if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed.' }
    Copy-Item -LiteralPath (Join-Path $wasmDir 'thodclip.js') -Destination (Join-Path $uiOut 'thodclip.js')

    # The sandboxed module iframe has an opaque origin. Load the Wasm bytes through a local
    # classic script, which the hub already supports, without a cross-origin fetch.
    $binary = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $wasmDir 'thodclip_bg.wasm')))
    $initializer = @"
(() => {
  const bytes = Uint8Array.from(atob('$binary'), char => char.charCodeAt(0));
  wasm_bindgen({ module_or_path: bytes }).catch(error => {
    console.error(error);
    document.getElementById('captureLabel').textContent = 'Не удалось загрузить ThodClip';
  });
})();
"@
    [IO.File]::WriteAllText((Join-Path $uiOut 'start.js'), $initializer, [Text.UTF8Encoding]::new($false))
    foreach ($file in 'index.html', 'styles.css', 'thodhub.js') {
        Copy-Item -LiteralPath (Join-Path $root "ui/$file") -Destination $uiOut
    }
}
finally {
    Pop-Location
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
