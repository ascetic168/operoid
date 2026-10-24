import { defineStore } from "pinia";
import { ref } from "vue";
import { check as checkUpdater, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * 更新檢查／安裝 store（個人版）。
 *
 * 走 Tauri 官方 updater 外掛：查 `tauri.conf.json` 設定的 latest.json endpoint
 * （GitHub Releases 最新發布），比對版本後可下載安裝（NSIS 靜默安裝／.app 替換／
 * AppImage 原地替換）並重新啟動。設定語意（AppConfig）：
 * - `update_notify`（預設開）：啟動＋每 24h 自動檢查，有新版顯示橫幅（安裝需按一下）。
 * - `auto_update`（預設關）：偵測到新版即自動下載安裝，完成後提示重新啟動。
 *
 * 錯誤分類：`noinfo`＝latest.json 尚不存在（新管線首版發布前），對使用者呈現
 * 「暫無更新資訊」而非原始錯誤；其餘保留原始訊息供診斷。
 */
export type UpdateState = "idle" | "checking" | "available" | "downloading" | "ready" | "uptodate" | "error";
export type UpdateErrorKind = "noinfo" | "other";

export const useUpdateStore = defineStore("update", () => {
  const state = ref<UpdateState>("idle");
  const errorKind = ref<UpdateErrorKind>("other");
  const errorMsg = ref<string | null>(null);
  /** 新版版本號（state=available／ready 時有值）。 */
  const version = ref<string | null>(null);
  /** 新版發布說明（updater 的 update.body，markdown 純文字）。 */
  const notes = ref<string | null>(null);
  /** 下載進度 0–100（null＝總長未知）。 */
  const progress = ref<number | null>(null);
  /** 上次自動檢查時間（本 session 內去重；24h 週期用）。 */
  const lastCheckedAt = ref<number | null>(null);

  // 檢查結果的 Update 物件不進 reactive（避免 proxy 干擾外掛內部）；重新檢查前先 close 釋放。
  let pending: Update | null = null;
  let autoTimer: ReturnType<typeof setInterval> | null = null;

  function reset() {
    state.value = "idle";
    errorKind.value = "other";
    errorMsg.value = null;
    version.value = null;
    notes.value = null;
    progress.value = null;
  }

  function classifyError(e: unknown): { kind: UpdateErrorKind; msg: string } {
    const raw = e instanceof Error ? e.message : String(e);
    if (/404|not\s*found/i.test(raw)) return { kind: "noinfo", msg: raw };
    return { kind: "other", msg: raw };
  }

  /**
   * 檢查更新。回傳 true＝有新版。`silent`＝錯誤不進入 error 狀態（啟動自動檢查用，
   * 避免網路不穩就跳錯誤橫幅）。
   */
  async function check(silent = false): Promise<boolean> {
    state.value = "checking";
    errorMsg.value = null;
    try {
      try {
        // 上一次檢查未安裝的 Update 物件釋放（Windows 需關閉下載檔 handle）。
        await pending?.close();
      } catch {
        /* 已失效則忽略 */
      }
      pending = null;
      const update = await checkUpdater();
      lastCheckedAt.value = Date.now();
      if (!update) {
        version.value = null;
        notes.value = null;
        state.value = "uptodate";
        return false;
      }
      pending = update;
      version.value = update.version;
      notes.value = update.body ?? null;
      state.value = "available";
      return true;
    } catch (e) {
      const { kind, msg } = classifyError(e);
      if (silent) {
        state.value = "idle";
      } else {
        state.value = "error";
        errorKind.value = kind;
        errorMsg.value = msg;
      }
      return false;
    }
  }

  /** 下載並安裝（available → downloading → ready；Windows 安裝期間 App 會被安裝器關閉）。 */
  async function downloadAndInstall(): Promise<void> {
    if (!pending) return;
    state.value = "downloading";
    progress.value = null;
    try {
      let received = 0;
      let total = 0;
      await pending.downloadAndInstall((event) => {
        if (event.event === "Started") {
          received = 0;
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          if (total > 0) progress.value = Math.min(100, Math.round((received / total) * 100));
        } else if (event.event === "Finished") {
          progress.value = 100;
        }
      });
      state.value = "ready";
    } catch (e) {
      state.value = "error";
      errorKind.value = "other";
      errorMsg.value = e instanceof Error ? e.message : String(e);
    }
  }

  /** 重新啟動以套用已安裝的更新。 */
  async function relaunchNow(): Promise<void> {
    await relaunch();
  }

  /**
   * 啟動自動檢查迴圈（App.vue onMounted 呼叫）：依 AppConfig 決定行為——
   * auto_update 開 → 檢查到新版直接背景下載安裝（完成後由橫幅提示重啟）；
   * 否則 update_notify 開 → 只檢查＋橫幅。之後每 24h 重評估（讀最新設定）。
   */
  function startAutoLoop(getFlags: () => { notify: boolean; auto: boolean }) {
    stopAutoLoop();
    const tick = async () => {
      const { notify, auto } = getFlags();
      if (!notify && !auto) return;
      const found = await check(true);
      if (found && auto) void downloadAndInstall();
    };
    void tick();
    autoTimer = setInterval(() => void tick(), 24 * 60 * 60 * 1000);
  }

  function stopAutoLoop() {
    if (autoTimer !== null) {
      clearInterval(autoTimer);
      autoTimer = null;
    }
  }

  return {
    state,
    errorKind,
    errorMsg,
    version,
    notes,
    progress,
    lastCheckedAt,
    check,
    downloadAndInstall,
    relaunchNow,
    startAutoLoop,
    stopAutoLoop,
    reset,
  };
});
