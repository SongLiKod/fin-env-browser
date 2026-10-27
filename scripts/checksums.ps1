param(
  [string]$Destination = "artifacts"
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
New-Item -ItemType Directory -Force -Path $Destination | Out-Null

Get-ChildItem -Path (Join-Path $root 'src-tauri\target') -Recurse -File -ErrorAction SilentlyContinue |
  Where-Object { $_.Length -gt 1MB -and $_.Extension -in '.exe', '.msi', '.msix', '.dmg', '.appimage' } |
  ForEach-Object { Copy-Item $_.FullName -Destination $Destination -Force }

$config = Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
Set-Content -Path (Join-Path $Destination 'VERSION') -Value $config.version -NoNewline

$lines = Get-ChildItem $Destination -File | Where-Object { $_.Name -ne 'SHA256SUMS.txt' } | ForEach-Object {
  $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()
  "$hash  $($_.Name)"
}
Set-Content -Path (Join-Path $Destination 'SHA256SUMS.txt') -Value $lines

Get-ChildItem $Destination | Format-Table Name, Length
