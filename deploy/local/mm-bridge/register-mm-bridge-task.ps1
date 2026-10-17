$ErrorActionPreference = 'Stop'
$bun  = 'C:\Users\charl\.bun\bin\bun.exe'
$script = 'C:\Users\charl\AppData\Local\llama-cpp\mm-bridge.mjs'
$user = "$env:USERDOMAIN\$env:USERNAME"

$act = New-ScheduledTaskAction -Execute $bun -Argument "`"$script`""
$trg = New-ScheduledTaskTrigger -AtLogOn -User $user
# S4U：免密碼、隱藏視窗（非互動 session），登入即啟動
$pr  = New-ScheduledTaskPrincipal -UserId $user -LogonType S4U -RunLevel Limited
# 無執行時限（預設 72h 會殺掉常駐 shim）、可跨電源、失敗重啟
$set = New-ScheduledTaskSettingsSet `
  -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
  -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) `
  -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName 'mm-bridge' -Action $act -Trigger $trg -Principal $pr -Settings $set -Force | Out-Null
Write-Output "task 'mm-bridge' registered (S4U logon trigger, no time limit)"
