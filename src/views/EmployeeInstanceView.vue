<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Plus, Loader2, X, UserSquare } from "lucide-vue-next";
import { useAgentStore } from "@/stores/agent";
import { useBrainsStore } from "@/stores/brains";
import { agentWatch, formatError, type Employee, type WatchSnapshot } from "@/lib/tauri";
import ContextMenu, { type MenuItem } from "@/components/ContextMenu.vue";

const { t } = useI18n();
const router = useRouter();
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
  { key: "chat", label: t("instances.chat") },
  { key: "detail", label: t("instances.detail") },
  { key: "delegate", label: t("instances.delegate") },
  { key: "watch", label: t("instances.watch") },
  { key: "rename", label: t("instances.rename") },
  { key: "delete", label: t("instances.delete"), danger: true },
]);
function onMenuSelect(key: string) {
  const emp = menu.value?.emp;
  if (!emp) return;
  if (key === "chat") router.push({ name: "employee-chat", params: { id: emp.id } });
  else if (key === "detail") detailTarget.value = emp;
  else if (key === "delegate") openDelegate(emp);
  else if (key === "watch") openWatch(emp);
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

// ── 監看（即時觀察：狀態／工作／產出／事件歷程，每 ~1.5s 輪詢）──
const watchTarget = ref<Employee | null>(null);
const watchData = ref<WatchSnapshot | null>(null);
const eventsEl = ref<HTMLElement | null>(null);
let watchTimer: ReturnType<typeof setInterval> | null = null;

async function pollWatch() {
  if (!watchTarget.value) return;
  try {
    watchData.value = await agentWatch(watchTarget.value.id);
    await nextTick();
    if (eventsEl.value) eventsEl.value.scrollTop = eventsEl.value.scrollHeight;
  } catch {
    // 唯讀觀察：靜默（如短暂鎖定／錯誤），下個 tick 再試。
  }
}
function openWatch(e: Employee) {
  watchTarget.value = e;
  watchData.value = null;
  pollWatch();
  if (watchTimer) clearInterval(watchTimer);
  watchTimer = setInterval(pollWatch, 1500);
}
function closeWatch() {
  watchTarget.value = null;
  watchData.value = null;
  if (watchTimer) {
    clearInterval(watchTimer);
    watchTimer = null;
  }
}
onUnmounted(() => {
  if (watchTimer) clearInterval(watchTimer);
});

// ── 交辦（建立承諾，後端立即喚醒員工自主跑）──
const delegateTarget = ref<Employee | null>(null);
const delegateTitle = ref("");
const delegateCondition = ref("");
const delegateBusy = ref(false);
function openDelegate(e: Employee) {
  delegateTarget.value = e;
  delegateTitle.value = "";
  delegateCondition.value = "";
  errorMsg.value = null;
}
async function submitDelegate() {
  if (!delegateTarget.value || !delegateTitle.value.trim() || !delegateCondition.value.trim()) return;
  delegateBusy.value = true;
  errorMsg.value = null;
  try {
    await store.createCommitment(
      delegateTarget.value.id,
      delegateTitle.value.trim(),
      delegateCondition.value.trim(),
    );
    delegateTarget.value = null;
  } catch (e) {
    errorMsg.value = formatError(e);
  } finally {
    delegateBusy.value = false;
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

    <!-- 監看 modal（輪詢即時觀察）-->
    <div
      v-if="watchTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="closeWatch"
    >
      <div class="flex max-h-[85vh] w-full max-w-lg flex-col rounded-xl border border-border bg-card shadow-2xl">
        <div class="flex items-center justify-between border-b border-border px-5 py-3">
          <h3 class="flex items-center gap-2 font-semibold">
            <span class="h-2 w-2 rounded-full" :class="watchData ? stateColor(watchData.employee.state) : 'bg-zinc-400'" />
            {{ watchTarget.name }}
            <span class="text-xs font-normal text-muted-foreground">{{ watchData ? watchData.employee.state : "…" }}</span>
          </h3>
          <button type="button" class="text-muted-foreground hover:text-foreground" @click="closeWatch">
            <X :size="16" />
          </button>
        </div>
        <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-5 py-4 text-sm">
          <div>
            <div class="mb-1 text-xs font-medium text-muted-foreground">{{ t("instances.watchCommitments") }}</div>
            <div v-if="watchData && watchData.commitments.length" class="flex flex-col gap-0.5">
              <div v-for="c in watchData.commitments" :key="(c as any).id" class="truncate">
                • {{ (c as any).title }} <span class="text-xs text-muted-foreground">[{{ (c as any).status }}]</span>
              </div>
            </div>
            <div v-else class="text-xs text-muted-foreground">{{ t("instances.watchNone") }}</div>
          </div>
          <div>
            <div class="mb-1 text-xs font-medium text-muted-foreground">{{ t("instances.watchTasks") }}</div>
            <div v-if="watchData && watchData.tasks.length" class="flex flex-col gap-0.5">
              <div v-for="tk in watchData.tasks" :key="(tk as any).id" class="truncate">
                ▸ {{ (tk as any).objective }} <span class="text-xs text-muted-foreground">[{{ (tk as any).status }}]</span>
              </div>
            </div>
            <div v-else class="text-xs text-muted-foreground">{{ t("instances.watchNone") }}</div>
          </div>
          <div>
            <div class="mb-1 text-xs font-medium text-muted-foreground">{{ t("instances.watchArtifacts") }}</div>
            <div v-if="watchData && watchData.artifacts.length" class="flex flex-col gap-2">
              <div v-for="(a, idx) in watchData.artifacts" :key="(a as any).id">
                <div class="truncate text-xs font-medium">◇ {{ (a as any).title }}</div>
                <div
                  v-if="idx === 0 && (a as any).content"
                  class="mt-1 max-h-44 overflow-y-auto whitespace-pre-wrap rounded-md border border-border bg-background p-2 text-[11px] leading-relaxed text-foreground"
                >{{ (a as any).content }}</div>
              </div>
            </div>
            <div v-else class="text-xs text-muted-foreground">{{ t("instances.watchNone") }}</div>
          </div>
          <div class="flex min-h-[90px] flex-1 flex-col rounded-md border border-border bg-background">
            <div class="border-b border-border px-2 py-1 text-xs text-muted-foreground">{{ t("instances.watchEvents") }}</div>
            <div ref="eventsEl" class="min-h-0 flex-1 overflow-y-auto p-2 font-mono text-[11px] leading-relaxed">
              <div v-if="watchData && watchData.events.length === 0" class="text-muted-foreground">{{ t("instances.watchNone") }}</div>
              <div v-for="ev in watchData?.events ?? []" :key="ev.id">
                <span class="text-muted-foreground">{{ ev.created_at.slice(11, 19) }}</span>
                <span
                  class="ml-1 font-medium"
                  :class="{
                    'text-emerald-500': ev.kind === 'satisfied' || ev.kind === 'wake' || ev.kind === 'artifact',
                    'text-amber-500': ev.kind === 'stalled',
                    'text-destructive': ev.kind === 'errored',
                  }"
                >{{ ev.kind }}</span>
                <span class="ml-1 text-muted-foreground">{{ ev.detail }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 交辦 modal（建立承諾，立即喚醒）-->
    <div
      v-if="delegateTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="delegateTarget = null"
    >
      <div class="w-full max-w-md rounded-xl border border-border bg-card p-5 shadow-2xl">
        <h3 class="mb-1 font-semibold">{{ t("instances.delegate") }}</h3>
        <p class="mb-3 text-xs text-muted-foreground">{{ delegateTarget.name }}</p>
        <div class="flex flex-col gap-3">
          <label class="flex flex-col gap-1 text-xs">
            {{ t("instances.delegateTitle") }}
            <input
              v-model="delegateTitle"
              type="text"
              :placeholder="t('instances.delegateTitlePh')"
              class="rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            />
          </label>
          <label class="flex flex-col gap-1 text-xs">
            {{ t("instances.delegateCondition") }}
            <textarea
              v-model="delegateCondition"
              rows="3"
              :placeholder="t('instances.delegateConditionPh')"
              class="resize-none rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            />
          </label>
          <p class="text-xs text-muted-foreground">{{ t("instances.delegateHint") }}</p>
          <p v-if="errorMsg" class="text-xs text-destructive">{{ errorMsg }}</p>
        </div>
        <div class="mt-4 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="delegateTarget = null"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            :disabled="delegateBusy || !delegateTitle.trim() || !delegateCondition.trim()"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="submitDelegate"
          >
            <Loader2 v-if="delegateBusy" :size="13" class="animate-spin" />
            {{ t("instances.delegateCreate") }}
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
