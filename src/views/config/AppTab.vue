<script setup lang="ts">
import { computed, reactive, ref, watchEffect } from "vue";
import { Save, CheckCircle2, RefreshCw, Download, RotateCcw, ExternalLink } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { useUpdateStore } from "@/stores/update";
import { formatError, openUrl, type AppConfig } from "@/lib/tauri";
import { LANGUAGE_OPTIONS } from "@/i18n/languageConfig";

const config = useConfigStore();
const update = useUpdateStore();

// 安全深拷貝（避開 structuredClone 對 Pinia reactive proxy 可能丟 DataCloneError）。
const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x)) as T;

// ---- App config form（整表批次儲存——本 TAB 唯一的儲存語意）----
const form = reactive<AppConfig>({
  notes_repo_path: "",
  gbrain_exe_path: "",
  gbrain_home_override: null,
  brains: [],
  active_brain_id: null,
  active_source_id: null,
  auto_sync: true,
  sync_no_pull: true,
  default_embedding_model: null,
  default_embedding_dimensions: null,
  llm_temperature: 0.2,
  llm_max_tokens: 4096,
  locale: null,
  recent_claude_cwds: [],
  claude_terminal: null,
  claude_terminal_template: null,
  agent_os_enabled: false,
  gbrain_transport: "mcp",
  obridge_config_path: null,
  obridge_autostart: false,
  obridge_executable: null,
  update_notify: true,
  auto_update: false,
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

async function onLocaleChange(v: string) {
  try {
    await config.setLocale(v || null);
  } catch (e) {
    appError.value = formatError(e);
  }
}

// ---- 版本與更新區塊 ----
const updateBusy = computed(() => update.state === "checking" || update.state === "downloading");
const RELEASES_URL = "https://github.com/ascetic168/operoid/releases";
</script>

<template>
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
        <label class="flex items-center gap-2 text-sm">
          <span>{{ $t("configView.gbrainTransportLabel") }}</span>
          <select v-model="form.gbrain_transport" class="rounded-md border border-border bg-background px-2 py-1.5">
            <option value="mcp">MCP</option>
            <option value="cli">CLI</option>
          </select>
          <span class="text-muted-foreground text-xs">{{ $t("configView.gbrainTransportHint") }}</span>
        </label>
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
        <label class="flex items-center gap-2">
          <input v-model="form.obridge_autostart" type="checkbox" />
          <span>{{ $t("configView.obridgeAutostartLabel") }}</span>
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-xs text-muted-foreground">{{ $t("configView.obridgeExeLabel") }}</span>
          <input
            v-model="form.obridge_executable"
            type="text"
            class="rounded-md border border-border bg-background px-2 py-1 text-xs"
            :placeholder="$t('configView.obridgeExePlaceholder')"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-xs text-muted-foreground">{{ $t("configView.obridgePathLabel") }}</span>
          <input
            v-model="form.obridge_config_path"
            type="text"
            class="rounded-md border border-border bg-background px-2 py-1 text-xs"
            :placeholder="$t('configView.obridgeExePlaceholder')"
          />
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

  <!-- 版本與更新 -->
  <section class="rounded-xl border border-border bg-card/40 p-5">
    <h2 class="mb-3 text-sm font-semibold">{{ $t("configView.updateSection") }}</h2>
    <div class="flex flex-col gap-2 text-sm">
      <div class="flex items-center gap-2">
        <span class="text-muted-foreground">{{ $t("configView.currentVersion") }}</span>
        <span class="font-mono text-foreground">v{{ config.info?.version ?? "?" }}</span>
        <button
          class="ml-2 flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-accent"
          :disabled="updateBusy"
          @click="update.check()"
        >
          <RefreshCw :size="12" :class="{ 'animate-spin': update.state === 'checking' }" />
          {{ $t("configView.checkUpdate") }}
        </button>
        <button
          v-if="update.state === 'available'"
          class="flex items-center gap-1 rounded-md bg-primary px-2 py-1 text-xs text-primary-foreground hover:opacity-90"
          :disabled="updateBusy"
          @click="update.downloadAndInstall()"
        >
          <Download :size="12" /> {{ $t("configView.downloadInstall") }}
        </button>
        <button
          v-if="update.state === 'ready'"
          class="flex items-center gap-1 rounded-md bg-primary px-2 py-1 text-xs text-primary-foreground hover:opacity-90"
          @click="update.relaunchNow()"
        >
          <RotateCcw :size="12" /> {{ $t("configView.relaunch") }}
        </button>
        <a
          class="ml-auto flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
          :href="RELEASES_URL"
          @click.prevent="openUrl(RELEASES_URL)"
        >
          <ExternalLink :size="11" /> GitHub Releases
        </a>
      </div>
      <!-- 檢查／下載狀態列 -->
      <div class="flex flex-wrap items-center gap-2 text-xs">
        <span v-if="update.state === 'uptodate'" class="text-muted-foreground">{{ $t("configView.upToDate") }}</span>
        <template v-else-if="update.state === 'available'">
          <span class="font-medium text-primary">{{ $t("configView.updateAvailable", { version: update.version }) }}</span>
        </template>
        <span v-else-if="update.state === 'downloading'" class="text-muted-foreground">
          {{ update.progress !== null
            ? $t("configView.downloadingProgress", { progress: update.progress })
            : $t("configView.downloadingIndeterminate") }}
        </span>
        <span v-else-if="update.state === 'ready'" class="text-green-500">
          {{ $t("configView.readyToRelaunch", { version: update.version }) }}
        </span>
        <span v-else-if="update.state === 'error' && update.errorKind === 'noinfo'" class="text-muted-foreground">
          {{ $t("configView.updateNoInfo") }}
        </span>
        <span v-else-if="update.state === 'error'" class="text-destructive">
          {{ $t("configView.updateCheckFailed", { msg: update.errorMsg }) }}
        </span>
      </div>
      <!-- 下載進度條 -->
      <div v-if="update.state === 'downloading' && update.progress !== null" class="h-1 w-full overflow-hidden rounded bg-background">
        <div class="h-full bg-primary transition-all" :style="{ width: `${update.progress}%` }" />
      </div>
      <!-- 版本說明（可摺疊） -->
      <details v-if="update.notes" class="mt-1">
        <summary class="cursor-pointer text-xs text-muted-foreground hover:text-foreground">
          {{ $t("configView.releaseNotes") }}
        </summary>
        <pre class="mt-1 max-h-60 overflow-y-auto whitespace-pre-wrap rounded-md border border-border bg-background/60 p-2 text-xs text-muted-foreground">{{ update.notes }}</pre>
      </details>
      <!-- 選項（併入本 TAB 批次儲存） -->
      <div class="mt-2 flex flex-col gap-1.5">
        <label class="flex items-center gap-2">
          <input v-model="form.update_notify" type="checkbox" />
          <span>{{ $t("configView.updateNotifyLabel") }}</span>
        </label>
        <div class="flex flex-col gap-0.5">
          <label class="flex items-center gap-2">
            <input v-model="form.auto_update" type="checkbox" />
            <span>{{ $t("configView.autoUpdateLabel") }}</span>
          </label>
          <span class="pl-6 text-xs text-warning">{{ $t("configView.autoUpdateHint") }}</span>
        </div>
      </div>
    </div>
  </section>
</template>
