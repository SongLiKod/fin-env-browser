param(
  [string]$Destination = "",
  [string]$DataDir = ""
)
$ErrorActionPreference = 'Stop'
if (-not $DataDir) {
  if ($env:FIN_ENV_DATA_DIR) { $DataDir = $env:FIN_ENV_DATA_DIR }
  else { $DataDir = Join-Path $env:LOCALAPPDATA 'FinEnv' }
}
if (-not (Test-Path $DataDir)) { throw "FinEnv 数据目录不存在: $DataDir" }
if (-not $Destination) {
  $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
  $Destination = Join-Path (Get-Location) "finenv-local-$stamp.zip"
}
Write-Host "备份 $DataDir -> $Destination"
Compress-Archive -Path (Join-Path $DataDir '*') -DestinationPath $Destination -Force -CompressionLevel Optimal
Write-Host "完成: $Destination"
