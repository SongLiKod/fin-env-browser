param(
  [string]$Output = "artifacts/finenv-portable-windows.zip"
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'src-tauri\target\release\finenv-browser.exe'
if (-not (Test-Path $exe)) { throw "未找到 $exe，请先执行 cargo tauri build" }
$tmp = Join-Path $env:TEMP ("finenv-portable-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
Copy-Item $exe (Join-Path $tmp 'FinEnv.exe')
Copy-Item (Join-Path $root 'README.md') $tmp
New-Item -ItemType Directory -Force -Path (Join-Path $tmp 'chromium-bin') | Out-Null
Copy-Item (Join-Path $root 'chromium-bin\README.md') (Join-Path $tmp 'chromium-bin')
$dest = Join-Path $root $Output
New-Item -ItemType Directory -Force -Path (Split-Path $dest) | Out-Null
if (Test-Path $dest) { Remove-Item $dest -Force }
Compress-Archive -Path (Join-Path $tmp '*') -DestinationPath $dest -CompressionLevel Optimal
Remove-Item $tmp -Recurse -Force
Write-Host "便携版: $dest"
