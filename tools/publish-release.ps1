[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('production-menu','gui-studio')][string]$Product,
      [Parameter(Mandatory)][string]$Tag,
      [Parameter(Mandatory)][string]$Repository,
      [Parameter(Mandatory)][string]$AssetDirectory)
$ErrorActionPreference = 'Stop'
$version = (Get-Content (Join-Path $PSScriptRoot 'release-versions.json') -Raw | ConvertFrom-Json).$Product
if ($Tag -cne "$Product-v$version") { throw 'Tag does not match product version' }
if (-not (Test-Path (Join-Path $PSScriptRoot '../LICENSE'))) { throw 'License is required for release' }
$checksums = Join-Path $AssetDirectory 'SHA256SUMS.txt'
$names = if ($Product -eq 'production-menu') { @("Production-Menu-$version-gog.zip", "Production-Menu-$version-history-edition.zip") } else { @("Anno-GUI-Studio-$version-windows-x64.zip") }
$rows = @(Get-Content -LiteralPath $checksums)
if ($rows.Count -ne $names.Count) { throw 'Unexpected checksum count' }
foreach ($name in $names) {
    $hash = (Get-FileHash -LiteralPath (Join-Path $AssetDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($rows -cnotcontains "$hash  $name") { throw "Checksum failed: $name" }
}
$actual = @(Get-ChildItem -LiteralPath $AssetDirectory -File -Recurse | ForEach-Object { $_.Name } | Sort-Object)
if (@(Compare-Object (@($names) + @('SHA256SUMS.txt') | Sort-Object) $actual).Count) { throw 'Unexpected release asset' }
$notes = Join-Path $PSScriptRoot "../docs/releases/$Product.md"
# Fail rather than changing an existing release's assets on a rerun.
$existing = & gh release list --repo $Repository --limit 100 --json tagName
if ($LASTEXITCODE) { throw 'Could not inspect releases' }
if (@($existing | ConvertFrom-Json | Where-Object tagName -eq $Tag).Count) { throw 'Release exists; assets will not be overwritten' }
$assets = @($names | ForEach-Object { Join-Path $AssetDirectory $_ }) + @($checksums)
# Create as draft so failed uploads never leave a partial public release.
& gh release create $Tag @assets --repo $Repository --verify-tag --title "$Product $version" --notes-file $notes --draft --prerelease
if ($LASTEXITCODE) { throw 'Draft release creation failed; inspect before rerunning' }
& gh release edit $Tag --repo $Repository --draft=false --prerelease
if ($LASTEXITCODE) { throw 'Release remains a draft; inspect before publishing' }
