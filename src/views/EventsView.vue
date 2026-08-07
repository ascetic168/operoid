<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { agentRecentEvents, type EventWithMeta } from "@/lib/tauri";

const { t } = useI18n();
const events = ref<EventWithMeta[]>([]);
let timer: ReturnType<typeof setInterval> | null = null;

async function refresh() {
  try {
    events.value = await agentRecentEvents(100);
  } catch {
    // 唯讀觀察：靜默，下個 tick 再試。
  }
}
function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString([], {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
/** 事件 kind 色彩（沿用 watch 面板 events 配色）。 */
function kindColor(kind: string): string {
  if (kind === "satisfied" || kind === "wake" || kind === "artifact") return "text-emerald-500";
  if (kind === "stalled") return "text-amber-500";
  if (kind === "errored") return "text-destructive";
  return "text-foreground";
}
onMounted(() => {
  refresh();
  timer = setInterval(refresh, 3000);
});
onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div class="flex h-full w-full flex-col overflow-hidden">
    <!-- 標題列 -->
    <div class="flex items-center gap-2 border-b border-border px-4 py-2.5">
      <h2 class="text-sm font-semibold">{{ t("events.title") }}</h2>
    </div>

    <!-- 時間軸 -->
    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      <div v-if="events.length === 0" class="py-10 text-center text-sm text-muted-foreground">
        {{ t("events.empty") }}
      </div>
      <div v-for="ev in events" :key="ev.id" class="mb-1 flex items-baseline gap-2 font-mono text-xs">
        <span class="shrink-0 text-muted-foreground">{{ formatTime(ev.created_at) }}</span>
        <span class="shrink-0 font-medium" :class="kindColor(ev.kind)">{{ ev.kind }}</span>
        <span class="shrink-0 text-muted-foreground">{{ ev.employee_name }}</span>
        <span class="truncate text-foreground">{{ ev.detail }}</span>
      </div>
    </div>
  </div>
</template>
