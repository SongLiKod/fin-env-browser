param(
  [switch]$SkipBundle
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location (Join-Path $root 'frontend')
try {
  npm ci
  npm run check
} finally {
  Pop-Location
}
Push-Location (Join-Path $root 'src-tauri')
try {
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
} finally {
  Pop-Location
}
if (-not $SkipBundle) {
  Push-Location $root
  try { cargo tauri build } finally { Pop-Location }
  & (Join-Path $PSScriptRoot 'checksums.ps1')
}
