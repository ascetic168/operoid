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
  X,
} from "lucide-vue-next";
import {
  factoryRun,
  factoryWritePages,
  factorySaveAuthored,
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
  { id: "inbox", icon: Inbox, titleKey: "factories.defs.inbox.title", acceptKey: "factories.defs.inbox.accept", targetKey: "factories.defs.inbox.target" },
];

// 點選擇器的副檔名過濾(每個工廠不同)。名稱為檔案類型標籤，語言中立，不譯。
const FILTERS: Record<Factory, { name: string; extensions: string[] }[]> = {
  people: [{ name: "CSV / Text / Markdown", extensions: ["csv", "txt", "md"] }],
  companies: [{ name: "Text / PDF", extensions: ["txt", "pdf"] }],
  meeting: [{ name: "Text / Markdown / PDF", extensions: ["txt", "md", "pdf"] }],
  inbox: [{ name: "Text / Markdown", extensions: ["txt", "md"] }],
};

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
      if (target) doRun(target, event.payload.paths);
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
    <header class="mb-5">
      <h1 class="text-xl font-semibold">{{ $t("factories.title") }}</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        {{ $t("factories.desc") }}
      </p>
    </header>

    <!-- 來源選擇：作用中腦 / 作用中來源（工廠寫入與 sync 的目標） -->
    <div class="mb-5 flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card/30 px-4 py-2.5 text-sm">
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

    <!-- 工廠卡 -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
      <div
        v-for="f in factories"
        :key="f.id"
        :ref="(el) => setCardRef(f.id, el as Element | null)"
        :class="[
          'flex flex-col gap-3 rounded-xl border-2 p-5 transition-colors',
          hovered === f.id ? 'border-primary bg-primary/5' : 'border-dashed border-border bg-card/40',
        ]"
      >
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent">
            <component :is="busy === f.id ? Loader2 : f.icon" :size="20" :class="busy === f.id ? 'animate-spin' : ''" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="font-medium">{{ $t(f.titleKey) }}</div>
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
          class="flex flex-1 cursor-pointer flex-col items-center justify-center gap-1 rounded-lg border border-dashed border-border/60 py-8 text-center text-sm transition-colors hover:border-primary/50 hover:bg-accent/30"
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
  </div>
</template>
