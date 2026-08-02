<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Trash2, Pencil, Loader2, Boxes, X } from "lucide-vue-next";
import { useAgentStore } from "@/stores/agent";
import { useBrainsStore } from "@/stores/brains";
import { formatError, type EmployeeTemplate } from "@/lib/tauri";

const { t } = useI18n();
const store = useAgentStore();
const brains = useBrainsStore();

const selectedId = ref<string | null>(null);
const selected = computed(
  () => store.templates.find((x) => x.id === selectedId.value) ?? null,
);

onMounted(async () => {
  await store.ensureAndLoad();
  if (!brains.brains.length) await brains.load();
  if (!selectedId.value && store.templates[0]) selectedId.value = store.templates[0].id;
});

function brainName(id: string): string {
  return brains.brains.find((b) => b.id === id)?.name ?? id;
}

const errorMsg = ref<string | null>(null);

// ── 新增模板 ──
const addOpen = ref(false);
const addName = ref("");
const addBrain = ref("");
const addRole = ref("");
const addBusy = ref(false);
const addError = ref<string | null>(null);

function openAdd() {
  addName.value = "";
  addRole.value = "";
  addBrain.value = brains.activeId ?? brains.brains[0]?.id ?? "";
  addError.value = null;
  addOpen.value = true;
}
async function submitAdd() {
  if (!addName.value.trim() || !addBrain.value) return;
  addBusy.value = true;
  addError.value = null;
  try {
    await store.createTemplate(
      addName.value.trim(),
      addBrain.value,
      addRole.value.trim() || null,
    );
    addOpen.value = false;
  } catch (e) {
    addError.value = formatError(e);
  } finally {
    addBusy.value = false;
  }
}

// ── 重新命名 ──
const renameTarget = ref<EmployeeTemplate | null>(null);
const renameName = ref("");
function openRename(tpl: EmployeeTemplate) {
  renameTarget.value = tpl;
  renameName.value = tpl.name;
  errorMsg.value = null;
}
async function submitRename() {
  if (!renameTarget.value || !renameName.value.trim()) return;
  try {
    await store.renameTemplate(renameTarget.value.id, renameName.value.trim());
    renameTarget.value = null;
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}

// ── 刪除確認 ──
const deleteTarget = ref<EmployeeTemplate | null>(null);
async function confirmDelete() {
  if (!deleteTarget.value) return;
  try {
    const id = deleteTarget.value.id;
    await store.deleteTemplate(id);
    if (selectedId.value === id) selectedId.value = store.templates[0]?.id ?? null;
    deleteTarget.value = null;
  } catch (e) {
    errorMsg.value = formatError(e);
  }
}
</script>

<template>
  <div class="flex h-full w-full">
    <!-- 左：模板清單 -->
    <aside class="flex w-64 shrink-0 flex-col border-r border-border bg-card/30">
      <div class="flex items-center justify-between px-4 py-3">
        <div class="flex items-center gap-2 font-medium">
          <Boxes :size="16" />
          <span>{{ t("templates.title") }}</span>
        </div>
        <button
          type="button"
          class="flex h-7 w-7 items-center justify-center rounded-md bg-primary text-primary-foreground hover:opacity-90"
          :title="t('templates.add')"
          @click="openAdd"
        >
          <Plus :size="15" />
        </button>
      </div>
      <div class="flex-1 overflow-auto px-2 pb-3">
        <button
          v-for="tpl in store.templates"
          :key="tpl.id"
          type="button"
          class="mb-1 flex w-full flex-col items-start gap-0.5 rounded-md px-3 py-2 text-left transition-colors"
          :class="
            selectedId === tpl.id ? 'bg-accent' : 'hover:bg-accent/50'
          "
          @click="selectedId = tpl.id"
        >
          <span class="text-sm font-medium">{{ tpl.name }}</span>
          <span class="text-[11px] text-muted-foreground">{{ brainName(tpl.brain.brain_id) }}</span>
        </button>
        <div
          v-if="!store.templates.length"
          class="px-3 py-6 text-center text-xs text-muted-foreground"
        >
          {{ t("templates.empty") }}
        </div>
      </div>
    </aside>

    <!-- 右：詳情 -->
    <main class="flex min-w-0 flex-1 flex-col overflow-auto p-6">
      <div v-if="selected" class="flex max-w-2xl flex-col gap-4">
        <div class="flex items-start justify-between gap-3">
          <div>
            <h2 class="text-lg font-semibold">{{ selected.name }}</h2>
            <p class="text-xs text-muted-foreground">{{ selected.id }}</p>
          </div>
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="flex items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs hover:bg-accent"
              @click="openRename(selected)"
            >
              <Pencil :size="13" /> {{ t("templates.rename") }}
            </button>
            <button
              type="button"
              class="flex items-center gap-1 rounded-md border border-destructive/40 px-2.5 py-1.5 text-xs text-destructive hover:bg-destructive/10"
              @click="deleteTarget = selected"
            >
              <Trash2 :size="13" /> {{ t("templates.delete") }}
            </button>
          </div>
        </div>
        <dl class="grid grid-cols-[120px_1fr] gap-y-2 text-sm">
          <dt class="text-muted-foreground">{{ t("templates.brain") }}</dt>
          <dd>{{ brainName(selected.brain.brain_id) }}</dd>
          <dt class="text-muted-foreground">{{ t("templates.role") }}</dt>
          <dd>{{ selected.role ?? "—" }}</dd>
          <dt class="text-muted-foreground">{{ t("templates.createdAt") }}</dt>
          <dd class="font-mono text-xs">{{ selected.created_at }}</dd>
        </dl>
      </div>
      <div
        v-else-if="store.loading"
        class="flex items-center gap-2 text-sm text-muted-foreground"
      >
        <Loader2 :size="15" class="animate-spin" /> {{ t("common.loading") }}
      </div>
      <div v-else class="text-sm text-muted-foreground">{{ t("templates.empty") }}</div>
    </main>

    <!-- 新增 modal -->
    <div
      v-if="addOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="addOpen = false"
    >
      <div class="w-full max-w-md rounded-xl border border-border bg-card p-5 shadow-2xl">
        <div class="mb-4 flex items-center justify-between">
          <h3 class="font-semibold">{{ t("templates.add") }}</h3>
          <button type="button" class="text-muted-foreground hover:text-foreground" @click="addOpen = false">
            <X :size="16" />
          </button>
        </div>
        <div class="flex flex-col gap-3">
          <label class="flex flex-col gap-1 text-xs">
            {{ t("templates.name") }}
            <input
              v-model="addName"
              type="text"
              :placeholder="t('templates.namePh')"
              class="rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            />
          </label>
          <label class="flex flex-col gap-1 text-xs">
            {{ t("templates.brain") }}
            <select
              v-model="addBrain"
              class="rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            >
              <option value="" disabled>{{ t("templates.pickBrain") }}</option>
              <option v-for="b in brains.brains" :key="b.id" :value="b.id">{{ b.name }}</option>
            </select>
          </label>
          <label class="flex flex-col gap-1 text-xs">
            {{ t("templates.role") }}
            <input
              v-model="addRole"
              type="text"
              :placeholder="t('templates.rolePh')"
              class="rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
            />
          </label>
          <p v-if="addError" class="text-xs text-destructive">{{ addError }}</p>
        </div>
        <div class="mt-5 flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            @click="addOpen = false"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            type="button"
            :disabled="addBusy || !addName.trim() || !addBrain"
            class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90 disabled:opacity-50"
            @click="submitAdd"
          >
            <Loader2 v-if="addBusy" :size="13" class="animate-spin" />
            {{ t("common.add") }}
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
        <h3 class="mb-3 font-semibold">{{ t("templates.rename") }}</h3>
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

    <!-- 刪除確認 modal -->
    <div
      v-if="deleteTarget"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      @click.self="deleteTarget = null"
    >
      <div class="w-full max-w-sm rounded-xl border border-border bg-card p-5 shadow-2xl">
        <h3 class="mb-2 font-semibold">{{ t("templates.delete") }}</h3>
        <p class="text-sm text-muted-foreground">
          {{ t("templates.confirmDelete", { name: deleteTarget.name }) }}
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
            {{ t("templates.delete") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
