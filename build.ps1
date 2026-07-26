# 构建 tikclock 并把 exe 拷入插件目录
$ErrorActionPreference = "Stop"

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$binDir = Join-Path $PSScriptRoot "com.vacio.tikclock.sdPlugin/bin"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item (Join-Path $PSScriptRoot "target/release/tikclock.exe") (Join-Path $binDir "tikclock.exe") -Force
Write-Host "Done -> $binDir\tikclock.exe"
