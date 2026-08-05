<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { ArrowLeft, Loader2, Send } from "lucide-vue-next";
import { agentSendMessage, agentWatch, formatError, type WatchSnapshot } from "@/lib/tauri";

const props = defineProps<{ id: string }>();
const { t } = useI18n();
const router = useRouter();

const data = ref<WatchSnapshot | null>(null);
const input = ref("");
const sending = ref(false);
const error = ref<string | null>(null);
const scrollEl = ref<HTMLElement | null>(null);
let timer: ReturnType<typeof setInterval> | null = null;

/** 對話串＝messages 反轉成時序（最舊在上、最新在下）。 */
const thread = computed(() => (data.value ? [...data.value.messages].reverse() : []));

async function poll() {
  try {
    data.value = await agentWatch(props.id);
    await nextTick();
    if (scrollEl.value) scrollEl.value.scrollTop = scrollEl.value.scrollHeight;
  } catch {
    // 唯讀觀察：靜默，下個 tick 再試。
  }
}
async function send() {
  const text = input.value.trim();
  if (!text) return;
  sending.value = true;
  error.value = null;
  try {
    await agentSendMessage(props.id, text, null);
    input.value = "";
    await poll();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    sending.value = false;
  }
}
onMounted(() => {
  poll();
  timer = setInterval(poll, 1500);
});
onUnmounted(() => {
  if (timer) clearInterval(timer);
});

function stateColor(s: string | undefined): string {
  switch (s) {
    case "working":
      return "bg-emerald-500";
    case "sleeping":
      return "bg-zinc-400";
    case "error":
      return "bg-destructive";
    case "paused":
      return "bg-amber-500";
    default:
      return "bg-sky-500";
  }
}
</script>

<template>
  <div class="flex h-full w-full flex-col">
    <!-- 標題列 -->
    <div class="flex items-center gap-2 border-b border-border px-4 py-2.5">
      <button class="text-muted-foreground hover:text-foreground" @click="router.push('/instances')">
        <ArrowLeft :size="16" />
      </button>
      <span class="h-2 w-2 rounded-full" :class="stateColor(data?.employee.state)" />
      <span class="text-sm font-medium">{{ data?.employee.name ?? "…" }}</span>
      <span class="text-xs text-muted-foreground">{{ data?.employee.state ?? "" }}</span>
      <span v-if="data?.llm_model" class="ml-auto rounded bg-accent px-1.5 py-0.5 text-[10px] text-muted-foreground">{{ data.llm_model }}</span>
    </div>

    <!-- 對話捲動區 -->
    <div ref="scrollEl" class="min-h-0 flex-1 overflow-y-auto p-4">
      <div
        v-if="thread.length === 0"
        class="py-10 text-center text-sm text-muted-foreground"
      >
        {{ t("chat.empty") }}
      </div>
      <div
        v-for="m in thread"
        :key="m.id"
        class="mb-2 flex"
        :class="m.direction === 'out' ? 'justify-end' : 'justify-start'"
      >
        <div
          class="max-w-[75%] whitespace-pre-wrap rounded-lg px-3 py-1.5 text-sm"
          :class="
            m.direction === 'out'
              ? 'bg-primary text-primary-foreground'
              : 'bg-accent text-foreground'
          "
        >
          {{ m.text }}
        </div>
      </div>
    </div>

    <!-- 輸入區 -->
    <div class="border-t border-border p-3">
      <div class="flex items-end gap-2">
        <textarea
          v-model="input"
          rows="1"
          :placeholder="t('chat.inputPh')"
          class="max-h-32 flex-1 resize-none rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus:ring-1 focus:ring-ring"
          @keydown.enter.exact.prevent="send"
        />
        <button
          type="button"
          :disabled="sending || !input.trim()"
          class="flex items-center gap-1 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
          @click="send"
        >
          <Loader2 v-if="sending" :size="14" class="animate-spin" />
          <Send v-else :size="14" />
          {{ t("chat.send") }}
        </button>
      </div>
      <p v-if="error" class="mt-1 text-xs text-destructive">{{ error }}</p>
    </div>
  </div>
</template>
