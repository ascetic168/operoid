<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { BookOpen, ChevronDown, ChevronRight, Loader2 } from "lucide-vue-next";
import { agentFetch } from "@/lib/http";
import { formatError } from "@/lib/tauri";

// ── API 資料形（對應 oserver GET /api/skills；SkillEntry 為 ocore::skills 的序列化）──
interface SkillEntry {
  zone: "draft" | "personal" | "library" | "catalog";
  dir_name: string;
  name: string;
  description: string;
  mount_path: string;
}
interface PersonalArea {
  /** null＝個人模式（單一共用個人區，如 ~/.agents/skills）。 */
  principal: string | null;
  skills: SkillEntry[];
}
interface DraftArea {
  employee_id: string;
  employee_name: string;
  skills: SkillEntry[];
}
interface SkillsPayload {
  mode: "personal" | "enterprise";
  library: SkillEntry[];
  personal_areas: PersonalArea[];
  drafts: DraftArea[];
  catalog: SkillEntry[];
}

const { t } = useI18n();
const library = ref<SkillEntry[]>([]);
const personalAreas = ref<PersonalArea[]>([]);
const drafts = ref<DraftArea[]>([]);
const catalog = ref<SkillEntry[]>([]);
const mode = ref<"personal" | "enterprise">("personal");
const loading = ref(true);
const error = ref<string | null>(null);
const notice = ref<string | null>(null);

async function load(): Promise<void> {
  loading.value = true;
  try {
    const data = await agentFetch<SkillsPayload>("/api/skills");
    library.value = data.library;
    personalAreas.value = data.personal_areas;
    drafts.value = data.drafts;
    catalog.value = data.catalog;
    mode.value = data.mode;
    error.value = null;
  } catch (e) {
    error.value = formatError(e);
  } finally {
    loading.value = false;
  }
}

/** 依 dir_name 前綴分組（`<plugin>/<skill>`；無 `/` 者歸根層）。 */
function groupBy(skills: SkillEntry[]): [string, SkillEntry[]][] {
  const groups = new Map<string, SkillEntry[]>();
  for (const s of skills) {
    const plugin = s.dir_name.includes("/") ? s.dir_name.split("/")[0] : "·";
    if (!groups.has(plugin)) groups.set(plugin, []);
    groups.get(plugin)!.push(s);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
}
const catalogGroups = computed(() => groupBy(catalog.value));
const libraryGroups = computed(() => groupBy(library.value));

// ── 摺疊（區層＋外掛分組層）──
// 區預設：全部展開；外掛分組預設**收合**（書架只看外掛清單＋數量，點開才攤技能）。
const expanded = ref<Record<string, boolean>>({
  catalog: true,
  personal: true,
  drafts: true,
  library: true,
});
function toggleSection(key: string): void {
  expanded.value[key] = !expanded.value[key];
}
const expandedGroups = ref<Set<string>>(new Set());
function toggleGroup(key: string): void {
  const next = new Set(expandedGroups.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedGroups.value = next;
}

// 內容檢視：同時只展開一列（key＝scope 前綴＋mount）。
const openKey = ref("");
const contentText = ref("");
const contentBusy = ref(false);

function keyOf(mount: string, scope: string): string {
  return `${scope}:${mount}`;
}

/** 內容檢視：path 採 read_file 同款掛載語意（catalog/…／personal/…／skills/…／library/…）。 */
async function toggle(mount: string, scope: string, employee?: string): Promise<void> {
  const key = keyOf(mount, scope);
  if (openKey.value === key) {
    openKey.value = "";
    contentText.value = "";
    return;
  }
  openKey.value = key;
  contentText.value = "";
  contentBusy.value = true;
  try {
    const qs = new URLSearchParams({ path: mount });
    if (employee) qs.set("employee", employee);
    const data = await agentFetch<{ path: string; text: string }>(
      `/api/skills/content?${qs.toString()}`,
    );
    contentText.value = data.text;
  } catch (e) {
    contentText.value = formatError(e);
  } finally {
    contentBusy.value = false;
  }
}

// ── 圖書館挑選／個人區移除（人類是唯一有權移動檔案的角色；伺服器代為執行）──

const busyKey = ref("");

async function adopt(name: string): Promise<void> {
  busyKey.value = `adopt:${name}`;
  notice.value = null;
  error.value = null;
  try {
    await agentFetch("/api/skills/adopt", { method: "POST", body: { name } });
    notice.value = t("skills.adopted", { name });
    await load();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    busyKey.value = "";
  }
}

// 兩段確認：第一次點擊進入待確認態，再點一次才真的移除。
const confirmKey = ref("");

async function remove(name: string): Promise<void> {
  if (confirmKey.value !== name) {
    confirmKey.value = name;
    return;
  }
  confirmKey.value = "";
  busyKey.value = `remove:${name}`;
  notice.value = null;
  error.value = null;
  try {
    await agentFetch("/api/skills/remove", { method: "POST", body: { name } });
    notice.value = t("skills.removed", { name });
    await load();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    busyKey.value = "";
  }
}

onMounted(load);
</script>

<template>
  <div class="flex h-full w-full flex-col overflow-hidden">
    <div class="flex items-center gap-2 border-b border-border px-4 py-2.5">
      <BookOpen :size="15" />
      <h2 class="text-sm font-semibold">{{ t("skills.title") }}</h2>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      <p v-if="error" class="mb-3 text-xs text-destructive">{{ error }}</p>
      <p v-if="notice" class="mb-3 text-xs text-emerald-600 dark:text-emerald-400">{{ notice }}</p>
      <div v-if="loading" class="py-10 text-center text-sm text-muted-foreground">
        <Loader2 class="mx-auto mb-2 h-4 w-4 animate-spin" />
      </div>

      <template v-else>
        <!-- 技能圖書館（書架；挑選後入個人區才生效） -->
        <section class="mb-4">
          <button
            class="mb-1 flex w-full items-center gap-1.5 text-left"
            :data-test="'section-catalog'"
            @click="toggleSection('catalog')"
          >
            <ChevronRight v-if="!expanded.catalog" :size="14" class="shrink-0 text-muted-foreground" />
            <ChevronDown v-else :size="14" class="shrink-0 text-muted-foreground" />
            <h3 class="text-xs font-medium text-muted-foreground">{{ t("skills.catalog") }}</h3>
            <span class="text-xs text-muted-foreground">· {{ catalog.length }}</span>
          </button>
          <p v-if="expanded.catalog" class="mb-2 text-xs text-muted-foreground">{{ t("skills.catalogHint") }}</p>
          <template v-if="expanded.catalog">
            <template v-if="catalogGroups.length">
              <div v-for="[plugin, skills] in catalogGroups" :key="plugin" class="mb-3">
                <button
                  class="mb-1 flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-left hover:bg-accent"
                  :data-test="'group-' + plugin"
                  @click="toggleGroup('catalog:' + plugin)"
                >
                  <ChevronRight v-if="!expandedGroups.has('catalog:' + plugin)" :size="13" class="shrink-0 text-muted-foreground" />
                  <ChevronDown v-else :size="13" class="shrink-0 text-muted-foreground" />
                  <h4 class="text-xs font-semibold">{{ plugin }}</h4>
                  <span class="text-xs font-normal text-muted-foreground">· {{ skills.length }}</span>
                </button>
                <div v-if="expandedGroups.has('catalog:' + plugin)" class="flex flex-col gap-1.5 pl-3">
                  <div v-for="s in skills" :key="s.mount_path">
                    <div class="flex items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5">
                      <button
                        class="flex min-w-0 flex-1 items-center gap-2 text-left"
                        :data-test="'skill-' + s.mount_path"
                        @click="toggle(s.mount_path, 'catalog')"
                      >
                        <ChevronRight v-if="openKey !== keyOf(s.mount_path, 'catalog')" :size="13" class="shrink-0 text-muted-foreground" />
                        <ChevronDown v-else :size="13" class="shrink-0 text-muted-foreground" />
                        <span class="shrink-0 text-xs font-medium">{{ s.name }}</span>
                        <span class="truncate text-xs text-muted-foreground" :title="s.description">{{ s.description }}</span>
                      </button>
                      <code class="hidden shrink-0 text-[10px] text-muted-foreground sm:inline">{{ s.mount_path }}</code>
                      <button
                        class="shrink-0 rounded-md border border-border px-2 py-0.5 text-xs hover:bg-accent disabled:opacity-50"
                        :disabled="busyKey === 'adopt:' + s.dir_name"
                        :data-test="'adopt-' + s.dir_name"
                        @click="adopt(s.dir_name)"
                      >
                        {{ t("skills.adopt") }}
                      </button>
                    </div>
                    <pre
                      v-if="openKey === keyOf(s.mount_path, 'catalog')"
                      class="mt-1 max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-muted/40 p-3 text-[11px] leading-relaxed"
                    >{{ contentBusy && !contentText ? "…" : contentText }}</pre>
                  </div>
                </div>
              </div>
            </template>
            <p v-else class="text-xs text-muted-foreground">{{ t("skills.empty") }}</p>
          </template>
        </section>

        <!-- 個人區（可移除） -->
        <section class="mb-4">
          <button
            class="mb-2 flex w-full items-center gap-1.5 text-left"
            data-test="section-personal"
            @click="toggleSection('personal')"
          >
            <ChevronRight v-if="!expanded.personal" :size="14" class="shrink-0 text-muted-foreground" />
            <ChevronDown v-else :size="14" class="shrink-0 text-muted-foreground" />
            <h3 class="text-xs font-medium text-muted-foreground">{{ t("skills.personal") }}</h3>
            <span class="text-xs text-muted-foreground">· {{ personalAreas.reduce((n, a) => n + a.skills.length, 0) }}</span>
          </button>
          <template v-if="expanded.personal">
            <template v-if="personalAreas.length">
              <div v-for="area in personalAreas" :key="area.principal ?? '__personal__'" class="mb-3">
                <h4 v-if="mode === 'enterprise'" class="mb-1.5 text-xs font-semibold">👤 {{ area.principal }}</h4>
                <div class="flex flex-col gap-1.5">
                  <div v-for="s in area.skills" :key="s.mount_path">
                    <div class="flex items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5">
                      <button
                        class="flex min-w-0 flex-1 items-center gap-2 text-left"
                        @click="toggle(s.mount_path, area.principal ?? 'personal')"
                      >
                        <ChevronRight v-if="openKey !== keyOf(s.mount_path, area.principal ?? 'personal')" :size="13" class="shrink-0 text-muted-foreground" />
                        <ChevronDown v-else :size="13" class="shrink-0 text-muted-foreground" />
                        <span class="shrink-0 text-xs font-medium">{{ s.name }}</span>
                        <span class="truncate text-xs text-muted-foreground" :title="s.description">{{ s.description }}</span>
                      </button>
                      <code class="hidden shrink-0 text-[10px] text-muted-foreground sm:inline">{{ s.mount_path }}</code>
                      <button
                        class="shrink-0 rounded-md border border-border px-2 py-0.5 text-xs hover:bg-accent disabled:opacity-50"
                        :disabled="busyKey === 'remove:' + s.dir_name"
                        :data-test="'remove-' + s.dir_name"
                        @click="remove(s.dir_name)"
                      >
                        {{ confirmKey === s.dir_name ? t("skills.confirmRemove") : t("skills.remove") }}
                      </button>
                    </div>
                    <pre
                      v-if="openKey === keyOf(s.mount_path, area.principal ?? 'personal')"
                      class="mt-1 max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-muted/40 p-3 text-[11px] leading-relaxed"
                    >{{ contentBusy && !contentText ? "…" : contentText }}</pre>
                  </div>
                </div>
              </div>
            </template>
            <p v-else class="text-xs text-muted-foreground">{{ t("skills.empty") }}</p>
          </template>
        </section>

        <!-- 員工草稿區 -->
        <section class="mb-4">
          <button
            class="mb-2 flex w-full items-center gap-1.5 text-left"
            data-test="section-drafts"
            @click="toggleSection('drafts')"
          >
            <ChevronRight v-if="!expanded.drafts" :size="14" class="shrink-0 text-muted-foreground" />
            <ChevronDown v-else :size="14" class="shrink-0 text-muted-foreground" />
            <h3 class="text-xs font-medium text-muted-foreground">{{ t("skills.drafts") }}</h3>
            <span class="text-xs text-muted-foreground">· {{ drafts.reduce((n, d) => n + d.skills.length, 0) }}</span>
          </button>
          <template v-if="expanded.drafts">
            <template v-if="drafts.length">
              <div v-for="d in drafts" :key="d.employee_id" class="mb-3">
                <h4 class="mb-1.5 text-xs font-semibold">🛠 {{ d.employee_name }}</h4>
                <div class="flex flex-col gap-1.5">
                  <div v-for="s in d.skills" :key="s.mount_path">
                    <div class="flex items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5">
                      <button
                        class="flex min-w-0 flex-1 items-center gap-2 text-left"
                        @click="toggle(s.mount_path, d.employee_id, d.employee_id)"
                      >
                        <ChevronRight v-if="openKey !== keyOf(s.mount_path, d.employee_id)" :size="13" class="shrink-0 text-muted-foreground" />
                        <ChevronDown v-else :size="13" class="shrink-0 text-muted-foreground" />
                        <span class="shrink-0 text-xs font-medium">{{ s.name }}</span>
                        <span class="truncate text-xs text-muted-foreground" :title="s.description">{{ s.description }}</span>
                      </button>
                      <code class="hidden shrink-0 text-[10px] text-muted-foreground sm:inline">{{ s.mount_path }}</code>
                    </div>
                    <pre
                      v-if="openKey === keyOf(s.mount_path, d.employee_id)"
                      class="mt-1 max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-muted/40 p-3 text-[11px] leading-relaxed"
                    >{{ contentBusy && !contentText ? "…" : contentText }}</pre>
                  </div>
                </div>
              </div>
            </template>
            <p v-else class="text-xs text-muted-foreground">{{ t("skills.empty") }}</p>
          </template>
        </section>

        <!-- 公司共用庫 -->
        <section>
          <button
            class="mb-2 flex w-full items-center gap-1.5 text-left"
            data-test="section-library"
            @click="toggleSection('library')"
          >
            <ChevronRight v-if="!expanded.library" :size="14" class="shrink-0 text-muted-foreground" />
            <ChevronDown v-else :size="14" class="shrink-0 text-muted-foreground" />
            <h3 class="text-xs font-medium text-muted-foreground">{{ t("skills.library") }}</h3>
            <span class="text-xs text-muted-foreground">· {{ library.length }}</span>
          </button>
          <template v-if="expanded.library">
            <template v-if="libraryGroups.length">
              <div v-for="[plugin, skills] in libraryGroups" :key="plugin" class="mb-3">
                <button
                  class="mb-1 flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-left hover:bg-accent"
                  @click="toggleGroup('library:' + plugin)"
                >
                  <ChevronRight v-if="!expandedGroups.has('library:' + plugin)" :size="13" class="shrink-0 text-muted-foreground" />
                  <ChevronDown v-else :size="13" class="shrink-0 text-muted-foreground" />
                  <h4 class="text-xs font-semibold">{{ plugin }}</h4>
                  <span class="text-xs font-normal text-muted-foreground">· {{ skills.length }}</span>
                </button>
                <div v-if="expandedGroups.has('library:' + plugin)" class="flex flex-col gap-1.5 pl-3">
                  <div v-for="s in skills" :key="s.mount_path">
                    <div class="flex items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5">
                      <button
                        class="flex min-w-0 flex-1 items-center gap-2 text-left"
                        @click="toggle(s.mount_path, 'library')"
                      >
                        <ChevronRight v-if="openKey !== keyOf(s.mount_path, 'library')" :size="13" class="shrink-0 text-muted-foreground" />
                        <ChevronDown v-else :size="13" class="shrink-0 text-muted-foreground" />
                        <span class="shrink-0 text-xs font-medium">{{ s.name }}</span>
                        <span class="truncate text-xs text-muted-foreground" :title="s.description">{{ s.description }}</span>
                      </button>
                      <code class="hidden shrink-0 text-[10px] text-muted-foreground sm:inline">{{ s.mount_path }}</code>
                    </div>
                    <pre
                      v-if="openKey === keyOf(s.mount_path, 'library')"
                      class="mt-1 max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-muted/40 p-3 text-[11px] leading-relaxed"
                    >{{ contentBusy && !contentText ? "…" : contentText }}</pre>
                  </div>
                </div>
              </div>
            </template>
            <p v-else class="text-xs text-muted-foreground">{{ t("skills.empty") }}</p>
          </template>
        </section>
      </template>
    </div>
  </div>
</template>
