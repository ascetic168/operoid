<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Terminal, FolderOpen, CheckCircle2, AlertTriangle, Loader2, X } from "lucide-vue-next";
import {
  claudeCodeLaunch,
  claudeCodeStatus,
  formatError,
  type ClaudeStatus,
} from "@/lib/tauri";
import { useConfigStore } from "@/stores/config";
import { useBrainsStore } from "@/stores/brains";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: []; launched: [] }>();
const { t } = useI18n();
const config = useConfigStore();
const brains = useBrainsStore();

const status = ref<ClaudeStatus | null>(null);
const errorMsg = ref("");
const launching = ref(false);

const selectedBrainId = ref<string | null>(null);
const cwd = ref("");
const selectedTerminal = ref<string>("");
const customTemplate = ref<string>("{cmd}");

const availableTerminals = computed(() => status.value?.terminals.filter((x) => x.available) ?? []);
const recent = computed(() => config.app?.recent_claude_cwds ?? []);
const cwdCandidates = computed(() => brains.sources.filter((s) => s.local_path && !s.federated));
const canLaunch = computed(
  () =>
    !!cwd.value &&
    !!status.value?.claude_installed &&
    !!status.value?.gbrain_ready &&
    (selectedTerminal.value !== "custom" || customTemplate.value.trim() !== ""),
);

async function init() {
  errorMsg.value = "";
  status.value = await claudeCodeStatus();
  if (!brains.brains.length) await brains.load();
  selectedBrainId.value = brains.activeId;
  const activeSrc = brains.activeSourceId
    ? brains.sources.find((s) => s.id === brains.activeSourceId)
    : null;
  cwd.value = activeSrc?.local_path ?? cwdCandidates.value[0]?.local_path ?? "";
  const remembered = config.app?.claude_terminal;
  if (
    remembered === "custom" ||
    (remembered && availableTerminals.value.some((x) => x.id === remembered))
  ) {
    selectedTerminal.value = remembered as string;
  } else {
    selectedTerminal.value = availableTerminals.value[0]?.id ?? "custom";
  }
  customTemplate.value = config.app?.claude_terminal_template ?? "{cmd}";
}

watch(
  () => props.open,
  async (o) => {
    if (o) {
      try {
        await init();
      } catch (e) {
        errorMsg.value = formatError(e);
      }
    }
  },
);
watch(selectedBrainId, (id) => {
  if (id) brains.loadSources(id);
});

async function pickCwd() {
  try {
    const d = await openDialog({ directory: true, multiple: false });
    if (typeof d === "string") cwd.value = d;
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}

function clone<T>(x: T): T {
  return JSON.parse(JSON.stringify(x));
}

async function launch() {
  if (!canLaunch.value || !config.app) return;
  launching.value = true;
  errorMsg.value = "";
  try {
    const term = selectedTerminal.value === "custom" ? "custom" : selectedTerminal.value;
    const tpl = selectedTerminal.value === "custom" ? customTemplate.value : null;
    await claudeCodeLaunch(selectedBrainId.value, cwd.value, term, tpl);
    const app = clone(config.app);
    app.recent_claude_cwds = [cwd.value, ...app.recent_claude_cwds.filter((c) => c !== cwd.value)].slice(0, 3);
    app.claude_terminal = term;
    app.claude_terminal_template = tpl;
    await config.saveAppConfig(app);
    emit("launched");
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    launching.value = false;
  }
}
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    @click.self="emit('close')"
  >
    <div class="flex max-h-[88vh] w-full max-w-lg flex-col rounded-xl border border-border bg-card shadow-2xl">
      <!-- header -->
      <div class="flex items-center justify-between border-b border-border px-5 py-3">
        <div class="flex items-center gap-2">
          <Terminal :size="16" />
          <span class="font-medium">{{ t("claude.title") }}</span>
        </div>
        <button class="text-muted-foreground hover:text-foreground" @click="emit('close')">
          <X :size="18" />
        </button>
      </div>

      <!-- 狀態列 -->
      <div class="border-b border-border px-5 py-2 text-xs text-muted-foreground">
        <template v-if="status?.claude_installed">
          <span class="inline-flex items-center gap-1 text-green-500">
            <CheckCircle2 :size="13" /> Claude Code {{ status.claude_version }}
          </span>
          <span class="mx-2">·</span>
          <span v-if="status.gbrain_ready" class="inline-flex items-center gap-1 text-green-500">
            <CheckCircle2 :size="13" /> gbrain
          </span>
          <span v-else class="inline-flex items-center gap-1 text-warning">
            <AlertTriangle :size="13" /> {{ t("claude.statusGbrainMissing") }}
          </span>
        </template>
        <span v-else class="inline-flex items-center gap-1 text-warning">
          <AlertTriangle :size="13" /> {{ t("claude.statusClaudeMissing") }}
        </span>
      </div>

      <!-- body -->
      <div class="flex-1 space-y-4 overflow-auto px-5 py-4 text-sm">
        <!-- 腦 -->
        <div>
          <label class="block text-xs text-muted-foreground">{{ t("claude.brain") }}</label>
          <select
            v-if="brains.brains.length"
            :value="selectedBrainId ?? ''"
            class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            @change="selectedBrainId = ($event.target as HTMLSelectElement).value || null"
          >
            <option v-for="b in brains.brains" :key="b.id" :value="b.id">{{ b.name }}</option>
          </select>
          <div v-else class="mt-1 text-[11px] text-muted-foreground/70">—</div>
          <div class="mt-1 text-[11px] text-muted-foreground/70">{{ t("claude.brainHint") }}</div>
        </div>

        <!-- 終端機 -->
        <div>
          <label class="block text-xs text-muted-foreground">{{ t("claude.terminal") }}</label>
          <select
            :value="selectedTerminal"
            class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            @change="selectedTerminal = ($event.target as HTMLSelectElement).value"
          >
            <option v-for="tm in availableTerminals" :key="tm.id" :value="tm.id">{{ tm.label }}</option>
            <option value="custom">{{ t("claude.terminalCustom") }}</option>
          </select>
          <div v-if="selectedTerminal === 'custom'" class="mt-2">
            <input
              v-model="customTemplate"
              class="w-full rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
              placeholder="{cwd} / {cmd}"
            />
            <div class="mt-1 text-[11px] text-muted-foreground/70">
              {{ t("claude.terminalTemplateHint", { cwd: "{cwd}", cmd: "{cmd}" }) }}
            </div>
          </div>
        </div>

        <!-- 工作目錄 -->
        <div>
          <label class="block text-xs text-muted-foreground">{{ t("claude.cwd") }}</label>
          <div class="mt-1 text-[11px] text-muted-foreground/70">{{ t("claude.cwdHint") }}</div>

          <div class="mt-1 text-[11px] text-muted-foreground">{{ t("claude.cwdFromSource") }}</div>
          <div v-if="cwdCandidates.length" class="mt-1 divide-y divide-border rounded-md border border-border">
            <button
              v-for="s in cwdCandidates"
              :key="s.id"
              type="button"
              :class="[
                'flex w-full items-center gap-2 px-3 py-2 text-left text-xs hover:bg-accent/50',
                cwd === s.local_path ? 'bg-accent/40' : '',
              ]"
              @click="cwd = s.local_path || ''"
            >
              <FolderOpen :size="13" class="shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1">
                <span class="font-medium">{{ s.name }}</span>
                <span class="ml-1 truncate text-muted-foreground">{{ s.local_path }}</span>
              </span>
            </button>
          </div>
          <div v-else class="mt-1 text-[11px] text-muted-foreground/70">—</div>

          <button
            type="button"
            class="mt-2 flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-accent/50"
            @click="pickCwd"
          >
            <FolderOpen :size="13" /> {{ t("claude.cwdBrowse") }}
          </button>

          <div v-if="recent.length" class="mt-3">
            <div class="text-[11px] text-muted-foreground">{{ t("claude.cwdRecent") }}</div>
            <div class="mt-1 divide-y divide-border rounded-md border border-border">
              <button
                v-for="c in recent"
                :key="c"
                type="button"
                :class="[
                  'flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs hover:bg-accent/50',
                  cwd === c ? 'bg-accent/40' : '',
                ]"
                @click="cwd = c"
              >
                <span class="flex-1 truncate font-mono text-muted-foreground">{{ c }}</span>
              </button>
            </div>
          </div>

          <div class="mt-3 text-[11px] text-muted-foreground">
            <code class="break-all">{{ cwd || t("claude.cwdNone") }}</code>
          </div>
        </div>
      </div>

      <!-- error -->
      <div v-if="errorMsg" class="px-5 py-2 text-xs text-destructive">{{ errorMsg }}</div>

      <!-- footer -->
      <div class="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
        <button class="rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent" @click="emit('close')">
          {{ t("claude.cancel") }}
        </button>
        <button
          :disabled="!canLaunch || launching"
          class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
          @click="launch"
        >
          <component :is="launching ? Loader2 : Terminal" :size="14" :class="launching ? 'animate-spin' : ''" />
          {{ t("claude.launch") }}
        </button>
      </div>
    </div>
  </div>
</template>
