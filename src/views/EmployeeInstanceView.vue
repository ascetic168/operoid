<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Loader2, X, UserSquare } from "lucide-vue-next";
import { useAgentStore } from "@/stores/agent";
import { useBrainsStore } from "@/stores/brains";
import { formatError, type Employee } from "@/lib/tauri";
import ContextMenu, { type MenuItem } from "@/components/ContextMenu.vue";

const { t } = useI18n();
const store = useAgentStore();
const brains = useBrainsStore();

onMounted(async () => {
  await store.ensureAndLoad();
  if (!brains.brains.length) await brains.load();
});

const errorMsg = ref<string | null>(null);

function brainName(id: string): string {
  return brains.brains.find((b) => b.id === id)?.name ?? id;
}
function tplName(id: string | null): string {
  const tp = store.templateById(id);
  return tp?.name ?? t("instances.noTemplate");
}
function stateColor(s: string): string {
  switch (s) {
    case "working": return "bg-emerald-500";
    case "sleeping": return "bg-zinc-400";
    case "error": return "bg-destructive";
    case "paused": return "bg-amber-500";
    default: return "bg-sky-500";
  }
}

// ── 右鍵選單 ──
const menu = ref<{ x: number; y: number; emp: Employee } | null>(null);
function openMenu(e: MouseEvent, emp: Employee) {
  menu.value = { x: e.clientX, y: e.clientY, emp };
}
const menuItems = computed<MenuItem[]>(() => [
  { key: "detail", label: t("instances.detail") },
  { key: "message", label: t("instances.message") },
  { key: "rename", label: t("instances.rename") },
  { key: "delete", label: t("instances.delete"), danger: true },
]);
function onMenuSelect(key: string) {
  const emp = menu.value?.emp;
  if (!emp) return;
  if (key === "detail") detailTarget.value = emp;
  else if (key === "message") openMessage(emp);
  else if (key === "rename") openRename(emp);
  else if (key === "delete") deleteTarget.value = emp;
}

// ── 部署新實體 ──
const deployOpen = ref(false);
const deployTpl = ref("");
const deployName = ref("");
const deployBusy = ref(false);
const deployError = ref<string | null>(null);
function openDeploy() {
  deployTpl.value = store.templates[0]?.id ?? "";
  deployName.value = "";
  deployError.value = null;
  deployOpen.value = true;
}
async function submitDeploy() {
  if (!deployTpl.value || !deployName.value.trim()) return;
  deployBusy.value = true;
  deployError.value = null;
  try {
    await store.deployInstance(deployTpl.value, deployName.value.trim());
    deployOpen.value = false;
  } catch (e) {
    deployError.value = formatError(e);
  } finally {
    deployBusy.value = false;
  }
}

// ── 重新命名 ──
const renameTarget = ref<Employee | null>(null);
const renameName = ref("");
function openRename(e: Employee) {
  renameTarget.value = e;
  renameName.value = e.name;
  errorMsg.value = null;
}
async function submitRename() {
  if (!renameTarget.value || !renameName.value.trim()) return;
  try {
    await store.renameEmployee(renameTarget.value.id, renameName.value.trim());
    renameTarget.value = null;
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}

// ── 溝通（訊息 → 員工 Inbox，喚醒）──
const messageTarget = ref<Employee | null>(null);
const messageText = ref("");
const messageBusy = ref(false);
function openMessage(e: Employee) {
  messageTarget.value = e;
  messageText.value = "";
  errorMsg.value = null;
}
async function submitMessage() {
  if (!messageTarget.value || !messageText.value.trim()) return;
  messageBusy.value = true;
  errorMsg.value = null;
  try {
    await store.sendMessage(messageTarget.value.id, messageText.value.trim());
    messageTarget.value = null;
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    messageBusy.value = false;
  }
}

// ── 詳情 ──
const detailTarget = ref<Employee | null>(null);

// ── 刪除確認 ──
const deleteTarget = ref<Employee | null>(null);
async function confirmDelete() {
  if (!deleteTarget.value) return;
  try {
    await store.deleteEmployee(deleteTarget.value.id);
    deleteTarget.value = null;
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}
</script>

<template>
  <div class="flex h-full w-full flex-col overflow-auto p-6">
    <!-- 頁頭 -->
    <div class="mb-4 flex items-center justify-between">
      <div>
        <h1 class="flex items-center gap-2 text-lg font-semibold">
          <UserSquare :size="18" /> {{ t("instances.title") }}
        </h1>
        <p class="text-xs text-muted-foreground">{{ t("instances.subtitle") }}</p>
      </div>
      <button
        type="button"
        :disabled="!store.templates.length"
        class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
        @click="openDeploy"
      >
        <Plus :size="14" /> {{ t("instances.deploy") }}
      </button>
    </div>

    <!-- 卡片網格 -->
    <div
      v-if="store.employees.length || store.loading"
      class="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4"
    >
      <div
        v-for="emp in store.employees"
        :key="emp.id"
        class="cursor-default select-none overflow-hidden rounded-lg border border-border bg-card shadow-sm transition-shadow hover:shadow-md"
        @contextmenu.prevent="openMenu($event, emp)"
        @dblclick="detailTarget = emp"
      >
        <!-- 視窗標題列 -->
        <div class="flex items-center gap-2 border-b border-border bg-accent/40 px-3 py-2">
          <span class="h-2 w-2 shrink-0 rounded-full" :class="stateColor(emp.state)" />
          <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ emp.name }}</span>
        </div>
        <!-- 內容 -->
        <div class="flex flex-col gap-1 px-3 py-2.5 text-[11px] text-muted-foreground">
          <div class="flex justify-between gap-2">
            <span>{{ t("instances.template") }}</span>
            <span class="truncate font-medium text-foreground">{{ tplName(emp.template_id) }}</span>
          </div>
          <div class="flex justify-between gap-2">
            <span>{{ t("instances.brain") }}</span>
            <span class="truncate font-medium text-foreground">{{ brainName(emp.brain.brain_id) }}</span>
          </div>
          <div class="flex justify-between gap-2">
            <span>{{ t("instances.state") }}</span>
            <span class="font-medium text-foreground">{{ emp.state }}</span>
          </div>
        </div>
      </div>
      <div
        v-if="store.loading && !store.employees.length"
        class="col-span-full flex items-center gap-2 py-8 text-sm text-muted-foreground"
      >
        <Loader2 :size="15" class="animate-spin" /> {{ t("common.loading") }}
      </div>
    </div>

    <!-- 空狀態 -->
    <div
      v-else
      class="flex flex-col items-center gap-3 py-16 text-center text-sm text-muted-foreground"
    >
      <UserSquare :size="32" class="opacity-40" />
      <p>{{ t("instances.empty") }}</p>
      <p class="text-xs">{{ t("instances.rightClickHint") }}</p>
    </div>

    <!-- 右鍵選單 -->
    <ContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      @select="onMenuSelect"
      @close="menu = null"
    />

    <!-- 部署 modal -->
    <div
      v-if="deployOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="deployOpen = false"
    >
      <div class="w-full max-w-md rounded-xl border border-border bg-card p-5 shadow-2xl">
        <div class="mb-4 flex items-center justify-between">
          <h3 class="font-semibold">{{ t("instances.deploy") }}</h3>
          <button type="button" class="text-muted-foreground hover:text-foreground" @click="deployOpen = false">
            <X :size="16" />
          </button>
        </div>
        <div class="flex flex-col gap-3">
          <label class="flex flex-col gap-1 text-xs">
            {{ t("instances.template") }}
            <select
              v-model="deployTpl"
              class="rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            >
              <option value="" disabled>{{ t("instances.pickTemplate") }}</option>
              <option v-for="tp in store.templates" :key="tp.id" :value="tp.id">{{ tp.name }}</option>
            </select>
          </label>
          <label class="flex flex-col gap-1 text-xs">
            {{ t("instances.instanceName") }}
            <input
              v-model="deployName"
              type="text"
              :placeholder="t('instances.instanceNamePh')"
              class="rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            />
          </label>
          <p v-if="deployError" class="text-xs text-destructive">{{ deployError }}</p>
        </div>
        <div class="mt-5 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="deployOpen = false"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            :disabled="deployBusy || !deployTpl || !deployName.trim()"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="submitDeploy"
          >
            <Loader2 v-if="deployBusy" :size="13" class="animate-spin" />
            {{ t("instances.deploy") }}
          </button>
        </div>
      </div>
    </div>

    <!-- 重新命名 modal -->
    <div
      v-if="renameTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="renameTarget = null"
    >
      <div class="w-full max-w-sm rounded-xl border border-border bg-card p-5 shadow-2xl">
        <h3 class="mb-3 font-semibold">{{ t("instances.rename") }}</h3>
        <input
          v-model="renameName"
          type="text"
          class="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        />
        <p v-if="errorMsg" class="mt-2 text-xs text-destructive">{{ errorMsg }}</p>
        <div class="mt-4 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="renameTarget = null"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            class="rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
            @click="submitRename"
          >
            {{ t("common.save") }}
          </button>
        </div>
      </div>
    </div>

    <!-- 溝通 modal -->
    <div
      v-if="messageTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="messageTarget = null"
    >
      <div class="w-full max-w-sm rounded-xl border border-border bg-card p-5 shadow-2xl">
        <h3 class="mb-1 font-semibold">{{ t("instances.messageTitle") }}</h3>
        <p class="mb-3 text-xs text-muted-foreground">{{ messageTarget.name }}</p>
        <textarea
          v-model="messageText"
          rows="4"
          :placeholder="t('instances.messagePh')"
          class="w-full resize-none rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        />
        <p v-if="errorMsg" class="mt-2 text-xs text-destructive">{{ errorMsg }}</p>
        <div class="mt-4 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="messageTarget = null"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            :disabled="messageBusy || !messageText.trim()"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="submitMessage"
          >
            <Loader2 v-if="messageBusy" :size="13" class="animate-spin" />
            {{ t("instances.send") }}
          </button>
        </div>
      </div>
    </div>

    <!-- 詳情 modal -->
    <div
      v-if="detailTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="detailTarget = null"
    >
      <div class="w-full max-w-md rounded-xl border border-border bg-card shadow-2xl">
        <div class="flex items-center justify-between border-b border-border px-5 py-3">
          <h3 class="flex items-center gap-2 font-semibold">
            <span class="h-2 w-2 rounded-full" :class="stateColor(detailTarget.state)" />
            {{ detailTarget.name }}
          </h3>
          <button type="button" class="text-muted-foreground hover:text-foreground" @click="detailTarget = null">
            <X :size="16" />
          </button>
        </div>
        <div class="px-5 py-4">
          <dl class="grid grid-cols-[110px_1fr] gap-y-2 text-sm">
            <dt class="text-muted-foreground">ID</dt>
            <dd class="font-mono text-xs">{{ detailTarget.id }}</dd>
            <dt class="text-muted-foreground">{{ t("instances.template") }}</dt>
            <dd>{{ tplName(detailTarget.template_id) }}</dd>
            <dt class="text-muted-foreground">{{ t("instances.brain") }}</dt>
            <dd>{{ brainName(detailTarget.brain.brain_id) }}</dd>
            <dt class="text-muted-foreground">{{ t("instances.state") }}</dt>
            <dd>{{ detailTarget.state }}</dd>
            <dt class="text-muted-foreground">{{ t("templates.createdAt") }}</dt>
            <dd class="font-mono text-xs">{{ detailTarget.created_at }}</dd>
          </dl>
        </div>
      </div>
    </div>

    <!-- 刪除確認 modal -->
    <div
      v-if="deleteTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="deleteTarget = null"
    >
      <div class="w-full max-w-sm rounded-xl border border-border bg-card p-5 shadow-2xl">
        <h3 class="mb-2 font-semibold">{{ t("instances.delete") }}</h3>
        <p class="text-sm text-muted-foreground">
          {{ t("instances.confirmDelete", { name: deleteTarget.name }) }}
        </p>
        <p v-if="errorMsg" class="mt-2 text-xs text-destructive">{{ errorMsg }}</p>
        <div class="mt-4 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="deleteTarget = null"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            class="rounded-md bg-destructive px-3 py-1.5 text-xs text-destructive-foreground hover:opacity-90"
            @click="confirmDelete"
          >
            {{ t("instances.delete") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
