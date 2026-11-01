param(
    [Parameter(Mandatory=$true)][string]$PackageDirectory,
    [Parameter(Mandatory=$true)][string]$Repository,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [string]$Notes = 'Первый выпуск для ThodHUB.'
)
$ErrorActionPreference = 'Stop'
$manifest = Get-Content -LiteralPath (Join-Path $PackageDirectory 'manifest.json') -Raw | ConvertFrom-Json
if ($manifest.version -notmatch '^\d+\.\d+\.\d+$' -or $manifest.id -notmatch '^[a-z][a-z0-9-]+$') { throw 'Invalid manifest' }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$name = "$($manifest.id)-$($manifest.version)-win-x64.zip"
$zip = Join-Path $OutputDirectory $name
if (Test-Path -LiteralPath $zip) { throw "Package already exists: $zip" }
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$rootPath = (Resolve-Path -LiteralPath $PackageDirectory).Path
$archive = [System.IO.Compression.ZipFile]::Open($zip, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    # Entry names must use '/' and must not contain the parent folder.
    Get-ChildItem -LiteralPath $rootPath -Recurse -File | Sort-Object FullName | ForEach-Object {
        $relative = $_.FullName.Substring($rootPath.Length + 1).Replace('\', '/')
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $archive, $_.FullName, $relative, [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
}
finally {
    $archive.Dispose()
}
$release = [ordered]@{
    version = $manifest.version
    packageUrl = "https://github.com/$Repository/releases/download/hub-v$($manifest.version)/$name"
    sha256 = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    size = (Get-Item -LiteralPath $zip).Length
    notes = $Notes
    platform = $manifest.platform
    hubApi = $manifest.hubApi
}
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText((Join-Path $OutputDirectory 'thodhub-release.json'), ($release | ConvertTo-Json -Depth 5), $utf8NoBom)
Write-Output $zip
