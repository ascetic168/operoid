<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
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
  runOp,
  type Factory,
  type PreviewResult,
  type WriteResult,
  type AuthoredResult,
} from "@/lib/tauri";

interface FactoryDef {
  id: Factory;
  icon: typeof Users;
  title: string;
  accept: string;
  target: string;
}

const factories: FactoryDef[] = [
  { id: "people", icon: Users, title: "people 工廠", accept: "CSV（Google Contacts）", target: "people/" },
  { id: "companies", icon: Building2, title: "companies 工廠", accept: "txt / pdf", target: "companies/" },
  { id: "meeting", icon: CalendarDays, title: "meeting 工廠", accept: "txt / md / pdf", target: "meetings/" },
  { id: "inbox", icon: Inbox, title: "inbox（速記）", accept: "txt / md", target: "inbox/（gbrain capture）" },
];

// 點選擇器的副檔名過濾(每個工廠不同)。
const FILTERS: Record<Factory, { name: string; extensions: string[] }[]> = {
  people: [{ name: "CSV", extensions: ["csv"] }],
  companies: [{ name: "Text / PDF", extensions: ["txt", "pdf"] }],
  meeting: [{ name: "Text / Markdown / PDF", extensions: ["txt", "md", "pdf"] }],
  inbox: [{ name: "Text / Markdown", extensions: ["txt", "md"] }],
};

// "+ 新增"編輯器載入的合規 template(frontmatter + body + timeline 骨架)。
// 故意用「純文字」人名/公司名(不要 [[wikilink]])——存檔時 LLM 會自動補成 wikilink。
const TEMPLATES: Record<Factory, string> = {
  people:
    "---\ntype: person\ntitle: ''\ntags: [people, contact]\n---\n\n# \n\n任職於 晶瀚半導體，職稱 。\n\nEmail：\n- \n\nPhone：\n- \n\n<!-- timeline -->\n### YYYY-MM-DD — \n",
  companies:
    "---\ntype: company\ntitle: ''\ntags: [companies, contact]\n---\n\n# \n\n簡介：。\n\n成員：趙建宏、陳志遠、林家豪、王淑芬\n",
  meeting:
    "---\ntype: meeting\ntitle: ''\ntags: [meeting]\n---\n\n# \n\n日期：YYYY-MM-DD\n地點：\n\n與會者：趙建宏、陳志遠、林家豪、王淑芬\n\n討論：\n- \n\n決議：\n- \n\n<!-- timeline -->\n### YYYY-MM-DD — \n",
  inbox: "---\ntype: note\ntitle: ''\ntags: [note]\n---\n\n# \n\n",
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
const editedMd = ref("");
const overwriteRes = ref<WriteResult | null>(null);

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
    errorMsg.value = String(e);
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
  try {
    preview.value = await factoryRun(factoryId as Factory, paths);
    syncEditedFromSample();
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    busy.value = null;
  }
}

function syncEditedFromSample() {
  const s = preview.value?.sample[selectedSample.value];
  editedMd.value = s ? s.markdown : "";
}

watch(selectedSample, syncEditedFromSample);

async function doOverwrite() {
  const s = preview.value?.sample[selectedSample.value];
  if (!s) return;
  busy.value = "overwrite";
  try {
    overwriteRes.value = await factoryWritePages([
      { slug: s.slug, target_dir: s.target_dir, markdown: editedMd.value },
    ]);
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    busy.value = null;
  }
}

async function doSync() {
  syncRunning.value = true;
  syncLog.value = ["▶ sync 開始（commit + sync + embed --stale + extract --stale）"];
  try {
    const res = await runOp("sync", null, (l) => syncLog.value.push(`[${l.stream}] ${l.text}`));
    syncLog.value.push(res.success ? "✓ sync 完成" : `✗ sync 結束（exit ${res.exit_code ?? "?"}）`);
    syncDone.value = res.success;
  } catch (e) {
    syncLog.value.push(`✗ 錯誤：${e}`);
  } finally {
    syncRunning.value = false;
  }
}

// "+ 新增"編輯器
function openEditor(f: Factory) {
  editorFactory.value = f;
  editorMd.value = TEMPLATES[f];
  editorSlug.value = null;
  editorResult.value = null;
  editorError.value = null;
  editorOpen.value = true;
}

async function saveEditor() {
  editorError.value = null;
  editorBusy.value = true;
  try {
    const res = await factorySaveAuthored(editorFactory.value, editorMd.value, editorSlug.value);
    editorSlug.value = res.slug; // 之後存檔覆蓋同檔
    editorMd.value = res.enriched_markdown; // 反映 LLM 補的 wikilink
    editorResult.value = res;
  } catch (e) {
    editorError.value = String(e);
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
      <h1 class="text-xl font-semibold">工廠 — 拖放 / 點選 / 新增</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        拖放或點拖放區選檔即自動轉換並寫入；按 <code>+</code> 直接在編輯器以合規 template 新增。
      </p>
    </header>

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
            <div class="font-medium">{{ f.title }}</div>
            <div class="text-xs text-muted-foreground">接受：{{ f.accept }}</div>
          </div>
          <button
            :title="`新增 ${f.title}`"
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
          <span>{{ hovered === f.id ? "放開即轉換並寫入" : "拖放，或點此選擇檔案" }}</span>
        </button>
        <div class="text-xs text-muted-foreground">輸出 → <code>{{ f.target }}</code></div>
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
            <span class="font-medium">{{ preview.summary }}</span>
          </div>
          <button
            :disabled="syncRunning"
            class="flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent/50 disabled:opacity-50"
            @click="doSync"
          >
            <RefreshCw :size="14" :class="syncRunning ? 'animate-spin' : ''" />
            {{ syncRunning ? "同步中…" : syncDone ? "重新同步" : "Sync 到腦" }}
          </button>
        </div>

        <div v-if="preview.errors.length" class="mb-3 rounded-md bg-destructive/10 p-2 text-xs text-destructive">
          <div v-for="(e, i) in preview.errors" :key="i">{{ e }}</div>
        </div>

        <div v-if="preview.sample.length > 1" class="mb-2 flex items-center gap-2">
          <span class="text-xs text-muted-foreground">預覽第</span>
          <select v-model.number="selectedSample" class="rounded border border-border bg-background px-2 py-1 text-xs">
            <option v-for="(s, i) in preview.sample" :key="i" :value="i">{{ s.slug }}</option>
          </select>
          <span class="text-xs text-muted-foreground">頁（可編輯後覆蓋）</span>
        </div>

        <div v-if="preview.sample.length" class="mb-2 flex items-center justify-between">
          <span class="text-xs text-muted-foreground">
            <code>{{ preview.sample[selectedSample]?.target_dir }}/{{ preview.sample[selectedSample]?.slug }}.md</code>
          </span>
          <button
            :disabled="busy !== null"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="doOverwrite"
          >
            <Save :size="14" /> 覆蓋此頁
          </button>
        </div>
        <textarea
          v-if="preview.sample.length"
          v-model="editedMd"
          spellcheck="false"
          class="h-80 w-full resize-y rounded-md border border-border bg-background p-3 font-mono text-xs leading-relaxed"
        />
        <div v-if="overwriteRes" class="mt-2 flex items-center gap-2 text-xs text-green-500">
          <CheckCircle2 :size="13" /> 已覆蓋寫入 {{ overwriteRes.written.length }} 個檔案
        </div>

        <pre v-if="syncLog.length" class="mt-3 max-h-40 overflow-auto rounded-md border border-border bg-background p-2 text-xs">{{ syncLog.join("\n") }}</pre>
      </template>

      <div v-else class="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 :size="16" class="animate-spin" /> 轉換並寫入中（LLM 結構化可能需數秒）…
      </div>
    </section>

    <div v-else class="mt-6 flex items-center gap-2 text-sm text-muted-foreground">
      <FileDown :size="16" /> 拖放、點拖放區選檔，或按工廠右上 <code>+</code> 新增。
    </div>

    <!-- "+ 新增" 編輯器彈窗 -->
    <div v-if="editorOpen" class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4" @click.self="editorOpen = false">
      <div class="flex max-h-[88vh] w-full max-w-3xl flex-col rounded-xl border border-border bg-card shadow-2xl">
        <div class="flex items-center justify-between border-b border-border px-5 py-3">
          <div class="flex items-center gap-2">
            <Plus :size="16" />
            <span class="font-medium">新增 {{ editorFactory }} 頁（合規 template）</span>
          </div>
          <button class="text-muted-foreground hover:text-foreground" @click="editorOpen = false">
            <X :size="18" />
          </button>
        </div>

        <div class="border-b border-border px-5 py-2 text-xs text-muted-foreground">
          <span v-if="editorResult">
            已寫入：<code>{{ editorResult.target_dir }}/{{ editorResult.slug }}.md</code>
            <span v-if="editorResult.enriched" class="ml-2 text-green-500">
              LLM 已補 {{ editorResult.names_count }} 個 wikilink（可見上方已連結的人名/公司名）
            </span>
            <span v-else class="ml-2 text-warning">LLM 未啟用，人名未自動連結（原文照存）</span>
            <span v-if="editorResult.used_fallback" class="ml-2 text-warning">；title 為空用預設檔名，請填 title 重存</span>
          </span>
          <span v-else>存檔時：LLM 會把你寫的人名/公司名補成 <code>[[dir/名字]]</code>，並以 <code>title:</code> 內容作為檔名。</span>
        </div>

        <textarea
          v-model="editorMd"
          spellcheck="false"
          class="min-h-[40vh] flex-1 resize-y bg-background p-4 font-mono text-xs leading-relaxed outline-none"
        />

        <div v-if="editorError" class="px-5 py-2 text-xs text-destructive">{{ editorError }}</div>

        <div class="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
          <button class="rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent" @click="editorOpen = false">
            關閉
          </button>
          <button
            :disabled="editorBusy"
            class="flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent/50 disabled:opacity-50"
            @click="saveEditor"
          >
            <component :is="editorBusy ? Loader2 : Save" :size="14" :class="editorBusy ? 'animate-spin' : ''" />
            {{ editorBusy ? "LLM 補全中…" : `儲存（${editorSlug ? "覆蓋" : "取名寫入"}）` }}
          </button>
          <button
            :disabled="editorBusy"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="saveEditorAndSync"
          >
            <RefreshCw :size="14" /> 儲存並 Sync
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
