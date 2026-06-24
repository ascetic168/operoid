<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  Boxes,
  Star,
  Plus,
  Trash2,
  RefreshCw,
  Loader2,
  CheckCircle2,
  AlertTriangle,
  FolderOpen,
} from "lucide-vue-next";
import { useBrainsStore } from "@/stores/brains";
import { brainSync, formatError, type GbrainSource } from "@/lib/tauri";

const store = useBrainsStore();
const { t } = useI18n();
onMounted(() => store.load());

const selected = computed(() =>
  store.brains.find((b) => b.id === store.selectedBrainId) ?? null,
);
const isActive = (id: string) => store.activeId === id;

// ── 新增腦對話框 ──
const addOpen = ref(false);
const addCreate = ref(false);
const addName = ref("");
const addHome = ref("");
const addEm = ref("ollama:embeddinggemma");
const addDim = ref(768);
const addCm = ref("groq:llama-3.3-70b-versatile");
const addBusy = ref(false);
const addError = ref<string | null>(null);

function openAdd() {
  addCreate.value = false;
  addName.value = "";
  addHome.value = "";
  addEm.value = "ollama:embeddinggemma";
  addDim.value = 768;
  addCm.value = "groq:llama-3.3-70b-versatile";
  addError.value = null;
  addOpen.value = true;
}

async function pickHomeDir() {
  const d = await openDialog({ directory: true, multiple: false });
  if (typeof d === "string") addHome.value = d;
}

async function submitAdd() {
  addError.value = null;
  if (!addName.value.trim() || !addHome.value.trim()) {
    addError.value = t("brainsView.needNamePath");
    return;
  }
  addBusy.value = true;
  try {
    await store.add({
      name: addName.value.trim(),
      gbrain_home: addHome.value.trim(),
      create: addCreate.value,
      embedding_model: addEm.value.trim() || undefined,
      embedding_dimensions: addDim.value || undefined,
      chat_model: addCm.value.trim() || undefined,
    });
    addOpen.value = false;
  } catch (e) {
    addError.value = formatError(e);
  } finally {
    addBusy.value = false;
  }
}

// ── 新增來源對話框 ──
const srcOpen = ref(false);
const srcId = ref("");
const srcPath = ref("");
const srcBusy = ref(false);
const srcError = ref<string | null>(null);

function openAddSource() {
  srcId.value = "";
  srcPath.value = "";
  srcError.value = null;
  srcOpen.value = true;
}
async function pickSrcDir() {
  const d = await openDialog({ directory: true, multiple: false });
  if (typeof d === "string") srcPath.value = d;
}
async function submitAddSource() {
  if (!selected.value) return;
  srcError.value = null;
  if (!srcId.value.trim() || !srcPath.value.trim()) {
    srcError.value = t("brainsView.needSourceFields");
    return;
  }
  srcBusy.value = true;
  try {
    await store.addSource(selected.value.id, srcId.value.trim(), srcPath.value.trim());
    srcOpen.value = false;
  } catch (e) {
    srcError.value = formatError(e);
  } finally {
    srcBusy.value = false;
  }
}

// ── 同步 console ──
const log = ref<string[]>([]);
const syncing = ref(false);
async function push(line: string) {
  log.value.push(line);
}
async function syncAll() {
  if (!selected.value) return;
  syncing.value = true;
  log.value = [t("brainsView.syncAllStart", { name: selected.value.name })];
  try {
    const res = await brainSync(selected.value.id, "all", null, (l) => push(`[${l.stream}] ${l.text}`));
    push(res.success ? t("brainsView.syncDoneOk") : t("brainsView.syncDoneFail", { code: res.exit_code ?? "?" }));
    await store.loadSources(selected.value.id);
  } catch (e) {
    push(t("brainsView.syncErr", { e: formatError(e) }));
  } finally {
    syncing.value = false;
  }
}
async function syncOne(s: GbrainSource) {
  if (!selected.value) return;
  syncing.value = true;
  log.value = [t("brainsView.syncOneStart", { id: s.id, name: selected.value.name })];
  try {
    const res = await brainSync(selected.value.id, "one", s.id, (l) => push(`[${l.stream}] ${l.text}`));
    push(res.success ? t("brainsView.syncDoneOk") : t("brainsView.syncDoneFail", { code: res.exit_code ?? "?" }));
    await store.loadSources(selected.value.id);
  } catch (e) {
    push(t("brainsView.syncErr", { e: formatError(e) }));
  } finally {
    syncing.value = false;
  }
}

function fmtDate(s: string | null) {
  return s ? s.replace("T", " ").replace(/\..*$/, "") : "—";
}
</script>

<template>
  <div class="flex h-full overflow-hidden p-6">
    <!-- 左欄：腦清單 -->
    <aside class="mr-4 flex w-72 flex-col overflow-hidden rounded-xl border border-border bg-card/40">
      <div class="flex items-center justify-between border-b border-border px-4 py-3">
        <div class="flex items-center gap-2 font-semibold">
          <Boxes :size="18" /> {{ $t("brainsView.title") }}
        </div>
        <button class="flex items-center gap-1 rounded-md bg-primary px-2 py-1 text-xs text-primary-foreground hover:opacity-90" @click="openAdd">
          <Plus :size="13" /> {{ $t("brainsView.add") }}
        </button>
      </div>
      <div class="flex-1 overflow-y-auto p-2">
        <button
          v-for="b in store.brains"
          :key="b.id"
          :class="[
            'mb-1 w-full rounded-lg border p-3 text-left transition-colors',
            store.selectedBrainId === b.id ? 'border-primary bg-primary/5' : 'border-transparent hover:bg-accent/40',
          ]"
          @click="store.loadSources(b.id)"
        >
          <div class="flex items-center justify-between">
            <span class="flex items-center gap-1.5 font-medium">
              <Star v-if="isActive(b.id)" :size="13" class="text-amber-400 fill-amber-400" />
              {{ b.name }}
            </span>
            <span v-if="b.id === '__default__'" class="text-[10px] text-muted-foreground">{{ $t("brainsView.default") }}</span>
          </div>
          <div class="mt-0.5 truncate text-[11px] text-muted-foreground">
            {{ b.gbrain_home ? b.gbrain_home + "/.gbrain" : "~/.gbrain" }}
          </div>
        </button>
      </div>
    </aside>

    <!-- 右欄：腦詳情 + 來源 -->
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <template v-if="selected">
        <!-- 腦識別 -->
        <div class="mb-4 flex items-center justify-between rounded-xl border border-border bg-card/40 p-4">
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-lg font-semibold">{{ selected.name }}</h2>
              <span v-if="isActive(selected.id)" class="rounded bg-amber-400/20 px-1.5 py-0.5 text-[10px] text-amber-300">{{ $t("brainsView.active") }}</span>
            </div>
            <div class="mt-1 text-xs text-muted-foreground">
              <code>{{ selected.gbrain_home ? selected.gbrain_home + "/.gbrain" : "~/.gbrain" }}</code>
            </div>
          </div>
          <button
            v-if="!isActive(selected.id)"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
            @click="store.setActive(selected.id)"
          >
            <Star :size="13" /> {{ $t("brainsView.setActive") }}
          </button>
          <button
            v-if="selected.id !== '__default__'"
            class="ml-2 flex items-center gap-1 rounded-md border border-border px-2 py-1.5 text-xs text-destructive hover:bg-destructive/10"
            @click="store.remove(selected.id)"
          >
            <Trash2 :size="13" /> {{ $t("brainsView.remove") }}
          </button>
        </div>

        <!-- 來源 -->
        <div class="mb-3 flex items-center justify-between">
          <h3 class="text-sm font-semibold">{{ $t("brainsView.sourcesTitle") }}</h3>
          <div class="flex gap-2">
            <button :disabled="syncing" class="flex items-center gap-1 rounded-md border border-border px-2.5 py-1 text-xs hover:bg-accent/50 disabled:opacity-50" @click="syncAll">
              <RefreshCw :size="13" :class="syncing ? 'animate-spin' : ''" /> {{ $t("brainsView.syncAll") }}
            </button>
            <button class="flex items-center gap-1 rounded-md border border-border px-2.5 py-1 text-xs hover:bg-accent/50" @click="openAddSource">
              <Plus :size="13" /> {{ $t("brainsView.addSource") }}
            </button>
          </div>
        </div>

        <div v-if="store.sourcesLoading" class="flex items-center gap-2 text-sm text-muted-foreground"><Loader2 :size="14" class="animate-spin" /> {{ $t("brainsView.loadingSources") }}</div>
        <div v-else-if="store.sourcesError" class="flex items-center gap-2 text-sm text-destructive"><AlertTriangle :size="14" /> {{ store.sourcesError }}</div>
        <div v-else-if="store.sources.length === 0" class="text-sm text-muted-foreground">{{ $t("brainsView.noSources") }}</div>
        <div v-else class="mb-3 overflow-hidden rounded-lg border border-border">
          <table class="w-full text-sm">
            <thead class="bg-card/60 text-xs text-muted-foreground">
              <tr>
                <th class="px-3 py-2 text-left">{{ $t("brainsView.colId") }}</th>
                <th class="px-3 py-2 text-left">{{ $t("brainsView.colPath") }}</th>
                <th class="px-3 py-2 text-right">{{ $t("brainsView.colPages") }}</th>
                <th class="px-3 py-2 text-left">{{ $t("brainsView.colLastSync") }}</th>
                <th class="px-3 py-2 text-right">{{ $t("brainsView.colActions") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="s in store.sources" :key="s.id" class="border-t border-border">
                <td class="px-3 py-2 font-mono">
                  {{ s.id }}
                  <Star v-if="store.activeId === selected.id && store.activeSourceId === s.id" :size="11" class="ml-1 inline text-amber-400 fill-amber-400" />
                </td>
                <td class="px-3 py-2 font-mono text-xs text-muted-foreground">{{ s.local_path }}</td>
                <td class="px-3 py-2 text-right">{{ s.page_count }}</td>
                <td class="px-3 py-2 text-xs text-muted-foreground">{{ fmtDate(s.last_sync_at) }}</td>
                <td class="px-3 py-2 text-right">
                  <button :disabled="syncing" class="mr-1 rounded border border-border px-1.5 py-0.5 text-xs hover:bg-accent/50 disabled:opacity-50" @click="syncOne(s)">{{ $t("brainsView.sync") }}</button>
                  <button
                    v-if="isActive(selected.id)"
                    class="mr-1 rounded border border-border px-1.5 py-0.5 text-xs hover:bg-accent/50"
                    @click="store.setActiveSource(s.id)"
                  >{{ $t("brainsView.setActiveSource") }}</button>
                  <button class="rounded border border-border px-1.5 py-0.5 text-xs text-destructive hover:bg-destructive/10" @click="store.removeSource(selected.id, s.id)">{{ $t("brainsView.remove") }}</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- sync console -->
        <div v-if="log.length" class="mt-auto max-h-40 overflow-auto rounded-lg border border-border bg-background p-3 font-mono text-xs">
          <div v-for="(line, i) in log" :key="i" :class="{ 'text-amber-400': line.includes('✗') }">{{ line }}</div>
        </div>
      </template>

      <div v-else class="flex h-full items-center justify-center text-sm text-muted-foreground">{{ $t("brainsView.selectPrompt") }}</div>
    </main>

    <!-- 新增腦對話框 -->
    <div v-if="addOpen" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div class="w-full max-w-md rounded-xl border border-border bg-card p-5">
        <h3 class="mb-3 text-base font-semibold">{{ $t("brainsView.addBrainTitle") }}</h3>
        <div class="space-y-3 text-sm">
          <label class="block"><span class="text-muted-foreground">{{ $t("brainsView.nameLabel") }}</span><input v-model="addName" class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" /></label>
          <div>
            <span class="text-muted-foreground">{{ $t("brainsView.typeLabel") }}</span>
            <div class="mt-1 flex gap-3">
              <label class="flex items-center gap-1"><input type="radio" :checked="!addCreate" @change="addCreate = false" /> {{ $t("brainsView.registerExisting") }}</label>
              <label class="flex items-center gap-1"><input type="radio" :checked="addCreate" @change="addCreate = true" /> {{ $t("brainsView.createNew") }}</label>
            </div>
          </div>
          <label class="block">
            <span class="text-muted-foreground">{{ $t("brainsView.pathLabel") }}</span>
            <div class="mt-1 flex gap-2">
              <input v-model="addHome" class="flex-1 rounded-md border border-border bg-background px-2 py-1.5" />
              <button class="rounded-md border border-border px-2 hover:bg-accent/50" @click="pickHomeDir"><FolderOpen :size="15" /></button>
            </div>
          </label>
          <template v-if="addCreate">
            <label class="block"><span class="text-muted-foreground">{{ $t("brainsView.embeddingModel") }}</span><input v-model="addEm" class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" /></label>
            <label class="block"><span class="text-muted-foreground">{{ $t("brainsView.embeddingDim") }}</span><input v-model.number="addDim" type="number" class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" /></label>
            <label class="block"><span class="text-muted-foreground">{{ $t("brainsView.chatModel") }}</span><input v-model="addCm" class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" /></label>
          </template>
          <div v-if="addError" class="text-xs text-destructive">{{ addError }}</div>
        </div>
        <div class="mt-4 flex justify-end gap-2">
          <button class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent/50" @click="addOpen = false">{{ $t("common.cancel") }}</button>
          <button :disabled="addBusy" class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50" @click="submitAdd">
            <Loader2 v-if="addBusy" :size="13" class="animate-spin" />
            <CheckCircle2 v-else :size="13" /> {{ addCreate ? $t("brainsView.create") : $t("brainsView.register") }}
          </button>
        </div>
      </div>
    </div>

    <!-- 新增來源對話框 -->
    <div v-if="srcOpen" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div class="w-full max-w-md rounded-xl border border-border bg-card p-5">
        <h3 class="mb-3 text-base font-semibold">{{ $t("brainsView.addSourceTitle") }}</h3>
        <div class="space-y-3 text-sm">
          <label class="block"><span class="text-muted-foreground">{{ $t("brainsView.sourceIdLabel") }}</span><input v-model="srcId" class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" /></label>
          <label class="block">
            <span class="text-muted-foreground">{{ $t("brainsView.repoPathLabel") }}</span>
            <div class="mt-1 flex gap-2">
              <input v-model="srcPath" class="flex-1 rounded-md border border-border bg-background px-2 py-1.5" />
              <button class="rounded-md border border-border px-2 hover:bg-accent/50" @click="pickSrcDir"><FolderOpen :size="15" /></button>
            </div>
          </label>
          <div v-if="srcError" class="text-xs text-destructive">{{ srcError }}</div>
        </div>
        <div class="mt-4 flex justify-end gap-2">
          <button class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent/50" @click="srcOpen = false">{{ $t("common.cancel") }}</button>
          <button :disabled="srcBusy" class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50" @click="submitAddSource">
            <Loader2 v-if="srcBusy" :size="13" class="animate-spin" />
            <CheckCircle2 v-else :size="13" /> {{ $t("brainsView.add") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
