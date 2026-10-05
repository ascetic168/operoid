# 開發重建腳本：解決「跑到舊版 oserver／obridge」的兩個來源——
#   1. 只編譯單一 crate（sibling 解析拿到另一個的舊檔）→ 本腳本兩個一起建；
#   2. Windows 下 cargo 無法替換**執行中**的 exe（os error 5，錯誤易被漏看）
#      → 先停掉 dev 行程（已註冊為服務的實例不在殺除範圍，另行提示）。
# 建完印出兩個執行檔的 build id 供核對（兩者 hash 必須一致）。
# 用法：powershell -File scripts\dev-rebuild.ps1   （或 cargo dev-build，但不含殺行程）
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

# 服務模式（SCM 註冊）的 oserver 不歸本腳本管——提示手動停，避免誤殺正式服務。
# （首次使用尚未建置時 target\debug\oserver.exe 不存在——略過偵測直接建。）
if (Test-Path "target\debug\oserver.exe") {
    $svcOut = & target\debug\oserver.exe status 2>$null
    $svc = $svcOut | ConvertFrom-Json
    if ($svc.installed) {
        Write-Warning "oserver 已註冊為系統服務——請先停止服務（sc stop Operoid 或 net stop Operoid）再重建，本腳本不會動它。"
    } else {
        foreach ($proc in @("oserver", "obridge")) {
            $running = Get-Process -Name $proc -ErrorAction SilentlyContinue
            if ($running) {
                Write-Host "停掉 dev 行程：$proc（pid=$($running.Id -join ',')）"
                Stop-Process -Name $proc -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

cargo dev-build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n── build id 核對（兩行 hash 必須一致）──"
& target\debug\obridge.exe --version
& target\debug\oserver.exe version
