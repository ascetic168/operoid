<script setup lang="ts">
import { computed, onMounted, reactive, ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { Save, AlertTriangle, CheckCircle2, RefreshCw, Trash2 } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { formatError, tL10n, type AppConfig, obridgeConfigLoad, obridgeConfigSave } from "@/lib/tauri";
import { LANGUAGE_OPTIONS } from "@/i18n/languageConfig";

const config = useConfigStore();
const { t } = useI18n();
if (!config.ready && !config.loading) config.load();

// 安全深拷貝（避開 structuredClone 對 Pinia reactive proxy 可能丟 DataCloneError）。
const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x)) as T;

// ---- GBrain raw JSON editor（進階；model/tier 鍵可能被 DB plane 蓋過） ----
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
    rawError.value = t("configView.jsonParseFail", { e: String(e) });
    return;
  }
  try {
    await config.saveGbrainRaw(parsed);
    rawSaved.value = true;
  } catch (e) {
    rawError.value = formatError(e);
  }
}

const llm = computed(() => config.gbrain?.llm_endpoint ?? null);

// ---- DB-plane 覆寫橫幅 ----
const dbOverrides = computed(() => config.gbrain?.db_overrides ?? []);
const clearing = ref(false);
const clearError = ref<string | null>(null);

async function clearOverrides() {
  clearing.value = true;
  clearError.value = null;
  try {
    await config.clearDbOverrides();
  } catch (e) {
    clearError.value = formatError(e);
  } finally {
    clearing.value = false;
  }
}

// ---- Tier 路由編輯（v0.42） ----
// unifyModel=true（預設）：單一輸入框，套用時同步到全部 tier（set_gbrain_models_all）。
// unifyModel=false：四個 tier 各自獨立編輯（set_gbrain_model）。
const unifyModel = ref(true);
const mainModel = ref("");
const mainModelSaved = ref(false);
const mainModelError = ref<string | null>(null);

// 四個 tier 的本地輸入值
const tierInputs = reactive<{ utility: string; reasoning: string; deep: string; subagent: string }>({
  utility: "",
  reasoning: "",
  deep: "",
  subagent: "",
});
const tierSaved = reactive<{ utility: boolean; reasoning: boolean; deep: boolean; subagent: boolean }>({
  utility: false,
  reasoning: false,
  deep: false,
  subagent: false,
});
const tierError = ref<string | null>(null);

// 從 store 同步輸入框初值
watchEffect(() => {
  const g = config.gbrain;
  if (!g) return;
  mainModel.value = g.chat_model ?? "";
  tierInputs.utility = g.tiers.utility ?? "";
  tierInputs.reasoning = g.tiers.reasoning ?? "";
  tierInputs.deep = g.tiers.deep ?? "";
  tierInputs.subagent = g.tiers.subagent ?? "";
});

const TIERS = [
  { key: "utility", modelKey: "models.tier.utility", label: "tierUtility" },
  { key: "reasoning", modelKey: "models.tier.reasoning", label: "tierReasoning" },
  { key: "deep", modelKey: "models.tier.deep", label: "tierDeep" },
  { key: "subagent", modelKey: "models.tier.subagent", label: "tierSubagent" },
] as const;

// 單一模型套用全部 tier
async function applyMainModel() {
  mainModelError.value = null;
  mainModelSaved.value = false;
  const v = mainModel.value.trim();
  if (!v) {
    mainModelError.value = t("gbrain.configEmptyValue");
    return;
  }
  try {
    await config.setGbrainModelsAll(v);
    mainModelSaved.value = true;
  } catch (e) {
    mainModelError.value = formatError(e);
  }
}

// 設單一 tier
async function applyTier(tierKey: "utility" | "reasoning" | "deep" | "subagent", modelKey: string) {
  tierError.value = null;
  const v = tierInputs[tierKey].trim();
  if (!v) {
    tierError.value = t("gbrain.configEmptyValue");
    return;
  }
  try {
    await config.setGbrainModel(modelKey, v);
    tierSaved[tierKey] = true;
  } catch (e) {
    tierError.value = formatError(e);
  }
}

// tier 來源徽章文字
function tierSourceLabel(src: string): string {
  if (src === "db") return t("configView.tierSourceDb");
  if (src === "file") return t("configView.tierSourceFile");
  return t("configView.tierSourceDefault");
}

// ---- Provider base URL 編輯（file-plane，直寫 config.json） ----
const PROVIDERS = [
  "groq", "openai", "anthropic", "ollama", "deepseek",
  "together", "openrouter", "zhipu", "dashscope", "zeroentropy",
];
const selProvider = ref("zhipu");
const baseUrl = ref("");
const baseUrlSaved = ref(false);
const baseUrlError = ref<string | null>(null);

watchEffect(() => {
  const pbus = config.gbrain?.provider_base_urls ?? {};
  // 當前選中 provider 的既有值帶入輸入框
  baseUrl.value = pbus[selProvider.value] ?? "";
});

async function applyBaseUrl() {
  baseUrlError.value = null;
  baseUrlSaved.value = false;
  try {
    const v = baseUrl.value.trim();
    await config.setProviderBaseUrl(selProvider.value, v || null);
    baseUrlSaved.value = true;
  } catch (e) {
    baseUrlError.value = formatError(e);
  }
}

async function clearBaseUrl() {
  baseUrl.value = "";
  await applyBaseUrl();
}

// ---- App config form ----
const form = reactive<AppConfig>({
  notes_repo_path: "",
  gbrain_exe_path: "",
  gbrain_home_override: null,
  brains: [],
  active_brain_id: null,
  active_source_id: null,
  auto_sync: true,
  sync_no_pull: true,
  llm_temperature: 0.2,
  llm_max_tokens: 4096,
  locale: null,
  recent_claude_cwds: [],
  claude_terminal: null,
  claude_terminal_template: null,
  agent_os_enabled: false,
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
    appError.value = formatError(e);
  }
}

// ---- Obridge 設定代管（原始文字編輯；路徑未設定時顯示提示而非區塊內容）----
const obridgeText = ref("");
const obridgeAvailable = ref(false);
const obridgeNeedPath = ref(false);
const obridgeError = ref<string | null>(null);
const obridgeSaved = ref(false);

async function loadObridge() {
  obridgeError.value = null;
  obridgeSaved.value = false;
  try {
    obridgeText.value = await obridgeConfigLoad();
    obridgeAvailable.value = true;
    obridgeNeedPath.value = false;
  } catch (e) {
    obridgeAvailable.value = false;
    obridgeNeedPath.value = String(e).includes("obridge.noConfigPath");
    if (!obridgeNeedPath.value) obridgeError.value = formatError(e);
  }
}

async function saveObridge() {
  obridgeError.value = null;
  obridgeSaved.value = false;
  try {
    await obridgeConfigSave(obridgeText.value);
    obridgeSaved.value = true;
  } catch (e) {
    obridgeError.value = formatError(e);
  }
}

onMounted(loadObridge);

async function onLocaleChange(v: string) {
  try {
    await config.setLocale(v || null);  } catch (e) {
    appError.value = formatError(e);
  }
}
</script>

<template>
  <div class="flex h-full flex-col overflow-y-auto p-6">
    <header class="mb-6">
      <h1 class="text-xl font-semibold">{{ $t("configView.title") }}</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        {{ $t("configView.desc") }}
      </p>
    </header>

    <!-- GBrain config -->
    <section class="mb-6 rounded-xl border border-border bg-card/40 p-5">
      <div class="mb-3 flex items-center justify-between">
        <h2 class="text-sm font-semibold">{{ $t("configView.gbrainSection") }}</h2>
        <button
          class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
          @click="config.loadGbrain()"
        >
          <RefreshCw :size="13" /> {{ $t("common.refresh") }}
        </button>
      </div>

      <div v-if="config.gbrain" class="grid grid-cols-1 gap-2 text-sm sm:grid-cols-2">
        <div><span class="text-muted-foreground">home：</span><code>{{ config.gbrain.home }}</code></div>
        <div>
          <span class="text-muted-foreground">config：</span>
          <code>{{ config.gbrain.config_path }}</code>
          <span v-if="!config.gbrain.exists" class="ml-1 text-warning">{{ $t("configView.notExists") }}</span>
        </div>
        <div><span class="text-muted-foreground">chat_model：</span><code>{{ config.gbrain.chat_model ?? $t("common.dash") }}</code></div>
        <div><span class="text-muted-foreground">embedding：</span><code>{{ config.gbrain.embedding_model ?? $t("common.dash") }}</code></div>
        <div><span class="text-muted-foreground">schema_pack：</span><code>{{ config.gbrain.schema_pack ?? $t("common.dash") }}</code></div>
        <div><span class="text-muted-foreground">database：</span><code>{{ config.gbrain.database_path ?? $t("common.dash") }}</code></div>
      </div>

      <!-- LLM endpoint resolution -->
      <div class="mt-4 rounded-lg border border-border/60 bg-background/40 p-3 text-sm">
        <div class="mb-1 font-medium">{{ $t("configView.llmTitle") }}</div>
        <div v-if="llm" class="flex flex-wrap items-center gap-x-4 gap-y-1">
          <span>provider：<code>{{ llm.provider }}</code></span>
          <span>model：<code>{{ llm.model }}</code></span>
          <span>base_url：<code>{{ llm.base_url }}</code></span>
          <span v-if="llm.has_api_key" class="flex items-center gap-1 text-green-500">
            <CheckCircle2 :size="14" /> {{ $t("configView.llmKeySet") }}
          </span>
          <span v-else class="flex items-center gap-1 text-warning">
            <AlertTriangle :size="14" /> {{ $t("configView.llmKeyMissing") }}
          </span>
        </div>
        <div v-else-if="config.gbrain?.llm_error" class="text-warning">
          {{ $t("configView.llmResolveFail", { error: tL10n(config.gbrain.llm_error) }) }}
        </div>
      </div>

      <!-- DB-plane 覆寫警告橫幅 -->
      <div
        v-if="dbOverrides.length > 0"
        class="mt-4 flex flex-wrap items-center gap-2 rounded-lg border border-warning/40 bg-warning/10 p-3 text-sm"
      >
        <AlertTriangle :size="15" class="shrink-0 text-warning" />
        <span class="text-warning">{{ $t("configView.dbOverrideBanner") }}</span>
        <code class="text-xs">{{ dbOverrides.join(", ") }}</code>
        <button
          class="ml-auto flex items-center gap-1 rounded-md border border-warning/50 px-2 py-1 text-xs text-warning hover:bg-warning/15"
          :disabled="clearing"
          @click="clearOverrides"
        >
          <Trash2 :size="13" /> {{ $t("configView.clearOverrides") }}
        </button>
        <span v-if="clearError" class="w-full text-xs text-destructive">{{ clearError }}</span>
      </div>

      <!-- Tier 路由編輯（v0.42） -->
      <div class="mt-4 rounded-lg border border-border/60 bg-background/40 p-3 text-sm">
        <div class="mb-1 flex items-center gap-2">
          <span class="font-medium">{{ $t("configView.tierSection") }}</span>
        </div>
        <p class="mb-2 text-xs text-muted-foreground">{{ $t("configView.tierDesc") }}</p>

        <!-- 勾選：單一模型 vs 分層 -->
        <label class="mb-3 flex items-center gap-2 text-xs">
          <input v-model="unifyModel" type="checkbox" class="h-3.5 w-3.5" />
          <span>{{ $t("configView.unifyModel") }}</span>
          <span class="text-muted-foreground">— {{ $t("configView.unifyModelHint") }}</span>
        </label>

        <!-- 勾選時：單一輸入框，套用全部 tier -->
        <div v-if="unifyModel">
          <p class="mb-2 text-xs text-muted-foreground">{{ $t("configView.mainModelHint") }}</p>
          <div class="flex flex-wrap items-center gap-2">
            <input
              v-model="mainModel"
              :placeholder="$t('configView.mainModelPh')"
              class="min-w-[16rem] flex-1 rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
            />
            <button
              class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
              @click="applyMainModel"
            >
              <Save :size="14" /> {{ $t("configView.apply") }}
            </button>
            <span v-if="mainModelError" class="text-xs text-destructive">{{ mainModelError }}</span>
            <span v-else-if="mainModelSaved" class="flex items-center gap-1 text-xs text-green-500">
              <CheckCircle2 :size="13" /> {{ $t("configView.saved") }}
            </span>
          </div>
        </div>

        <!-- 取消勾選時：四個 tier 各自獨立 -->
        <div v-else class="flex flex-col gap-2">
          <div v-for="t in TIERS" :key="t.key" class="flex flex-wrap items-center gap-2">
            <span class="w-40 shrink-0 text-xs text-muted-foreground">{{ $t(`configView.${t.label}`) }}</span>
            <input
              v-model="tierInputs[t.key]"
              :placeholder="$t('configView.tierModelPh')"
              class="min-w-[14rem] flex-1 rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
            />
            <button
              class="flex items-center gap-1 rounded-md bg-primary px-2 py-1.5 text-xs text-primary-foreground hover:opacity-90"
              @click="applyTier(t.key, t.modelKey)"
            >
              <Save :size="13" /> {{ $t("configView.apply") }}
            </button>
            <span
              v-if="config.gbrain"
              class="rounded px-1.5 py-0.5 text-[10px]"
              :class="config.gbrain.tier_source[t.key] === 'db'
                ? 'bg-green-500/15 text-green-500'
                : 'bg-muted text-muted-foreground'"
            >
              {{ tierSourceLabel(config.gbrain.tier_source[t.key]) }}
            </span>
            <CheckCircle2 v-if="tierSaved[t.key]" :size="13" class="text-green-500" />
          </div>
          <!-- subagent prompt-caching 警告 -->
          <p class="flex items-start gap-1 text-xs text-warning">
            <AlertTriangle :size="13" class="mt-0.5 shrink-0" />
            <span>{{ $t("configView.subagentCacheWarn") }}</span>
          </p>
          <span v-if="tierError" class="text-xs text-destructive">{{ tierError }}</span>
        </div>
      </div>

      <!-- Provider base URL 編輯（file-plane） -->
      <div class="mt-4 rounded-lg border border-border/60 bg-background/40 p-3 text-sm">
        <div class="mb-1 font-medium">{{ $t("configView.providerUrlSection") }}</div>
        <p class="mb-2 text-xs text-muted-foreground">{{ $t("configView.providerUrlDesc") }}</p>
        <div class="flex flex-wrap items-center gap-2">
          <select
            v-model="selProvider"
            class="rounded-md border border-border bg-background px-2 py-1.5 text-xs"
          >
            <option v-for="p in PROVIDERS" :key="p" :value="p">{{ p }}</option>
          </select>
          <input
            v-model="baseUrl"
            :placeholder="$t('configView.baseUrlPh')"
            class="min-w-[16rem] flex-1 rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
          />
          <button
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
            @click="applyBaseUrl"
          >
            <Save :size="14" /> {{ $t("configView.apply") }}
          </button>
          <button
            class="flex items-center gap-1 rounded-md border border-border px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent"
            @click="clearBaseUrl"
            :title="$t('configView.clear')"
          >
            <Trash2 :size="13" /> {{ $t("configView.clear") }}
          </button>
          <span class="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {{ $t("configView.filePlaneOnly") }}
          </span>
          <span v-if="baseUrlError" class="w-full text-xs text-destructive">{{ baseUrlError }}</span>
          <span v-else-if="baseUrlSaved" class="flex items-center gap-1 text-xs text-green-500">
            <CheckCircle2 :size="13" /> {{ $t("configView.saved") }}
          </span>
        </div>
      </div>

      <!-- raw editor（進階；警告 model/tier 可能被 DB plane 蓋過） -->
      <label class="mt-4 block text-xs text-muted-foreground">{{ $t("configView.rawLabel") }}</label>
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
          <Save :size="14" /> {{ $t("configView.writeBack") }}
        </button>
        <span v-if="rawError" class="text-xs text-destructive">{{ rawError }}</span>
        <span v-else-if="rawSaved" class="flex items-center gap-1 text-xs text-green-500">
          <CheckCircle2 :size="13" /> {{ $t("configView.saved") }}
        </span>
      </div>
    </section>

    <!-- App config -->
    <section class="rounded-xl border border-border bg-card/40 p-5">
      <h2 class="mb-3 text-sm font-semibold">{{ $t("configView.appSection") }}</h2>
      <div class="grid grid-cols-1 gap-4 text-sm sm:grid-cols-2">
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">{{ $t("configView.notesRepoLabel") }}</span>
          <input v-model="form.notes_repo_path" class="rounded-md border border-border bg-background px-2 py-1.5" />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">{{ $t("configView.exeLabel") }}</span>
          <input v-model="form.gbrain_exe_path" class="rounded-md border border-border bg-background px-2 py-1.5" />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">{{ $t("configView.homeOverrideLabel") }}</span>
          <input
            v-model="form.gbrain_home_override"
            :placeholder="$t('configView.homeOverridePh')"
            class="rounded-md border border-border bg-background px-2 py-1.5"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">{{ $t("configView.tempLabel") }}</span>
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
          <span class="text-muted-foreground">{{ $t("configView.maxTokensLabel") }}</span>
          <input
            v-model.number="form.llm_max_tokens"
            type="number"
            step="128"
            min="256"
            class="rounded-md border border-border bg-background px-2 py-1.5"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-muted-foreground">{{ $t("configView.languageLabel") }}</span>
          <select
            class="rounded-md border border-border bg-background px-2 py-1.5"
            :value="config.app?.locale ?? ''"
            @change="onLocaleChange(($event.target as HTMLSelectElement).value)"
          >
            <option value="">{{ $t("configView.languageAuto") }}</option>
            <option v-for="opt in LANGUAGE_OPTIONS" :key="opt.locale" :value="opt.locale">{{ opt.displayName }}</option>
          </select>
        </label>
        <div class="flex flex-col gap-2 sm:col-span-2">
          <label class="flex items-center gap-2">
            <input v-model="form.auto_sync" type="checkbox" />
            <span>{{ $t("configView.autoSyncLabel") }}</span>
          </label>
          <label class="flex items-center gap-2">
            <input v-model="form.sync_no_pull" type="checkbox" />
            <span>{{ $t("configView.noPullLabel") }}</span>
          </label>
          <label class="flex items-center gap-2">
            <input v-model="form.agent_os_enabled" type="checkbox" />
            <span>{{ $t("configView.agentOsLabel") }}</span>
          </label>
        </div>
      </div>
      <div class="mt-4 flex items-center gap-3">
        <button
          class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
          @click="saveApp"
        >
          <Save :size="14" /> {{ $t("common.save") }}
        </button>
        <span v-if="appError" class="text-xs text-destructive">{{ appError }}</span>
        <span v-else-if="appSaved" class="flex items-center gap-1 text-xs text-green-500">
          <CheckCircle2 :size="13" /> {{ $t("configView.saved") }}
        </span>
      </div>
    </section>

    <!-- Obridge 設定代管（原始文字編輯——Operoid 只當編輯器，不解讀內容） -->
    <section class="mt-6 rounded-xl border border-border bg-card/40 p-5">
      <h2 class="mb-2 text-sm font-semibold">{{ $t("configView.obridgeTitle") }}</h2>
      <p class="mb-3 text-xs text-muted-foreground">{{ $t("configView.obridgeDesc") }}</p>
      <p v-if="obridgeNeedPath" class="text-xs text-muted-foreground">
        {{ $t("configView.obridgeNeedPath") }}
      </p>
      <template v-else-if="obridgeAvailable">
        <textarea
          v-model="obridgeText"
          class="h-72 w-full rounded-md border border-border bg-background p-2 font-mono text-xs"
          spellcheck="false"
        />
        <div class="mt-3 flex items-center gap-3">
          <button
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
            @click="saveObridge"
          >
            <Save :size="14" /> {{ $t("common.save") }}
          </button>
          <button
            class="flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs hover:opacity-80"
            @click="loadObridge"
          >
            <RefreshCw :size="13" /> {{ $t("configView.obridgeReload") }}
          </button>
          <span v-if="obridgeError" class="text-xs text-destructive">{{ obridgeError }}</span>
          <span v-else-if="obridgeSaved" class="flex items-center gap-1 text-xs text-green-500">
            <CheckCircle2 :size="13" /> {{ $t("configView.saved") }}
            {{ $t("configView.obridgeRestartNote") }}
          </span>
        </div>
      </template>
      <p v-else-if="obridgeError" class="text-xs text-destructive">{{ obridgeError }}</p>
    </section>
  </div>
</template>
