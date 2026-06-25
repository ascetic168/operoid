<script setup lang="ts">
import { nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
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
import {
  runOp,
  extractCompaniesRun,
  formatError,
  tL10n,
  openNote,
  type CliLine,
  type OpName,
} from "@/lib/tauri";

const { t } = useI18n();

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

const ops: { id: OpName; icon: typeof BarChart3; title: string; descKey: string; needsArg?: "query" | "slug" }[] = [
  { id: "stats", icon: BarChart3, title: "stat", descKey: "operations.ops.statsDesc" },
  { id: "sync", icon: RefreshCw, title: "sync", descKey: "operations.ops.syncDesc" },
  { id: "extract", icon: Link2, title: "extract", descKey: "operations.ops.extractDesc" },
  { id: "ask", icon: Search, title: "ask", descKey: "operations.ops.askDesc", needsArg: "query" },
  { id: "think", icon: Brain, title: "think", descKey: "operations.ops.thinkDesc", needsArg: "query" },
];

const diagnostics: { id: OpName; icon: typeof BarChart3; title: string; descKey: string; needsArg?: "query" | "slug" }[] = [
  { id: "doctor", icon: Stethoscope, title: "doctor", descKey: "operations.diagnostics.doctorDesc" },
  { id: "orphans", icon: Network, title: "orphans", descKey: "operations.diagnostics.orphansDesc" },
  { id: "storage", icon: HardDrive, title: "storage", descKey: "operations.diagnostics.storageDesc" },
  { id: "graph-query", icon: Network, title: "graph-query", descKey: "operations.diagnostics.graphQueryDesc", needsArg: "slug" },
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
  await push({ stream: "step", text: t("operations.stepOp", { op: id }) });
  try {
    const res = await runOp(id, arg, (l: CliLine) => push({ stream: l.stream, text: l.text }));
    const mark = res.success ? "✓" : "✗";
    await push({
      stream: res.success ? "step" : "stderr",
      text: res.note
        ? `${mark} ${tL10n(res.note)}`
        : t("operations.doneFallback", {
            mark,
            state: res.success ? t("operations.stateDone") : t("operations.stateFail"),
            code: res.exit_code ?? "?",
          }),
    });
  } catch (e) {
    await push({ stream: "stderr", text: t("operations.errLine", { e: formatError(e) }) });
  } finally {
    running.value = null;
  }
}

function clearLog() {
  log.value = [];
}

/** 一行輸出切成「純文字 / wikilink」段落，供模板分段渲染（不用 v-html，防 XSS）。
 *  支援兩種 gbrain 標籤：雙括 `[[dir/slug]]`/`[[dir/slug|name]]`（筆記內文），
 *  與單括 `[dir/slug]`（gbrain think/ask 輸出的引用格式）。單括須含 `/` 且後不接 `(`，
 *  以避開 markdown 連結 `[text](url)`。 */
interface LinkSeg {
  kind: "text" | "link";
  text: string;
  target?: string;
}
function linkSegments(text: string): LinkSeg[] {
  const segs: LinkSeg[] = [];
  const re = /\[\[([^\]]+)\]\]|\[([^\]\[\s|\/]+\/[^\]\[\s|\/]+)\](?!\()/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    const idx = m.index ?? 0;
    if (idx > last) segs.push({ kind: "text", text: text.slice(last, idx) });
    let target: string;
    let display: string;
    if (m[1] !== undefined) {
      // 雙括：inner 可能含 |name
      const inner = m[1];
      const pipe = inner.indexOf("|");
      target = (pipe >= 0 ? inner.slice(0, pipe) : inner).trim();
      const dispRaw = pipe >= 0 ? inner.slice(pipe + 1) : "";
      display = dispRaw.trim() || target.split("/").pop()?.trim() || target;
    } else {
      // 單括：[dir/slug]
      target = m[2].trim();
      display = target.split("/").pop()?.trim() || target;
    }
    if (target) segs.push({ kind: "link", text: display, target });
    else segs.push({ kind: "text", text: m[0] });
    last = idx + m[0].length;
  }
  if (last < text.length) segs.push({ kind: "text", text: text.slice(last) });
  return segs;
}

/** 點擊 wikilink → 後端轉 HTML 用預設瀏覽器開啟。 */
async function openLink(target: string) {
  try {
    const res = await openNote(target);
    await push({ stream: "step", text: t("note.opened", { title: res.title }) });
  } catch (e) {
    await push({ stream: "stderr", text: t("operations.errLine", { e: formatError(e) }) });
  }
}

async function rebuildCompanies() {
  running.value = "companies-extract";
  await push({ stream: "step", text: t("operations.stepRebuild") });
  try {
    const res = await extractCompaniesRun(cleanCompanies.value, null);
    await push({ stream: "step", text: t("operations.companiesWrittenN", { n: res.written.length }) });
    if (res.note) await push({ stream: "stdout", text: tL10n(res.note) });
    for (const e of res.errors) await push({ stream: "stderr", text: tL10n(e) });
  } catch (e) {
    await push({ stream: "stderr", text: t("operations.errLine", { e: formatError(e) }) });
  } finally {
    running.value = null;
  }
}
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden p-6">
    <header class="mb-4">
      <h1 class="text-xl font-semibold">{{ $t("operations.title") }}</h1>
      <p class="mt-1 text-sm text-muted-foreground">{{ $t("operations.desc") }}</p>
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
        <span class="text-[11px] text-muted-foreground">{{ $t(op.descKey) }}</span>
      </button>
    </div>

    <!-- ask/think 輸入 -->
    <div class="mb-3 flex flex-wrap items-center gap-2">
      <input
        v-model="query"
        :placeholder="$t('operations.askPlaceholder')"
        class="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-1.5 text-sm"
        @keydown.enter="run('think', 'query')"
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
        :placeholder="$t('operations.slugPlaceholder')"
        class="ml-2 w-48 rounded-md border border-border bg-background px-2 py-1.5 text-xs"
      />
      <button
        :disabled="running !== null"
        class="flex items-center gap-1.5 rounded-md border border-border bg-card/40 px-3 py-1.5 text-xs transition-colors hover:bg-accent/50 disabled:opacity-50"
        @click="rebuildCompanies"
      >
        <component :is="running === 'companies-extract' ? Loader2 : Building2" :size="14" :class="running === 'companies-extract' ? 'animate-spin' : ''" />
        {{ $t("operations.rebuildCompanies") }}
      </button>
      <label class="flex items-center gap-1 text-xs text-muted-foreground">
        <input v-model="cleanCompanies" type="checkbox" /> {{ $t("operations.cleanFlag") }}
      </label>
    </div>

    <!-- console -->
    <div class="flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-background">
      <div class="flex items-center justify-between border-b border-border px-3 py-1.5">
        <span class="text-xs text-muted-foreground">{{ $t("operations.output") }}</span>
        <button class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" @click="clearLog">
          <Trash2 :size="13" /> {{ $t("operations.clear") }}
        </button>
      </div>
      <div ref="consoleEl" class="min-h-0 flex-1 overflow-y-auto p-3 font-mono text-xs leading-relaxed">
        <div v-if="log.length === 0" class="text-muted-foreground">{{ $t("operations.empty") }}</div>
        <div
          v-for="(entry, i) in log"
          :key="i"
          :class="{
            'text-foreground': entry.stream === 'stdout',
            'text-warning': entry.stream === 'stderr',
            'text-sky-400': entry.stream === 'step',
          }"
        >
          <template v-for="(seg, j) in linkSegments(entry.text)" :key="j">
            <span
              v-if="seg.kind === 'link'"
              class="cursor-pointer font-medium text-sky-400 underline-offset-2 hover:underline"
              :title="seg.target"
              @click="openLink(seg.target!)"
              >{{ seg.text }}</span
            >
            <template v-else>{{ seg.text }}</template>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>
