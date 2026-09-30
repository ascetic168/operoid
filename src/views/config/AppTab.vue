<script setup lang="ts">
import { reactive, ref, watchEffect } from "vue";
import { Save, CheckCircle2 } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { formatError, type AppConfig } from "@/lib/tauri";
import { LANGUAGE_OPTIONS } from "@/i18n/languageConfig";

const config = useConfigStore();

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
</template>
