<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { ArrowLeft, Eraser, Loader2, Send, Wrench } from "lucide-vue-next";
import MarkdownText from "@/components/MarkdownText.vue";
import {
  agentApproveCommitment,
  agentClearMessages,
  agentGetArtifact,
  agentRejectCommitment,
  agentSendMessage,
  agentWatch,
  formatError,
  gateReasonText,
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

type ChatMessage = WatchSnapshot["messages"][number];
/** 對話串項目：訊息，或插入在最後一則 In 訊息之後的「工具過程列」。 */
type ThreadItem = { kind: "msg"; msg: ChatMessage } | { kind: "trace" };

/** tool_call 事件 detail（契約 v1：v/step/tool/args/status/ms/note；ocore record_tool_call_event）。 */
interface ToolCallStep {
  v: number;
  step: number;
  tool: string;
  args: string;
  status: string;
  ms: number;
  note: string;
}

function parseToolCall(detail: string): ToolCallStep | null {
  try {
    const d = JSON.parse(detail) as ToolCallStep;
    if (d && typeof d.tool === "string") return d;
  } catch {
    /* 非 JSON 或舊格式事件：略過 */
  }
  return null;
}

const working = computed(() => data.value?.employee.state === "working");
/** 送出訊息後、回覆抵達前：員工 working 且最新一則是 In（或尚無訊息）。 */
const awaitingReply = computed(() => {
  if (!working.value) return false;
  const msgs = data.value?.messages ?? [];
  return msgs.length === 0 || (msgs[0]?.direction ?? "in") === "in";
});

/** 本回合的過程：最後一則 In 訊息之後的 tool_call 事件（時序排列）。 */
const toolCalls = computed<ToolCallStep[]>(() => {
  const events = data.value?.events ?? [];
  let sinceTs = 0;
  for (const m of data.value?.messages ?? []) {
    if (m.direction !== "in") continue;
    const ts = new Date(m.created_at).getTime();
    if (!Number.isNaN(ts) && ts > sinceTs) sinceTs = ts;
  }
  const out: ToolCallStep[] = [];
  for (const e of events) {
    if (e.kind !== "tool_call") continue;
    if (sinceTs && new Date(e.created_at).getTime() < sinceTs) continue;
    const d = parseToolCall(e.detail);
    if (d) out.push(d);
  }
  return out.reverse(); // events 最新在前 → 時序
});

const traceOpen = ref(false);
const showTrace = computed(() => toolCalls.value.length > 0 || awaitingReply.value);

const thread = computed<ThreadItem[]>(() => {
  const msgs = data.value ? [...data.value.messages].reverse() : [];
  const items: ThreadItem[] = msgs.map((m) => ({ kind: "msg", msg: m }));
  if (showTrace.value) {
    let lastInIdx = -1;
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].direction === "in") {
        lastInIdx = i;
        break;
      }
    }
    items.splice(lastInIdx + 1, 0, { kind: "trace" });
  }
  return items;
});

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

/** R5（Ch.20 §5.4）：為何進到人類通道——把 gate_reason 機讀碼翻成可讀文字（M2 呈現設計：讓賭注可見）。 */
function whyPending(cid: string): string | null {
  const p = (data.value?.proposals ?? []).find((x) => x.id === cid);
  return gateReasonText(p?.gate_reason ?? null, t);
}

/** 動作中的 commitment id（防重入、期間 disable 按鈕）。 */
const pending = ref<string | null>(null);
/** 本 session 內已決策的提案（核可／拒絕後立刻顯示徽章；proposed_commitment_id 為鍵）。 */
const decided = ref(new Map<string, "approved" | "rejected">());

async function approve(cid: string) {
  if (pending.value) return; // 防重入
  pending.value = cid;
  try {
    await agentApproveCommitment(cid);
    decided.value.set(cid, "approved");
    await poll();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    pending.value = null;
  }
}
async function reject(cid: string) {
  if (pending.value) return; // 防重入
  pending.value = cid;
  try {
    await agentRejectCommitment(cid);
    decided.value.set(cid, "rejected");
    await poll();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    pending.value = null;
  }
}

// ── Artifact 展開：watch 只帶近 10 筆——更舊的展開時 fallback 載 API（F3）──
const openArtifacts = ref<Set<string>>(new Set());
function artifactOf(id: string) {
  return (
    loadedArtifacts.value[id] ??
    data.value?.artifacts.find((a) => a.id === id) ??
    null
  );
}
const loadedArtifacts = ref<Record<string, { title: string; content: string }>>({});
const artifactErrors = ref<Record<string, boolean>>({});
async function toggleArtifact(id: string) {
  if (!openArtifacts.value.has(id) && !artifactOf(id) && !artifactErrors.value[id]) {
    try {
      loadedArtifacts.value[id] = await agentGetArtifact(id);
    } catch (e) {
      artifactErrors.value[id] = true;
      void e;
    }
  }
  const next = new Set(openArtifacts.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  openArtifacts.value = next;
}

// ── 過程列 args 展開 ──
const openArgs = ref<Set<number>>(new Set());
function toggleArgs(i: number) {
  const next = new Set(openArgs.value);
  if (next.has(i)) next.delete(i);
  else next.add(i);
  openArgs.value = next;
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

let pollFailures = 0;

// ── 捲動：黏底偵測 ──
// 輪詢每 1.5s 刷新一次；若使用者上捲閱讀歷史，自動捲底會把視角拖走。
// 故只有「使用者本來就在底部附近」（黏底）才跟隨新訊息捲底；送出訊息時強制回底。
const stickToBottom = ref(true);

function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  stickToBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
}

function scrollToBottom(force = false) {
  const el = scrollEl.value;
  if (!el) return;
  if (force) stickToBottom.value = true;
  if (stickToBottom.value) el.scrollTop = el.scrollHeight;
}

async function poll() {
  try {
    data.value = await agentWatch(props.id);
    pollFailures = 0;
    await nextTick();
    scrollToBottom();
  } catch {
    // 唯讀觀察：靜默重試；連續失敗才提示使用者（不阻斷操作）。
    pollFailures += 1;
    if (pollFailures === 3) error.value = t("chat.pollError");
  }
}
async function send() {
  const text = input.value.trim();
  if (!text) return;
  sending.value = true;
  error.value = null;
  traceOpen.value = true; // 送出後自動展開過程列，讓「正在做什麼」可見
  try {
    await agentSendMessage(props.id, text, null);
    input.value = "";
    await poll();
    scrollToBottom(true);
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
    <div ref="scrollEl" class="min-h-0 flex-1 overflow-y-auto p-4" @scroll.passive="onScroll">
      <div
        v-if="thread.length === 0"
        class="py-10 text-center text-sm text-muted-foreground"
      >
        {{ t("chat.empty") }}
      </div>
      <template v-for="item in thread" :key="item.kind === 'msg' ? item.msg.id : 'trace'">
        <!-- 工具過程列：插在最後一則 In 訊息之後（本回合「正在做什麼」） -->
        <div v-if="item.kind === 'trace'" class="mb-2 w-full shrink-0">
          <div class="overflow-hidden rounded-lg border border-border bg-card">
            <button
              class="flex w-full items-center gap-2 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground"
              @click="traceOpen = !traceOpen"
            >
              <Loader2 v-if="awaitingReply" :size="12" class="animate-spin text-emerald-500" />
              <Wrench v-else :size="12" />
              <span v-if="awaitingReply" class="font-medium">{{ t("chat.processing") }}</span>
              <span v-if="toolCalls.length">{{ t("chat.traceCount", toolCalls.length) }}</span>
              <span class="ml-auto text-[10px]">{{ traceOpen ? "▾" : "▸" }}</span>
            </button>
            <div v-if="traceOpen" class="border-t border-border px-3 py-1.5">
              <div v-for="(c, ci) in toolCalls" :key="ci" class="py-1 text-xs">
                <div class="flex items-center gap-2">
                  <span
                    class="h-1.5 w-1.5 shrink-0 rounded-full"
                    :class="c.status === 'ok' ? 'bg-emerald-500' : 'bg-amber-500'"
                    :title="c.status"
                  />
                  <code class="font-medium">{{ c.tool }}</code>
                  <span class="text-muted-foreground">· {{ c.ms }}ms</span>
                  <span class="ml-auto text-[10px] text-muted-foreground">#{{ c.step }}</span>
                </div>
                <p
                  v-if="c.note"
                  class="mt-0.5 cursor-pointer pl-3.5 text-muted-foreground"
                  :class="openArgs.has(ci) ? '' : 'line-clamp-2'"
                  :title="c.args"
                  @click="toggleArgs(ci)"
                >
                  {{ c.note }}
                </p>
                <pre
                  v-if="openArgs.has(ci)"
                  class="mt-1 overflow-x-auto rounded border border-border bg-accent px-2 py-1 text-[10px] text-muted-foreground"
                >{{ c.args }}</pre>
              </div>
            </div>
          </div>
        </div>

        <!-- 訊息氣泡 -->
        <div
          v-else
          class="mb-2 flex flex-col shrink-0"
          :class="item.msg.direction === 'out' ? 'items-end' : 'items-start'"
        >
          <!-- 員工回覆：Markdown（共用元件內部 marked→DOMPurify 消毒） -->
          <div
            v-if="item.msg.direction === 'out'"
            class="max-w-[75%] rounded-lg bg-primary px-3 py-1.5 text-sm text-primary-foreground"
          >
            <MarkdownText :text="item.msg.text" />
          </div>
          <!-- 使用者訊息：純文字 -->
          <div
            v-else
            class="max-w-[75%] whitespace-pre-wrap rounded-lg bg-accent px-3 py-1.5 text-sm text-foreground"
          >
            {{ item.msg.text }}
          </div>
          <!-- 時間戳（氣泡下方） -->
          <time
            v-if="formatTime(item.msg.created_at)"
            class="mt-0.5 px-1 text-[10px] text-muted-foreground"
            :class="item.msg.direction === 'out' ? 'text-right' : 'text-left'"
          >
            {{ formatTime(item.msg.created_at) }}
          </time>
          <!-- Artifact 展開卡：watch 近 10 筆內直接顯示，更舊的 fallback 載 API -->
          <div v-if="item.msg.artifact_id" class="mt-1 w-full px-1">
            <button
              class="text-[10px] text-muted-foreground hover:text-foreground"
              @click="toggleArtifact(item.msg.artifact_id)"
            >
              📦 {{ artifactOf(item.msg.artifact_id)?.title ?? item.msg.artifact_id }}
              {{ openArtifacts.has(item.msg.artifact_id) ? "▾" : "▸" }}
            </button>
            <div
              v-if="openArtifacts.has(item.msg.artifact_id) && artifactOf(item.msg.artifact_id)"
              class="mt-1 max-h-60 overflow-y-auto whitespace-pre-wrap rounded border border-border bg-accent px-2 py-1.5 text-xs text-foreground"
            >
              {{ artifactOf(item.msg.artifact_id)?.content ?? "" }}
            </div>
            <p
              v-else-if="openArtifacts.has(item.msg.artifact_id) && artifactErrors[item.msg.artifact_id]"
              class="mt-1 text-[10px] text-destructive"
            >
              {{ t("chat.artifactLoadFailed") }}
            </p>
          </div>
          <!-- 決策徽章（本 session 已核可／拒絕）-->
          <div
            v-if="item.msg.direction === 'out' && item.msg.proposed_commitment_id && decided.has(item.msg.proposed_commitment_id)"
            class="mt-1 px-1 text-[10px]"
            :class="decided.get(item.msg.proposed_commitment_id!) === 'approved' ? 'text-emerald-600' : 'text-muted-foreground'"
          >
            {{ decided.get(item.msg.proposed_commitment_id!) === "approved" ? "✓ " + t("approval.approved") : "✗ " + t("approval.rejected") }}
          </div>
          <!-- 提案核可鈕（僅 Out message 帶待核可提案、且尚未決策時顯示） -->
          <div
            v-else-if="item.msg.direction === 'out' && item.msg.proposed_commitment_id && proposedIds.has(item.msg.proposed_commitment_id)"
            class="mt-1"
          >
            <!-- R5（Ch.20 §5.4）：為何進到人類通道——讓賭注可見（核可卡原因行）-->
            <div v-if="whyPending(item.msg.proposed_commitment_id!)" class="mb-1 px-1 text-[10px] text-muted-foreground">
              {{ t("approval.whyPending") }}：{{ whyPending(item.msg.proposed_commitment_id!) }}
            </div>
            <div class="flex gap-2">
              <button
                class="flex items-center gap-1 rounded bg-emerald-600 px-2.5 py-1 text-xs text-white hover:opacity-90 disabled:opacity-50"
                :disabled="pending !== null"
                @click="approve(item.msg.proposed_commitment_id!)"
              >
                <Loader2 v-if="pending === item.msg.proposed_commitment_id" :size="12" class="animate-spin" />
                ✓ {{ t("approval.approve") }}
              </button>
              <button
                class="flex items-center gap-1 rounded border border-border px-2.5 py-1 text-xs hover:bg-accent disabled:opacity-50"
                :disabled="pending !== null"
                @click="reject(item.msg.proposed_commitment_id!)"
              >
                <Loader2 v-if="pending === item.msg.proposed_commitment_id" :size="12" class="animate-spin" />
                ✗ {{ t("approval.reject") }}
              </button>
            </div>
          </div>
        </div>
      </template>
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

