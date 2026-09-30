<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { Save, CheckCircle2, RefreshCw } from "lucide-vue-next";
import { formatError, obridgeConfigLoad, obridgeConfigSave, serverInfoExt, serverServiceInstall, serverServiceUninstall } from "@/lib/tauri";

// 本地服務（oserver）狀態與開機服務開關（P5）——即時切換，不經表單儲存。
const server = reactive({ port: 0, running: false, installed: false, busy: false, error: null as string | null });
async function loadServerInfo() {
  try {
    const info = await serverInfoExt();
    server.port = info.port;
    server.running = info.running;
    server.installed = info.service_installed;
    server.error = null;
  } catch (e) {
    server.error = formatError(e);
  }
}
async function toggleService() {
  server.busy = true;
  server.error = null;
  try {
    if (server.installed) {
      await serverServiceUninstall();
    } else {
      await serverServiceInstall();
    }
    await loadServerInfo();
  } catch (e) {
    server.error = formatError(e);
  } finally {
    server.busy = false;
  }
}
onMounted(loadServerInfo);

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
    // AppError 是物件（{code, params}）——以 code 判別「未設定路徑」的指引情境。
    const code = e && typeof e === "object" && "code" in e ? String((e as { code: unknown }).code) : "";
    obridgeNeedPath.value = code.includes("obridge.noConfigPath");
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
</script>

<template>
  <div class="flex flex-col gap-6">
    <!-- 本地服務（oserver）：從 app config 表單移入——開關是即時動作，非表單欄位 -->
    <section class="rounded-xl border border-border bg-card/40 p-5">
      <h2 class="mb-3 text-sm font-semibold">{{ $t("server.section") }}</h2>
      <div class="rounded-md border border-border p-3 flex flex-col gap-2 text-sm">
        <div class="flex items-center gap-2 text-xs">
          <span :class="server.running ? 'text-green-600' : 'text-red-500'">
            {{ server.running ? $t("server.running") : $t("server.stopped") }} · 127.0.0.1:{{ server.port }}
          </span>
        </div>
        <label class="flex items-center gap-2">
          <input
            type="checkbox"
            :checked="server.installed"
            :disabled="server.busy"
            @change="toggleService"
          />
          <span class="text-xs">{{ $t("server.serviceLabel") }}</span>
        </label>
        <p class="text-[11px] text-muted-foreground">
          {{ server.installed ? $t("server.serviceOnHint") : $t("server.serviceOffHint") }}
        </p>
        <p v-if="server.error" class="text-[11px] text-red-500">{{ server.error }}</p>
      </div>
    </section>

    <!-- Obridge 設定代管（原始文字編輯——Operoid 只當編輯器，不解讀內容） -->
    <section class="rounded-xl border border-border bg-card/40 p-5">
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
