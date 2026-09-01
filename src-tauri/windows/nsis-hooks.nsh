; NSIS installer hooks —— oserver 開機服務的安裝／升級／反安裝處理。
; 服務由應用程式在設定頁註冊（oserver install），安裝器與反安裝器原本皆不會清理。
;
; - 升級安裝（PREINSTALL）：服務在跑會鎖住 oserver.exe 導致覆寫失敗／服務續用舊碼，
;   先停服務、裝完（POSTINSTALL）再啟動。以 `sc stop` 回傳碼偵測「原本在跑」：
;   回 0＝服務存在且接受了停止（原為 running）；非 0＝未安裝或已停止，不動。
; - 反安裝（PREUNINSTALL）：先走 oserver 正規 uninstall，再以 SCM 保險移除，
;   避免留下指向已刪除 exe 的孤兒服務。

Var OserverWasRunning

!macro NSIS_HOOK_PREINSTALL
  StrCpy $OserverWasRunning 0
  nsExec::ExecToStack 'sc stop Operoid'
  Pop $0
  ${If} $0 == 0
    StrCpy $OserverWasRunning 1
    ; 等 SCM 完成停止、釋放 oserver.exe 的檔案鎖再進行檔案覆寫
    Sleep 1500
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $OserverWasRunning == 1
    nsExec::ExecToLog 'sc start Operoid'
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 檔案刪除前執行：$INSTDIR\oserver.exe 仍在，先走正規移除（含資料庫參數由服務端自理）
  nsExec::ExecToLog '"$INSTDIR\oserver.exe" uninstall'
  ; 保險：exe 已失效或 uninstall 失敗時，直接以 SCM 停止並刪除服務
  nsExec::ExecToLog 'sc stop Operoid'
  nsExec::ExecToLog 'sc delete Operoid'
!macroend
