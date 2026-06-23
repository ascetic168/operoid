<script setup lang="ts">
import { nextTick, ref } from "vue";
import {
  BarChart3,
  RefreshCw,
  Link2,
  Search,
  Brain,
  Stethoscope,
  Network,
  HardDrive,
  Trash2,
  Loader2,
  Building2,
} from "lucide-vue-next";
import { runOp, extractCompaniesRun, type CliLine, type OpName } from "@/lib/tauri";

interface LogEntry {
  stream: string;
  text: string;
}

const log = ref<LogEntry[]>([]);
const running = ref<string | null>(null);
const query = ref(""); // ask / think 共用
const slug = ref(""); // graph-query
const cleanCompanies = ref(false);
const consoleEl = ref<HTMLElement | null>(null);

const ops: { id: OpName; icon: typeof BarChart3; title: string; desc: string; needsArg?: "query" | "slug" }[] = [
  { id: "stats", icon: BarChart3, title: "stat", desc: "gbrain stats" },
  { id: "sync", icon: RefreshCw, title: "sync", desc: "commit + sync + embed + extract" },
  { id: "extract", icon: Link2, title: "extract", desc: "extract --stale" },
  { id: "ask", icon: Search, title: "ask", desc: "混合檢索", needsArg: "query" },
  { id: "think", icon: Brain, title: "think", desc: "多跳合成（可選 anchor:slug 換行）", needsArg: "query" },
];

const diagnostics: { id: OpName; icon: typeof BarChart3; title: string; desc: string; needsArg?: "query" | "slug" }[] = [
  { id: "doctor", icon: Stethoscope, title: "doctor", desc: "健康檢查" },
  { id: "orphans", icon: Network, title: "orphans", desc: "無入邊頁" },
  { id: "storage", icon: HardDrive, title: "storage", desc: "層狀態" },
  { id: "graph-query", icon: Network, title: "graph-query", desc: "看實體邊", needsArg: "slug" },
];

async function push(line: LogEntry) {
  log.value.push(line);
  await nextTick();
  if (consoleEl.value) consoleEl.value.scrollTop = consoleEl.value.scrollHeight;
}

async function run(id: OpName, needsArg?: "query" | "slug") {
  let arg: string | null = null;
  if (needsArg === "query") {
    if (!query.value.trim()) return;
    arg = query.value.trim();
  } else if (needsArg === "slug") {
    if (!slug.value.trim()) return;
    arg = slug.value.trim();
  }
  running.value = id;
  await push({ stream: "step", text: `── ${id} ──` });
  try {
    const res = await runOp(id, arg, (l: CliLine) => push({ stream: l.stream, text: l.text }));
    await push({
      stream: res.success ? "step" : "stderr",
      text: res.note
        ? `${res.success ? "✓" : "✗"} ${res.note}`
        : `${res.success ? "✓ 完成" : "✗ 失敗"}（exit ${res.exit_code ?? "?"}）`,
    });
  } catch (e) {
    await push({ stream: "stderr", text: `錯誤：${e}` });
  } finally {
    running.value = null;
  }
}

function clearLog() {
  log.value = [];
}

async function rebuildCompanies() {
  running.value = "companies-extract";
  await push({ stream: "step", text: "── 重建 companies（從 people 掃 公司/組織 bullet）──" });
  try {
    const res = await extractCompaniesRun(cleanCompanies.value);
    await push({ stream: "step", text: `✓ 寫入 ${res.written.length} 個公司頁` });
    if (res.note) await push({ stream: "stdout", text: res.note });
    for (const e of res.errors) await push({ stream: "stderr", text: e });
  } catch (e) {
    await push({ stream: "stderr", text: `錯誤：${e}` });
  } finally {
    running.value = null;
  }
}
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden p-6">
    <header class="mb-4">
      <h1 class="text-xl font-semibold">操作 — GBrain CLI</h1>
      <p class="mt-1 text-sm text-muted-foreground">包裝 gbrain 指令並串流輸出。</p>
    </header>

    <!-- 主操作 -->
    <div class="mb-3 grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">
      <button
        v-for="op in ops"
        :key="op.id"
        :disabled="running !== null"
        class="flex flex-col items-start gap-1 rounded-lg border border-border bg-card/40 p-3 text-left transition-colors hover:bg-accent/50 disabled:opacity-50"
        @click="run(op.id, op.needsArg)"
      >
        <div class="flex items-center gap-2">
          <component :is="running === op.id ? Loader2 : op.icon" :size="16" :class="running === op.id ? 'animate-spin' : ''" />
          <span class="font-mono text-sm font-medium">{{ op.title }}</span>
        </div>
        <span class="text-[11px] text-muted-foreground">{{ op.desc }}</span>
      </button>
    </div>

    <!-- ask/think 輸入 -->
    <div class="mb-3 flex flex-wrap items-center gap-2">
      <input
        v-model="query"
        placeholder="ask / think 的問題（think 可用 anchor:slug 換行後接問題）"
        class="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        @keydown.enter="run('ask', 'query')"
      />
    </div>

    <!-- 診斷 -->
    <div class="mb-3 flex flex-wrap items-center gap-2">
      <button
        v-for="d in diagnostics"
        :key="d.id"
        :disabled="running !== null"
        class="flex items-center gap-1.5 rounded-md border border-border bg-card/40 px-3 py-1.5 text-xs transition-colors hover:bg-accent/50 disabled:opacity-50"
        @click="run(d.id, d.needsArg)"
      >
        <component :is="running === d.id ? Loader2 : d.icon" :size="14" :class="running === d.id ? 'animate-spin' : ''" />
        {{ d.title }}
      </button>
      <input
        v-model="slug"
        placeholder="slug（graph-query）"
        class="ml-2 w-48 rounded-md border border-border bg-background px-2 py-1.5 text-xs"
      />
      <button
        :disabled="running !== null"
        class="flex items-center gap-1.5 rounded-md border border-border bg-card/40 px-3 py-1.5 text-xs transition-colors hover:bg-accent/50 disabled:opacity-50"
        @click="rebuildCompanies"
      >
        <component :is="running === 'companies-extract' ? Loader2 : Building2" :size="14" :class="running === 'companies-extract' ? 'animate-spin' : ''" />
        重建 companies
      </button>
      <label class="flex items-center gap-1 text-xs text-muted-foreground">
        <input v-model="cleanCompanies" type="checkbox" /> --clean
      </label>
    </div>

    <!-- console -->
    <div class="flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-background">
      <div class="flex items-center justify-between border-b border-border px-3 py-1.5">
        <span class="text-xs text-muted-foreground">輸出</span>
        <button class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" @click="clearLog">
          <Trash2 :size="13" /> 清空
        </button>
      </div>
      <div ref="consoleEl" class="min-h-0 flex-1 overflow-y-auto p-3 font-mono text-xs leading-relaxed">
        <div v-if="log.length === 0" class="text-muted-foreground">尚無輸出。點上方按鈕執行操作。</div>
        <div
          v-for="(entry, i) in log"
          :key="i"
          :class="{
            'text-foreground': entry.stream === 'stdout',
            'text-warning': entry.stream === 'stderr',
            'text-sky-400': entry.stream === 'step',
          }"
        >
          {{ entry.text }}
        </div>
      </div>
    </div>
  </div>
</template>
