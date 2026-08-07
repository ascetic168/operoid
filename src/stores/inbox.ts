import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { agentInboxSummary, type InboxSummary } from "@/lib/tauri";

/** 動詞軌 store：跨員工聚合「需要人類關注的事」，供頂列鈴鐺／待辦頁使用。全域輪詢。 */
export const useInboxStore = defineStore("inbox", () => {
  const summary = ref<InboxSummary | null>(null);
  const lastError = ref<string | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  /** 待辦總數（待核可提案 + 異常員工）。鈴鐺徽章用。 */
  const pendingCount = computed(
    () =>
      (summary.value?.proposals.length ?? 0) +
      (summary.value?.flagged_employees.length ?? 0),
  );

  /** 拉取最新聚合；靜默失敗（鈴鐺是輔助性，不該阻斷主流程）。 */
  async function refresh() {
    try {
      summary.value = await agentInboxSummary();
      lastError.value = null;
    } catch {
      // 靜默；下個 tick 再試。
    }
  }

  /** 啟動全域輪詢（3s——比 per-view 1.5s 慢，全域聚合不需太頻繁）。 */
  function start() {
    refresh();
    if (timer) clearInterval(timer);
    timer = setInterval(refresh, 3000);
  }
  function stop() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  return { summary, pendingCount, lastError, refresh, start, stop };
});
