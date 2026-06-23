<script setup lang="ts">
import { computed, reactive, ref, watchEffect } from "vue";
import { Save, AlertTriangle, CheckCircle2, RefreshCw } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import type { AppConfig } from "@/lib/tauri";

const config = useConfigStore();
if (!config.ready && !config.loading) config.load();

// 安全深拷貝（避開 structuredClone 對 Pinia reactive proxy 可能丟 DataCloneError）。
const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x)) as T;

// ---- GBrain raw JSON editor ----
const rawText = ref("");
const rawError = ref<string | null>(null);
const rawSaved = ref(false);

watchEffect(() => {
  if (config.gbrain) rawText.value = JSON.stringify(config.gbrain.raw, null, 2);
});

async function saveRaw() {
  rawError.value = null;
  rawSaved.value = false;
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawText.value);
  } catch (e) {
    rawError.value = `JSON 解析失敗：${e}`;
    return;
  }
  try {
    await config.saveGbrainRaw(parsed);
    rawSaved.value = true;
  } catch (e) {
    rawError.value = String(e);
  }
}

const llm = computed(() => config.gbrain?.llm_endpoint ?? null);

// ---- App config form ----
const form = reactive<AppConfig>({
  notes_repo_path: "",
  gbrain_exe_path: "",
  gbrain_home_override: null,
  auto_sync: true,
  sync_no_pull: true,
  factory_targets: { people: "people", companies: "companies", meetings: "meetings" },
  llm_temperature: 0.2,
  llm_max_tokens: 4096,
});

watchEffect(() => {
  if (config.app) Object.assign(form, clone(config.app));
});

const appSaved = ref(false);
const appError = ref<string | null>(null);

async function saveApp() {
  appError.value = null;
  appSaved.value = false;
  try {
    await config.saveAppConfig(clone(form) as AppConfig);
    appSaved.value = true;
  } catch (e) {
    appError.value = String(e);
  }
}
</script>

<template>
  <div class="flex h-full flex-col overflow-y-auto p-6">
    <header class="mb-6">
      <h1 class="text-xl font-semibold">設定</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        GBrain 的 config.json 是權威來源，系統讀取並直接使用；下方可檢視/編輯，以及本系統自有的設定。
      </p>
    </header>

    <!-- GBrain config -->
    <section class="mb-6 rounded-xl border border-border bg-card/40 p-5">
      <div class="mb-3 flex items-center justify-between">
        <h2 class="text-sm font-semibold">GBrain config（權威來源）</h2>
        <button
          class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
          @click="config.loadGbrain()"
        >
          <RefreshCw :size="13" /> 重讀
        </button>
      </div>

      <div v-if="config.gbrain" class="grid grid-cols-1 gap-2 text-sm sm:grid-cols-2">
        <div><span class="text-muted-foreground">home：</span><code>{{ config.gbrain.home }}</code></div>
        <div>
          <span class="text-muted-foreground">config：</span>
          <code>{{ config.gbrain.config_path }}</code>
          <span v-if="!config.gbrain.exists" class="ml-1 text-warning">（不存在）</span>
        </div>
        <div><span class="text-muted-foreground">chat_model：</span><code>{{ config.gbrain.chat_model ?? "—" }}</code></div>
        <div><span class="text-muted-foreground">embedding：</span><code>{{ config.gbrain.embedding_model ?? "—" }}</code></div>
        <div><span class="text-muted-foreground">schema_pack：</span><code>{{ config.gbrain.schema_pack ?? "—" }}</code></div>
        <div><span class="text-muted-foreground">database：</span><code>{{ config.gbrain.database_path ?? "—" }}</code></div>
      </div>

      <!-- LLM endpoint resolution -->
      <div class="mt-4 rounded-lg border border-border/60 bg-background/40 p-3 text-sm">
        <div class="mb-1 font-medium">LLM 端點解析（工廠結構化用）</div>
        <div v-if="llm" class="flex flex-wrap items-center gap-x-4 gap-y-1">
          <span>provider：<code>{{ llm.provider }}</code></span>
          <span>model：<code>{{ llm.model }}</code></span>
          <span>base_url：<code>{{ llm.base_url }}</code></span>
          <span v-if="llm.has_api_key" class="flex items-center gap-1 text-green-500">
            <CheckCircle2 :size="14" /> API key 已設
          </span>
          <span v-else class="flex items-center gap-1 text-warning">
            <AlertTriangle :size="14" /> 缺 API key（環境變數）
          </span>
        </div>
        <div v-else-if="config.gbrain?.llm_error" class="text-warning">
          無法解析：{{ config.gbrain.llm_error }}
        </div>
      </div>

      <!-- raw editor -->
      <label class="mt-4 block text-xs text-muted-foreground">config.json（file-plane 編輯；provider_base_urls 僅此處生效）</label>
      <textarea
        v-model="rawText"
        spellcheck="false"
        class="mt-1 h-64 w-full resize-y rounded-md border border-border bg-background p-2 font-mono text-xs"
      />
      <div class="mt-2 flex items-center gap-3">
        <button
          class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
          @click="saveRaw"
        >
          <Save :size="14" /> 寫回 config.json
        </button>
        <span v-if="rawError" class="text-xs text-destructive">{{ rawError }}</span>
        <span v-else-if="rawSaved" class="flex items-center gap-1 text-xs text-green-500">
          <CheckCircle2 :size="13" /> 已儲存
        </span>
      </div>
    </section>

    <!-- App config -->
    <section class="rounded-xl border border-border bg-card/40 p-5">
      <h2 class="mb-3 text-sm font-semibold">本系統設定（app config）</h2>
      <div class="grid grid-cols-1 gap-4 text-sm sm:grid-cols-2">
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">notes repo 路徑（sync --repo 對象）</span>
          <input v-model="form.notes_repo_path" class="rounded-md border border-border bg-background px-2 py-1.5" />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">gbrain.exe 路徑</span>
          <input v-model="form.gbrain_exe_path" class="rounded-md border border-border bg-background px-2 py-1.5" />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">GBRAIN_HOME 覆寫（指向 .gbrain 的父目錄；留空=預設腦）</span>
          <input
            v-model="form.gbrain_home_override"
            placeholder="（留空使用預設）"
            class="rounded-md border border-border bg-background px-2 py-1.5"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">LLM 溫度</span>
          <input
            v-model.number="form.llm_temperature"
            type="number"
            step="0.1"
            min="0"
            max="2"
            class="rounded-md border border-border bg-background px-2 py-1.5"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">LLM max_tokens</span>
          <input
            v-model.number="form.llm_max_tokens"
            type="number"
            step="128"
            min="256"
            class="rounded-md border border-border bg-background px-2 py-1.5"
          />
        </label>
        <div class="flex flex-col gap-2 sm:col-span-2">
          <label class="flex items-center gap-2">
            <input v-model="form.auto_sync" type="checkbox" />
            <span>工廠寫檔後自動 commit + sync</span>
          </label>
          <label class="flex items-center gap-2">
            <input v-model="form.sync_no_pull" type="checkbox" />
            <span>sync 加 --no-pull（無 remote 的腦建議開）</span>
          </label>
        </div>
        <fieldset class="grid grid-cols-3 gap-2 sm:col-span-2">
          <legend class="mb-1 text-muted-foreground">工廠目標子目錄（白名單）</legend>
          <input v-model="form.factory_targets.people" placeholder="people" class="rounded-md border border-border bg-background px-2 py-1.5" />
          <input v-model="form.factory_targets.companies" placeholder="companies" class="rounded-md border border-border bg-background px-2 py-1.5" />
          <input v-model="form.factory_targets.meetings" placeholder="meetings" class="rounded-md border border-border bg-background px-2 py-1.5" />
        </fieldset>
      </div>
      <div class="mt-4 flex items-center gap-3">
        <button
          class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
          @click="saveApp"
        >
          <Save :size="14" /> 儲存
        </button>
        <span v-if="appError" class="text-xs text-destructive">{{ appError }}</span>
        <span v-else-if="appSaved" class="flex items-center gap-1 text-xs text-green-500">
          <CheckCircle2 :size="13" /> 已儲存
        </span>
      </div>
    </section>
  </div>
</template>
