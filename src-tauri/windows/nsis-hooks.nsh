; NSIS installer hooks —— 反安裝時移除 oserver 開機服務。
; 服務由應用程式在設定頁註冊（oserver install），安裝器與反安裝器原本皆不會清理，
; 不在此處移除會留下指向已刪除 exe 的孤兒服務。

!macro NSIS_HOOK_PREUNINSTALL
  ; 檔案刪除前執行：$INSTDIR\oserver.exe 仍在，先走正規移除（含資料庫參數由服務端自理）
  nsExec::ExecToLog '"$INSTDIR\oserver.exe" uninstall'
  ; 保險：exe 已失效或 uninstall 失敗時，直接以 SCM 停止並刪除服務
  nsExec::ExecToLog 'sc stop Operoid'
  nsExec::ExecToLog 'sc delete Operoid'
!macroend
