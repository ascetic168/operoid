<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  Users,
  Building2,
  CalendarDays,
  Inbox,
  Loader2,
  CheckCircle2,
  AlertTriangle,
  Save,
  RefreshCw,
  FileDown,
  Plus,
  Wand2,
  Lightbulb,
  Target,
  X,
} from "lucide-vue-next";
import {
  factoryRun,
  factoryWritePages,
  factorySaveAuthored,
  factoryClassify,
  brainSync,
  formatError,
  tL10n,
  type Factory,
  type PreviewPage,
  type PreviewResult,
  type WriteResult,
  type AuthoredResult,
} from "@/lib/tauri";
import { useBrainsStore } from "@/stores/brains";

const { t } = useI18n();

interface FactoryDef {
  id: Factory;
  icon: typeof Users;
  titleKey: string;
  acceptKey: string;
  targetKey: string;
}

const factories: FactoryDef[] = [
  { id: "people", icon: Users, titleKey: "factories.defs.people.title", acceptKey: "factories.defs.people.accept", targetKey: "factories.defs.people.target" },
  { id: "companies", icon: Building2, titleKey: "factories.defs.companies.title", acceptKey: "factories.defs.companies.accept", targetKey: "factories.defs.companies.target" },
  { id: "meeting", icon: CalendarDays, titleKey: "factories.defs.meeting.title", acceptKey: "factories.defs.meeting.accept", targetKey: "factories.defs.meeting.target" },
  { id: "projects", icon: Target, titleKey: "factories.defs.projects.title", acceptKey: "factories.defs.projects.accept", targetKey: "factories.defs.projects.target" },
  { id: "concepts", icon: Lightbulb, titleKey: "factories.defs.concepts.title", acceptKey: "factories.defs.concepts.accept", targetKey: "factories.defs.concepts.target" },
  { id: "inbox", icon: Inbox, titleKey: "factories.defs.inbox.title", acceptKey: "factories.defs.inbox.accept", targetKey: "factories.defs.inbox.target" },
];

// 點選擇器的副檔名過濾(每個工廠不同)。名稱為檔案類型標籤，語言中立，不譯。
const FILTERS: Record<Factory, { name: string; extensions: string[] }[]> = {
  people: [{ name: "CSV / Text / Markdown", extensions: ["csv", "txt", "md"] }],
  companies: [{ name: "Text / PDF", extensions: ["txt", "pdf"] }],
  meeting: [{ name: "Text / Markdown / PDF", extensions: ["txt", "md", "pdf"] }],
  projects: [{ name: "Text / Markdown / PDF", extensions: ["txt", "md", "pdf"] }],
  concepts: [{ name: "Text / Markdown / PDF", extensions: ["txt", "md", "pdf"] }],
  inbox: [{ name: "Text / Markdown", extensions: ["txt", "md"] }],
};

// 自動分類入口接受所有目前工廠支援的副檔名。
const AUTO_FILTER = [{ name: "CSV / Text / Markdown / PDF", extensions: ["csv", "txt", "md", "pdf"] }];
const factoryOptions: Factory[] = ["people", "companies", "meeting", "projects", "concepts", "inbox"];

const cardEls = new Map<string, HTMLElement>();
function setCardRef(id: string, el: Element | null) {
  if (el instanceof HTMLElement) cardEls.set(id, el);
  else cardEls.delete(id);
}

const hovered = ref<string | null>(null);
const busy = ref<string | null>(null);
const preview = ref<PreviewResult | null>(null);
const errorMsg = ref<string | null>(null);

const selectedSample = ref(0);
const selectedFile = ref<number | null>(null); // >1 檔:null=清單, 數字=預覽某檔
const editedMd = ref("");
const overwriteRes = ref<WriteResult | null>(null);

// 當前預覽的頁陣列:多檔模式=選中檔的 pages;否則=sample。
const currentPages = computed<PreviewPage[]>(() => {
  const p = preview.value;
  if (!p) return [];
  if (p.files.length > 1 && selectedFile.value !== null) {
    return p.files[selectedFile.value]?.pages ?? [];
  }
  return p.sample;
});

const syncLog = ref<string[]>([]);
const syncRunning = ref(false);
const syncDone = ref(false);

// 編輯器("+ 新增")狀態
const editorOpen = ref(false);
const editorFactory = ref<Factory>("meeting");
const editorMd = ref("");
const editorSlug = ref<string | null>(null);
const editorResult = ref<AuthoredResult | null>(null);
const editorError = ref<string | null>(null);
const editorBusy = ref(false);

// 自動分類確認框狀態
interface ClassifyItem {
  path: string;
  chosen: string;
  reason: string;
}
const classifyOpen = ref(false);
const classifyItems = ref<ClassifyItem[]>([]);
const classifyBusy = ref(false);

// 來源感知：作用中腦的來源清單 + 選定來源 → targetRepo / sync 對象
const brains = useBrainsStore();
const targetRepo = computed<string | null>(() => {
  if (!brains.activeSourceId) return null;
  const s = brains.sources.find((x) => x.id === brains.activeSourceId);
  return s ? s.local_path : null;
});
const activeBrainName = computed(() => brains.brains.find((b) => b.id === brains.activeId)?.name ?? null);
async function pickSource(sid: string) {
  await brains.setActiveSource(sid || null);
}

function factoryAt(x: number, y: number): string | null {
  const dpr = window.devicePixelRatio || 1;
  const cx = x / dpr;
  const cy = y / dpr;
  for (const [id, el] of cardEls) {
    const r = el.getBoundingClientRect();
    if (cx >= r.left && cx <= r.right && cy >= r.top && cy <= r.bottom) return id;
  }
  return null;
}

let unlisten: (() => void) | null = null;
onMounted(async () => {
  brains.load(); // 載入腦清單 + 作用中腦的來源（供來源選擇器）
  const webview = getCurrentWebview();
  unlisten = await webview.onDragDropEvent((event) => {
    if (event.payload.type === "drop") {
      const target = hovered.value;
      if (target === "auto") onAuto(event.payload.paths);
      else if (target) doRun(target, event.payload.paths);
    } else if (event.payload.type === "leave") {
      hovered.value = null;
    } else {
      hovered.value = factoryAt(event.payload.position.x, event.payload.position.y);
    }
  });
});
onUnmounted(() => unlisten?.());

// 點拖放區 → 原生檔案選擇器
async function pickFiles(f: Factory) {
  try {
    const selected = await openDialog({ multiple: true, filters: FILTERS[f] });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length) doRun(f, paths);
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}

// ── 自動分類（統一入口）──────────────────────────────────────────────
// 點自動卡 → 選檔 → 分類；高/中信心直接跑，低信心跳確認框。
async function onAutoPick() {
  try {
    const selected = await openDialog({ multiple: true, filters: AUTO_FILTER });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length) onAuto(paths);
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}

async function onAuto(paths: string[]) {
  busy.value = "auto";
  preview.value = null;
  overwriteRes.value = null;
  errorMsg.value = null;
  syncLog.value = [];
  syncDone.value = false;
  selectedFile.value = null;
  selectedSample.value = 0;
  try {
    const cls = await factoryClassify(paths);
    const skipped = cls.filter((c) => !c.factory); // 不支援的副檔名
    const autoCls = cls.filter((c) => c.factory && c.confidence !== "low"); // 高/中 → 自動跑
    const confirmCls = cls.filter((c) => c.factory && c.confidence === "low"); // 低 → 確認

    // 依判定的 factory 分組，逐群呼叫既有 factory_run。
    const groups: Record<string, string[]> = {};
    for (const c of autoCls) (groups[c.factory] ??= []).push(c.path);
    const parts = await runGroups(groups);

    preview.value = parts.length || skipped.length ? mergePreview(parts, skipped.length) : null;

    if (confirmCls.length) {
      classifyItems.value = confirmCls.map((c) => ({
        path: c.path,
        chosen: c.factory || "inbox",
        reason: c.reason,
      }));
      classifyOpen.value = true;
    }
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    busy.value = null;
  }
}

/** 依分組跑各工廠；單群失敗不中斷其餘，轉成含 errors 的結果。 */
async function runGroups(groups: Record<string, string[]>): Promise<PreviewResult[]> {
  const results: PreviewResult[] = [];
  for (const [f, ps] of Object.entries(groups)) {
    try {
      results.push(await factoryRun(f as Factory, ps, targetRepo.value));
    } catch (e) {
      const detail = formatError(e);
      const err = { code: "factory.fileError", params: { file: f, detail } };
      results.push({ factory: f, summary: err, sample: [], total: 0, written: [], errors: [err], files: [] });
    }
  }
  return results;
}

/** 把多個工廠的 PreviewResult 合併成一個（factory="auto"）餵給既有預覽面板。 */
function mergePreview(parts: PreviewResult[], skippedCount = 0): PreviewResult {
  const sample = parts.flatMap((p) => p.sample).slice(0, 10);
  const files = parts.flatMap((p) => p.files);
  const written = parts.flatMap((p) => p.written);
  const errors = parts.flatMap((p) => p.errors);
  const total = parts.reduce((n, p) => n + p.total, 0);
  const code = skippedCount > 0 ? "factories.classify.autoSummarySkipped" : "factories.classify.autoSummary";
  return {
    factory: "auto",
    summary: { code, params: { n: String(written.length), skipped: String(skippedCount) } },
    sample,
    total,
    written,
    errors,
    files,
  };
}

/** 確認框：依使用者選定的工廠分組跑，結果併入現有預覽。 */
async function confirmClassify() {
  classifyOpen.value = false;
  if (!classifyItems.value.length) return;
  classifyBusy.value = true;
  busy.value = "auto";
  try {
    const groups: Record<string, string[]> = {};
    for (const it of classifyItems.value) (groups[it.chosen] ??= []).push(it.path);
    const parts = await runGroups(groups);
    if (parts.length) {
      preview.value = preview.value ? mergePreview([preview.value, ...parts]) : mergePreview(parts);
    }
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    classifyBusy.value = false;
    busy.value = null;
    classifyItems.value = [];
  }
}

async function doRun(factoryId: string, paths: string[]) {
  busy.value = factoryId;
  preview.value = null;
  overwriteRes.value = null;
  errorMsg.value = null;
  syncLog.value = [];
  syncDone.value = false;
  selectedSample.value = 0;
  selectedFile.value = null;
  try {
    preview.value = await factoryRun(factoryId as Factory, paths, targetRepo.value);
    syncEditedFromSample();
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    busy.value = null;
  }
}

function syncEditedFromSample() {
  const s = currentPages.value[selectedSample.value];
  editedMd.value = s ? s.markdown : "";
}

watch(selectedSample, syncEditedFromSample);

async function doOverwrite() {
  const s = currentPages.value[selectedSample.value];
  if (!s) return;
  busy.value = "overwrite";
  try {
    overwriteRes.value = await factoryWritePages(
      [{ slug: s.slug, target_dir: s.target_dir, markdown: editedMd.value }],
      targetRepo.value,
    );
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    busy.value = null;
  }
}

async function doSync() {
  if (!brains.activeId) return;
  if (!brains.activeSourceId) {
    syncLog.value = [t("factories.syncNeedSource")];
    return;
  }
  syncRunning.value = true;
  syncLog.value = [t("factories.syncStart", { id: brains.activeSourceId })];
  try {
    const res = await brainSync(
      brains.activeId,
      "one",
      brains.activeSourceId,
      (l) => syncLog.value.push(`[${l.stream}] ${l.text}`),
    );
    syncLog.value.push(res.success ? t("factories.syncDoneOk") : t("factories.syncDoneFail", { code: res.exit_code ?? "?" }));
    syncDone.value = res.success;
  } catch (e) {
    syncLog.value.push(t("factories.syncErr", { e: formatError(e) }));
  } finally {
    syncRunning.value = false;
  }
}

// "+ 新增"編輯器
function openEditor(f: Factory) {
  editorFactory.value = f;
  editorMd.value = t(`factories.templates.${f}`);
  editorSlug.value = null;
  editorResult.value = null;
  editorError.value = null;
  editorOpen.value = true;
}

async function saveEditor() {
  editorError.value = null;
  editorBusy.value = true;
  try {
    const res = await factorySaveAuthored(
      editorFactory.value,
      editorMd.value,
      editorSlug.value,
      targetRepo.value,
    );
    editorSlug.value = res.slug; // 之後存檔覆蓋同檔
    editorMd.value = res.enriched_markdown; // 反映 LLM 補的 wikilink
    editorResult.value = res;
  } catch (e) {
    editorError.value = formatError(e);
  } finally {
    editorBusy.value = false;
  }
}

async function saveEditorAndSync() {
  await saveEditor();
  if (editorResult.value) {
    editorOpen.value = false;
    await doSync();
  }
}
</script>

<template>
  <div class="flex h-full flex-col overflow-y-auto p-6">
    <header class="mb-3">
      <h1 class="text-xl font-semibold">{{ $t("factories.title") }}</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        {{ $t("factories.desc") }}
      </p>
    </header>

    <!-- 來源選擇：作用中腦 / 作用中來源（工廠寫入與 sync 的目標） -->
    <div class="mb-3 flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card/30 px-4 py-2.5 text-sm">
      <span class="text-muted-foreground">{{ $t("factories.activeBrain") }}</span>
      <code class="font-semibold">{{ activeBrainName ?? $t("common.dash") }}</code>
      <span class="text-muted-foreground">{{ $t("factories.sourceSep") }}</span>
      <select
        v-if="brains.sources.length"
        class="rounded border border-border bg-background px-2 py-1 text-xs"
        :value="brains.activeSourceId ?? ''"
        @change="pickSource(($event.target as HTMLSelectElement).value)"
      >
        <option value="">{{ $t("factories.sourceNone") }}</option>
        <option v-for="s in brains.sources" :key="s.id" :value="s.id">{{ s.id }} — {{ s.local_path }}</option>
      </select>
      <span v-else class="text-xs text-warning">{{ $t("factories.noSource") }}</span>
    </div>

    <!-- 工廠卡（第一張為自動分類統一入口；其餘各對應一個 DIR_PATTERN 白名單目錄）-->
    <div class="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4">
      <!-- 自動分類：丟任何 csv/txt/md/pdf，程式判斷歸屬 -->
      <div
        :ref="(el) => setCardRef('auto', el as Element | null)"
        :class="[
          'col-span-2 flex flex-col gap-2 rounded-xl border-2 p-4 transition-colors',
          hovered === 'auto' ? 'border-primary bg-primary/10' : 'border-primary/40 bg-primary/5',
        ]"
      >
        <div class="flex items-center gap-3">
          <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/15">
            <component :is="busy === 'auto' ? Loader2 : Wand2" :size="18" :class="busy === 'auto' ? 'animate-spin' : ''" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="font-medium">{{ $t("factories.defs.auto.title") }}</div>
            <div class="text-xs text-muted-foreground">{{ $t("factories.accept") }}{{ $t("factories.defs.auto.accept") }}</div>
          </div>
        </div>
        <button
          type="button"
          class="flex flex-1 cursor-pointer items-center justify-center gap-2 rounded-lg border border-dashed border-primary/40 px-3 py-2.5 text-sm transition-colors hover:bg-primary/10"
          @click="onAutoPick"
        >
          <component :is="busy === 'auto' ? Loader2 : FileDown" :size="16" :class="busy === 'auto' ? 'animate-spin' : ''" />
          <span class="truncate">{{ hovered === 'auto' ? $t("factories.dropActive") : $t("factories.defs.auto.hint") }}</span>
        </button>
        <div class="text-xs text-muted-foreground">{{ $t("factories.output") }} <code>{{ $t("factories.defs.auto.target") }}</code></div>
      </div>

      <div
        v-for="f in factories"
        :key="f.id"
        :ref="(el) => setCardRef(f.id, el as Element | null)"
        :class="[
          'flex flex-col gap-2 rounded-xl border-2 p-4 transition-colors',
          hovered === f.id ? 'border-primary bg-primary/5' : 'border-dashed border-border bg-card/40',
        ]"
      >
        <div class="flex items-center gap-3">
          <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-accent">
            <component :is="busy === f.id ? Loader2 : f.icon" :size="18" :class="busy === f.id ? 'animate-spin' : ''" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="font-medium">
              {{ $t(f.titleKey) }}
              <span
                v-if="f.id === 'inbox'"
                :title="$t('factories.defs.inbox.noGraphHint')"
                class="ml-1 rounded bg-warning/15 px-1.5 py-0.5 align-middle text-[10px] font-normal text-warning"
                >{{ $t("factories.defs.inbox.noGraph") }}</span
              >
            </div>
            <div class="text-xs text-muted-foreground">{{ $t("factories.accept") }}{{ $t(f.acceptKey) }}</div>
          </div>
          <button
            :title="$t('factories.addTooltip', { title: $t(f.titleKey) })"
            class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-border text-muted-foreground hover:bg-accent hover:text-foreground"
            @click="openEditor(f.id)"
          >
            <Plus :size="16" />
          </button>
        </div>
        <button
          type="button"
          class="flex flex-1 cursor-pointer flex-col items-center justify-center gap-1 rounded-lg border border-dashed border-border/60 py-3 text-center text-sm transition-colors hover:border-primary/50 hover:bg-accent/30"
          :class="hovered === f.id ? 'text-foreground' : 'text-muted-foreground'"
          @click="pickFiles(f.id)"
        >
          <component :is="busy === f.id ? Loader2 : FileDown" :size="18" :class="busy === f.id ? 'animate-spin' : ''" />
          <span>{{ hovered === f.id ? $t("factories.dropActive") : $t("factories.dropHint") }}</span>
        </button>
        <div class="text-xs text-muted-foreground">{{ $t("factories.output") }} <code>{{ $t(f.targetKey) }}</code></div>
      </div>
    </div>

    <!-- 預覽 / 結果 -->
    <section v-if="busy || preview || errorMsg" class="mt-6 rounded-xl border border-border bg-card/40 p-5">
      <div v-if="errorMsg" class="flex items-center gap-2 text-sm text-destructive">
        <AlertTriangle :size="16" /> {{ errorMsg }}
      </div>

      <template v-else-if="preview">
        <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-2 text-sm">
            <CheckCircle2 :size="16" class="text-green-500" />
            <span class="font-medium">{{ tL10n(preview.summary) }}</span>
          </div>
          <button
            :disabled="syncRunning"
            class="flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent/50 disabled:opacity-50"
            @click="doSync"
          >
            <RefreshCw :size="14" :class="syncRunning ? 'animate-spin' : ''" />
            {{ syncRunning ? $t("factories.syncing") : syncDone ? $t("factories.resync") : $t("factories.syncToBrain") }}
          </button>
        </div>

        <div v-if="preview.errors.length" class="mb-3 rounded-md bg-destructive/10 p-2 text-xs text-destructive">
          <div v-for="(e, i) in preview.errors" :key="i">{{ tL10n(e) }}</div>
        </div>

        <!-- 多檔批次:檔案清單(點選進入單檔預覽) -->
        <div v-if="preview.files.length > 1 && selectedFile === null" class="mb-2">
          <div class="mb-2 text-xs text-muted-foreground">{{ $t("factories.filesProcessed") }}</div>
          <div class="divide-y divide-border rounded-md border border-border">
            <button
              v-for="(f, i) in preview.files"
              :key="i"
              type="button"
              class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs hover:bg-accent/50"
              @click="selectedFile = i; selectedSample = 0; syncEditedFromSample()"
            >
              <component :is="f.ok ? CheckCircle2 : AlertTriangle" :size="14" :class="f.ok ? 'text-green-500' : 'text-destructive'" />
              <span class="flex-1 truncate font-mono">{{ f.path }}</span>
              <span class="text-muted-foreground">{{ $t("factories.pagesN", { n: f.pages.length }) }}</span>
            </button>
          </div>
        </div>

        <!-- 單檔預覽/編輯;或多檔選了某檔後 -->
        <template v-else>
          <div v-if="preview.files.length > 1" class="mb-2 flex items-center gap-2">
            <button type="button" class="text-xs text-muted-foreground hover:text-foreground" @click="selectedFile = null">
              ← {{ $t("factories.backToList") }}
            </button>
            <span class="truncate font-mono text-xs text-muted-foreground">{{ preview.files[selectedFile ?? 0]?.path }}</span>
          </div>

          <div v-if="currentPages.length > 1" class="mb-2 flex items-center gap-2">
            <span class="text-xs text-muted-foreground">{{ $t("factories.previewN") }}</span>
            <select v-model.number="selectedSample" class="rounded border border-border bg-background px-2 py-1 text-xs">
              <option v-for="(s, i) in currentPages" :key="i" :value="i">{{ s.slug }}</option>
            </select>
            <span class="text-xs text-muted-foreground">{{ $t("factories.pageEditable") }}</span>
          </div>

          <div v-if="currentPages.length" class="mb-2 flex items-center justify-between">
            <span class="text-xs text-muted-foreground">
              <code>{{ currentPages[selectedSample]?.target_dir }}/{{ currentPages[selectedSample]?.slug }}.md</code>
            </span>
            <button
              :disabled="busy !== null"
              class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
              @click="doOverwrite"
            >
              <Save :size="14" /> {{ $t("factories.overwrite") }}
            </button>
          </div>
          <textarea
            v-if="currentPages.length"
            v-model="editedMd"
            spellcheck="false"
            class="h-80 w-full resize-y rounded-md border border-border bg-background p-3 font-mono text-xs leading-relaxed"
          />
        </template>
        <div v-if="overwriteRes" class="mt-2 flex items-center gap-2 text-xs text-green-500">
          <CheckCircle2 :size="13" /> {{ $t("factories.overwrittenN", { n: overwriteRes.written.length }) }}
        </div>

        <pre v-if="syncLog.length" class="mt-3 max-h-40 overflow-auto rounded-md border border-border bg-background p-2 text-xs">{{ syncLog.join("\n") }}</pre>
      </template>

      <div v-else class="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 :size="16" class="animate-spin" /> {{ $t("factories.converting") }}
      </div>
    </section>

    <div v-else class="mt-6 flex items-center gap-2 text-sm text-muted-foreground">
      <FileDown :size="16" /> {{ $t("factories.empty") }}
    </div>

    <!-- "+ 新增" 編輯器彈窗 -->
    <div v-if="editorOpen" class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4" @click.self="editorOpen = false">
      <div class="flex max-h-[88vh] w-full max-w-3xl flex-col rounded-xl border border-border bg-card shadow-2xl">
        <div class="flex items-center justify-between border-b border-border px-5 py-3">
          <div class="flex items-center gap-2">
            <Plus :size="16" />
            <span class="font-medium">{{ $t("factories.editorTitle", { factory: editorFactory }) }}</span>
          </div>
          <button class="text-muted-foreground hover:text-foreground" @click="editorOpen = false">
            <X :size="18" />
          </button>
        </div>

        <div class="border-b border-border px-5 py-2 text-xs text-muted-foreground">
          <span v-if="editorResult">
            {{ $t("factories.editorWritten") }}<code>{{ editorResult.target_dir }}/{{ editorResult.slug }}.md</code>
            <span v-if="editorResult.enriched" class="ml-2 text-green-500">
              {{ $t("factories.editorEnriched", { n: editorResult.names_count }) }}
            </span>
            <span v-else class="ml-2 text-warning">{{ $t("factories.editorNotEnriched") }}</span>
            <span v-if="editorResult.used_fallback" class="ml-2 text-warning">{{ $t("factories.editorFallback") }}</span>
          </span>
          <span v-else>{{ $t("factories.editorHint") }}</span>
        </div>

        <textarea
          v-model="editorMd"
          spellcheck="false"
          class="min-h-[40vh] flex-1 resize-y bg-background p-4 font-mono text-xs leading-relaxed outline-none"
        />

        <div v-if="editorError" class="px-5 py-2 text-xs text-destructive">{{ editorError }}</div>

        <div class="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
          <button class="rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent" @click="editorOpen = false">
            {{ $t("common.close") }}
          </button>
          <button
            :disabled="editorBusy"
            class="flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent/50 disabled:opacity-50"
            @click="saveEditor"
          >
            <component :is="editorBusy ? Loader2 : Save" :size="14" :class="editorBusy ? 'animate-spin' : ''" />
            {{ editorBusy ? $t("factories.editorEnriching") : $t("factories.editorSave", { mode: editorSlug ? $t("factories.editorSaveOverwrite") : $t("factories.editorSaveNew") }) }}
          </button>
          <button
            :disabled="editorBusy"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="saveEditorAndSync"
          >
            <RefreshCw :size="14" /> {{ $t("factories.editorSaveAndSync") }}
          </button>
        </div>
      </div>
    </div>

    <!-- 自動分類確認框（低信心檔案） -->
    <div v-if="classifyOpen" class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4" @click.self="classifyOpen = false">
      <div class="flex max-h-[80vh] w-full max-w-2xl flex-col rounded-xl border border-border bg-card shadow-2xl">
        <div class="flex items-center justify-between border-b border-border px-5 py-3">
          <div class="flex items-center gap-2">
            <Wand2 :size="16" />
            <span class="font-medium">{{ $t("factories.classify.confirmTitle") }}</span>
          </div>
          <button class="text-muted-foreground hover:text-foreground" @click="classifyOpen = false">
            <X :size="18" />
          </button>
        </div>
        <div class="border-b border-border px-5 py-2 text-xs text-muted-foreground">{{ $t("factories.classify.confirmHint") }}</div>
        <div class="flex-1 overflow-auto px-5 py-3">
          <div v-for="(it, i) in classifyItems" :key="i" class="mb-2 flex items-center gap-3 rounded-md border border-border p-2">
            <span class="min-w-0 flex-1">
              <span class="block truncate font-mono text-xs">{{ it.path }}</span>
              <span class="block text-xs text-muted-foreground">{{ it.reason }}</span>
            </span>
            <select v-model="it.chosen" class="rounded border border-border bg-background px-2 py-1 text-xs">
              <option v-for="fo in factoryOptions" :key="fo" :value="fo">{{ fo }}</option>
            </select>
          </div>
        </div>
        <div class="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
          <button class="rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent" @click="classifyOpen = false">
            {{ $t("common.close") }}
          </button>
          <button
            :disabled="classifyBusy"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="confirmClassify"
          >
            <component :is="classifyBusy ? Loader2 : CheckCircle2" :size="14" :class="classifyBusy ? 'animate-spin' : ''" />
            {{ $t("factories.classify.confirmRun") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
