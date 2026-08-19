param(
  [ValidateSet('x64', 'x86', 'arm64')]
  [string]$Arch = 'x64'
)

if ($Arch -eq 'x86') {
  $Arch = 'x64'
}

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Dist = Join-Path $Root 'dist'
$Manifest = Get-Content -LiteralPath (Join-Path $Root 'package.json') -Raw | ConvertFrom-Json
$Version = [string]$Manifest.version
$RustTarget = if ($Arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$BuiltExe = Join-Path $Root "desktop-sample\src-tauri\target\$RustTarget\release\cursor-i18n-desktop-sample.exe"
if (!(Test-Path -LiteralPath $BuiltExe -PathType Leaf) -and $Arch -eq 'x64') {
  $BuiltExe = Join-Path $Root 'desktop-sample\src-tauri\target\release\cursor-i18n-desktop-sample.exe'
}
$CliZip = Join-Path $Dist 'cursor-i18n-zh-windows.zip'
$WorkbenchName = 'localization-workbench'
$Suffix = "windows-$Arch"
$ExeName = "$WorkbenchName-v$Version-$Suffix.exe"
$PortableName = "$WorkbenchName-v$Version-$Suffix.zip"
$PublishedExe = Join-Path $Dist $ExeName
$PortableZip = Join-Path $Dist $PortableName
$PortableTemp = Join-Path $Dist ("desktop-portable-$([guid]::NewGuid().ToString('N')).zip")
$Stage = Join-Path $Dist ("desktop-stage-$([guid]::NewGuid().ToString('N'))")
$Checksums = Join-Path $Dist "SHA256SUMS-$Suffix.txt"

function Get-Sha256([string]$Path) {
  $stream = [IO.File]::OpenRead($Path)
  $sha256 = [Security.Cryptography.SHA256]::Create()
  try {
    return ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
  } finally {
    $sha256.Dispose()
    $stream.Dispose()
  }
}

foreach ($file in @($BuiltExe, $CliZip)) {
  if (!(Test-Path -LiteralPath $file -PathType Leaf)) {
    throw "Missing desktop package input: $file"
  }
}

$fileVersion = (Get-Item -LiteralPath $BuiltExe).VersionInfo.ProductVersion
if ($fileVersion -ne $Version) {
  throw "Desktop EXE version $fileVersion does not match package version $Version"
}

New-Item -ItemType Directory -Force -Path $Dist | Out-Null
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

try {
  Expand-Archive -LiteralPath $CliZip -DestinationPath $Stage
  Copy-Item -LiteralPath $BuiltExe -Destination (Join-Path $Stage $ExeName)
  Copy-Item -LiteralPath (Join-Path $Root 'desktop-sample\README.md') -Destination (Join-Path $Stage 'README-DESKTOP.md')

  $ClaudeLicenses = Join-Path $Stage 'licenses\claude-translation-memory'
  New-Item -ItemType Directory -Force -Path $ClaudeLicenses | Out-Null
  Copy-Item -LiteralPath (Join-Path $Root 'desktop-sample\resources\claude\SOURCE.md') -Destination $ClaudeLicenses
  Copy-Item -LiteralPath (Join-Path $Root 'desktop-sample\resources\claude\APACHE-2.0.txt') -Destination $ClaudeLicenses

  foreach ($relative in @(
    $ExeName,
    'compat\cursor-stable.json',
    'src\cli.js',
    'dict',
    'node_modules\acorn\package.json',
    'node_modules\opencc-js\package.json',
    'LICENSE',
    'CHANGELOG.md',
    'THIRD_PARTY_LICENSES',
    'licenses\claude-translation-memory\SOURCE.md',
    'licenses\claude-translation-memory\APACHE-2.0.txt'
  )) {
    if (!(Test-Path -LiteralPath (Join-Path $Stage $relative))) {
      throw "Desktop portable package is missing: $relative"
    }
  }

  Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $PortableTemp -Force
  Copy-Item -LiteralPath $BuiltExe -Destination $PublishedExe -Force
  if (Test-Path -LiteralPath $PortableZip) {
    Remove-Item -LiteralPath $PortableZip -Force
  }
  Move-Item -LiteralPath $PortableTemp -Destination $PortableZip

  $assets = @($PublishedExe, $PortableZip, $CliZip)
  $lines = foreach ($asset in $assets) {
    $hash = Get-Sha256 $asset
    "$hash  $([IO.Path]::GetFileName($asset))"
  }
  $encoding = [Text.UTF8Encoding]::new($false)
  [IO.File]::WriteAllLines($Checksums, $lines, $encoding)
  [IO.File]::WriteAllLines((Join-Path $Dist 'SHA256SUMS.txt'), $lines, $encoding)

  Write-Host "Desktop EXE: $PublishedExe"
  Write-Host "Desktop portable package: $PortableZip"
  Write-Host "Checksums: $Checksums"
} finally {
  if (Test-Path -LiteralPath $Stage) {
    Remove-Item -LiteralPath $Stage -Recurse -Force
  }
  if (Test-Path -LiteralPath $PortableTemp) {
    Remove-Item -LiteralPath $PortableTemp -Force
  }
}
