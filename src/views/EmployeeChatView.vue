<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { ArrowLeft, Eraser, Loader2, Send } from "lucide-vue-next";
import {
  agentApproveCommitment,
  agentClearMessages,
  agentRejectCommitment,
  agentSendMessage,
  agentWatch,
  formatError,
  type WatchSnapshot,
} from "@/lib/tauri";

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

/** 將 RFC 3339 時間戳格式化為本地 HH:MM。失敗回空字串。 */
function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** 目前待核可的提案 commitment IDs（用來在 Out 氣泡上顯示核可／拒絕鈕）。 */
const proposedIds = computed(
  () => new Set((data.value?.proposals ?? []).map((p) => p.id)),
);

async function approve(cid: string) {
  try {
    await agentApproveCommitment(cid);
    await poll();
  } catch (e) {
    error.value = formatError(e);
  }
}
async function reject(cid: string) {
  try {
    await agentRejectCommitment(cid);
    await poll();
  } catch (e) {
    error.value = formatError(e);
  }
}

// ── 清除對話 ──
const clearOpen = ref(false);
const clearing = ref(false);

async function confirmClear() {
  clearing.value = true;
  try {
    await agentClearMessages(props.id);
    clearOpen.value = false;
    await poll();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    clearing.value = false;
  }
}

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
      <div class="ml-auto flex items-center gap-2">
        <span v-if="data?.llm_model" class="rounded bg-accent px-1.5 py-0.5 text-[10px] text-muted-foreground">{{ data.llm_model }}</span>
        <button
          v-if="thread.length > 0"
          class="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
          :title="t('chat.clear')"
          @click="clearOpen = true"
        >
          <Eraser :size="13" /> {{ t("chat.clear") }}
        </button>
      </div>
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
        class="mb-2 flex flex-col"
        :class="m.direction === 'out' ? 'items-end' : 'items-start'"
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
        <!-- 時間戳（氣泡下方） -->
        <time
          v-if="formatTime(m.created_at)"
          class="mt-0.5 px-1 text-[10px] text-muted-foreground"
          :class="m.direction === 'out' ? 'text-right' : 'text-left'"
        >
          {{ formatTime(m.created_at) }}
        </time>
        <!-- 提案核可鈕（僅 Out message 的 commitment 在 proposals 待核可時顯示） -->
        <div
          v-if="m.direction === 'out' && m.commitment_id && proposedIds.has(m.commitment_id)"
          class="mt-1 flex gap-2"
        >
          <button
            class="flex items-center gap-1 rounded bg-emerald-600 px-2.5 py-1 text-xs text-white hover:opacity-90"
            @click="approve(m.commitment_id!)"
          >
            ✓ {{ t("approval.approve") }}
          </button>
          <button
            class="rounded border border-border px-2.5 py-1 text-xs hover:bg-accent"
            @click="reject(m.commitment_id!)"
          >
            ✗ {{ t("approval.reject") }}
          </button>
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

    <!-- 清除對話確認 modal -->
    <div
      v-if="clearOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="clearOpen = false"
    >
      <div class="w-full max-w-sm rounded-xl border border-border bg-card p-5 shadow-2xl">
        <h3 class="mb-2 font-semibold">{{ t("chat.clearConfirmTitle") }}</h3>
        <p class="text-sm text-muted-foreground">{{ t("chat.clearConfirmText") }}</p>
        <p v-if="error" class="mt-2 text-xs text-destructive">{{ error }}</p>
        <div class="mt-4 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="clearOpen = false"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            :disabled="clearing"
            class="flex items-center gap-1 rounded-md bg-destructive px-3 py-1.5 text-xs text-destructive-foreground hover:opacity-90 disabled:opacity-50"
            @click="confirmClear"
          >
            <Loader2 v-if="clearing" :size="13" class="animate-spin" />
            <Eraser v-else :size="13" />
            {{ t("chat.clear") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
