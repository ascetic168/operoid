<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { Save, CheckCircle2, RefreshCw } from "lucide-vue-next";
import { formatError, knowledgeHealth, obridgeConfigLoad, obridgeConfigSave, serverInfoExt, serverServiceInstall, serverServiceUninstall, type KnowledgeHealth } from "@/lib/tauri";

// ---- K6 知識管線能力狀態（MinerU／嵌入多模態／chat VLM）----
const caps = ref<KnowledgeHealth | null>(null);
const capsError = ref<string | null>(null);
const capsLoading = ref(false);
async function loadCaps() {
  capsLoading.value = true;
  capsError.value = null;
  try {
    caps.value = await knowledgeHealth();
  } catch (e) {
    capsError.value = formatError(e);
  } finally {
    capsLoading.value = false;
  }
}

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
onMounted(() => {
  loadServerInfo();
  void loadCaps();
});

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
    <!-- K6 知識管線能力狀態：能力矩陣（缺項不阻塞——fallback 已內建；顯示解鎖條件） -->
    <section class="rounded-xl border border-border bg-card/40 p-5">
      <div class="mb-2 flex items-center justify-between">
        <h2 class="text-sm font-semibold">{{ $t("caps.title") }}</h2>
        <button
          class="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:opacity-80"
          :disabled="capsLoading"
          @click="loadCaps"
        >
          <RefreshCw :size="13" /> {{ $t("caps.recheck") }}
        </button>
      </div>
      <p class="mb-3 text-xs text-muted-foreground">{{ $t("caps.desc") }}</p>
      <p v-if="capsError" class="mb-2 text-xs text-destructive">{{ $t("caps.unavailable") }}（{{ capsError }}）</p>
      <div v-else-if="caps" class="flex flex-col gap-2 text-sm">
        <!-- MinerU（複雜 PDF 轉換） -->
        <div class="rounded-md border border-border p-3">
          <div class="flex items-center gap-2">
            <span :class="caps.mineru.resolved ? 'text-green-500' : 'text-amber-500'">{{ caps.mineru.resolved ? "✓" : "✗" }}</span>
            <span class="font-medium">{{ $t("caps.mineru") }}</span>
            <span v-if="caps.mineru.resolved" class="font-mono text-[11px] text-muted-foreground">
              {{ caps.mineru.program }}<template v-if="caps.mineru.version"> · {{ caps.mineru.version }}</template>
            </span>
          </div>
          <p class="mt-1 pl-6 text-xs text-muted-foreground">
            {{ caps.mineru.resolved ? $t("caps.mineruOk") : $t("caps.mineruMissing") }}
          </p>
          <p v-if="!caps.mineru.resolved && caps.mineru.hint" class="mt-1 pl-6 text-[11px] text-muted-foreground/80">{{ caps.mineru.hint }}</p>
        </div>
        <!-- 嵌入端點（多模態＝vision；批次旗標＝長 chunk 紀律） -->
        <div class="rounded-md border border-border p-3">
          <div class="flex items-center gap-2">
            <span v-if="!caps.embedding.reachable" class="text-amber-500">?</span>
            <span v-else-if="caps.embedding.vision" class="text-green-500">✓</span>
            <span v-else class="text-amber-500">✗</span>
            <span class="font-medium">{{ $t("caps.embedding") }}</span>
            <span v-if="caps.embedding.reachable" class="font-mono text-[11px] text-muted-foreground">
              {{ caps.embedding.model }} · {{ caps.embedding.dimensions }}d · {{ $t("caps.vision") }}:
              {{ caps.embedding.vision ? "✓" : "✗" }}
            </span>
          </div>
          <p class="mt-1 pl-6 text-xs text-muted-foreground">
            {{ !caps.embedding.reachable ? $t("caps.embeddingDown") : caps.embedding.vision ? $t("caps.embeddingMmOk") : $t("caps.embeddingTextOnly") }}
          </p>
          <p v-if="caps.embedding.reachable && !caps.embedding.long_input_ok" class="mt-1 pl-6 text-xs text-amber-500">
            {{ $t("caps.batchFlag") }}
          </p>
        </div>
        <!-- chat 端點 VLM（生成端讀圖；未配置端點時為未探測） -->
        <div class="rounded-md border border-border p-3">
          <div class="flex items-center gap-2">
            <span v-if="caps.vlm.capable === null" class="text-muted-foreground">–</span>
            <span v-else-if="caps.vlm.capable" class="text-green-500">✓</span>
            <span v-else class="text-amber-500">✗</span>
            <span class="font-medium">{{ $t("caps.vlm") }}</span>
            <span v-if="caps.vlm.model" class="font-mono text-[11px] text-muted-foreground">{{ caps.vlm.model }}</span>
          </div>
          <p class="mt-1 pl-6 text-xs text-muted-foreground">
            {{ caps.vlm.capable === null ? $t("caps.vlmUnknown") : caps.vlm.capable ? $t("caps.vlmOk") : $t("caps.vlmNo") }}
          </p>
        </div>
      </div>
    </section>

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
