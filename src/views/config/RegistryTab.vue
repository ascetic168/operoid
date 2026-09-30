<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { AlertTriangle, CheckCircle2, ChevronDown, Plus, RefreshCw, Save, X } from "lucide-vue-next";
import {
  formatError,
  type ActionRegistry,
  type RegistryCategory,
  agentRegistryLoad,
  agentRegistrySave,
} from "@/lib/tauri";
import TierGuide from "@/views/config/registry/TierGuide.vue";
import CategoryCard from "@/views/config/registry/CategoryCard.vue";

const { t } = useI18n();

// ---- 動作類別登記表（Ch.20 §5——畫線權的畫布：哪些承諾類別可免個案核可自動啟用）----
// 結構化表單：本地 drafts 為編輯狀態，存檔時序列化送 POST /api/registry（後端不變）。
interface CatError {
  key: string;
  params?: Record<string, unknown>;
}
interface Draft {
  key: number;
  open: boolean;
  isNew: boolean;
  cat: RegistryCategory;
}

let keySeq = 0;
const drafts = ref<Draft[]>([]);
const cardErrors = ref<Record<number, CatError[]>>({});
const registryVersion = ref<number | null>(null);
const snapPath = ref("");
const registryDivergence = ref<{ samples: number; misclassified: number; ratio_30d: number } | null>(null);
const registryLoadError = ref<string | null>(null);
const registryError = ref<string | null>(null);
const registrySaved = ref(false);

// 全域設定（fuse_keywords / budget / divergence_threshold）——用獨立 ref 便於留空＝停用
const fuseKeywords = ref<string[]>([]);
const fuseInput = ref("");
const budgetInput = ref("");
const divergenceInput = ref("");

// 原始 JSON（進階收合區）
const rawOpen = ref(false);
const rawText = ref("");
const rawError = ref<string | null>(null);
const rawApplied = ref(false);

// ---- 正規化：缺欄位補 Rust serde 預設值，讓手改壞的檔案也能載入修復 ----
function normalizeCategory(raw: unknown): RegistryCategory {
  const c = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const q = (typeof c.questions === "object" && c.questions !== null ? c.questions : {}) as Record<string, unknown>;
  const f = (typeof c.fences === "object" && c.fences !== null ? c.fences : {}) as Record<string, unknown>;
  return {
    id: typeof c.id === "string" ? c.id : "",
    description: typeof c.description === "string" ? c.description : "",
    questions: {
      reversible: q.reversible === true,
      blast_radius: typeof q.blast_radius === "string" ? q.blast_radius : "internal",
      accountable: typeof q.accountable === "string" ? q.accountable : "",
    },
    tier: c.tier === "fenced" || c.tier === "owned" ? c.tier : "human",
    fences: {
      tools: Array.isArray(f.tools) ? f.tools.filter((x): x is string => typeof x === "string") : null,
      no_outbound: f.no_outbound !== false,
      template_only: f.template_only === true,
    },
    emergency_stop: typeof c.emergency_stop === "string" ? c.emergency_stop : null,
    expiry: typeof c.expiry === "string" ? c.expiry : null,
    sampling_rate: typeof c.sampling_rate === "number" && Number.isFinite(c.sampling_rate) ? c.sampling_rate : 1,
    evidence: typeof c.evidence === "string" ? c.evidence : null,
    updated_at: typeof c.updated_at === "string" ? c.updated_at : null,
  };
}

function normalizeRegistry(raw: unknown): ActionRegistry {
  const r = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  return {
    version: typeof r.version === "number" ? r.version : 0,
    fuse_keywords: Array.isArray(r.fuse_keywords)
      ? r.fuse_keywords.filter((x): x is string => typeof x === "string")
      : [],
    weekly_proposal_budget: typeof r.weekly_proposal_budget === "number" ? r.weekly_proposal_budget : null,
    divergence_threshold:
      typeof r.divergence_threshold === "number" && Number.isFinite(r.divergence_threshold)
        ? r.divergence_threshold
        : 0.2,
    categories: Array.isArray(r.categories) ? r.categories.map(normalizeCategory) : [],
  };
}

function blankCategory(): RegistryCategory {
  return {
    id: "",
    description: "",
    questions: { reversible: false, blast_radius: "internal", accountable: "" },
    tier: "human",
    fences: { tools: null, no_outbound: true, template_only: false },
    emergency_stop: null,
    expiry: null,
    sampling_rate: 1,
    evidence: null,
    updated_at: null,
  };
}

function hydrate(reg: ActionRegistry) {
  const norm = normalizeRegistry(reg);
  keySeq = 0;
  drafts.value = norm.categories.map((c) => ({ key: ++keySeq, open: false, isNew: false, cat: c }));
  fuseKeywords.value = norm.fuse_keywords;
  budgetInput.value = norm.weekly_proposal_budget === null ? "" : String(norm.weekly_proposal_budget);
  divergenceInput.value = String(norm.divergence_threshold);
  cardErrors.value = {};
}

const lapsedIds = computed(() => {
  const now = Date.now();
  return new Set(
    drafts.value
      .filter((d) => d.cat.tier !== "human" && d.cat.expiry && new Date(d.cat.expiry).getTime() <= now)
      .map((d) => d.cat.id.trim()),
  );
});

// ---- 類別列表操作 ----
function addCategory() {
  // 新類別預設人裁決層（從嚴預設），展開待填
  drafts.value.push({ key: ++keySeq, open: true, isNew: true, cat: blankCategory() });
}
function removeCategory(key: number) {
  drafts.value = drafts.value.filter((d) => d.key !== key);
  delete cardErrors.value[key];
}

function addFuse() {
  const parts = fuseInput.value
    .split(/[,，\s]+/)
    .map((s) => s.trim())
    .filter(Boolean);
  for (const p of parts) if (!fuseKeywords.value.includes(p)) fuseKeywords.value.push(p);
  fuseInput.value = "";
}

// ---- 序列化與驗證（客戶端鏡射 ocore validate_registry 的 V1–V5）----
function buildRegistry(): ActionRegistry {
  const budgetRaw = budgetInput.value.trim();
  const budget = budgetRaw === "" ? null : Number(budgetRaw);
  const div = Number(divergenceInput.value);
  return {
    version: registryVersion.value ?? 0,
    fuse_keywords: [...fuseKeywords.value],
    weekly_proposal_budget: budget !== null && Number.isInteger(budget) && budget >= 0 ? budget : null,
    divergence_threshold: Number.isFinite(div) ? div : 0.2,
    categories: drafts.value.map((d) => ({ ...d.cat, id: d.cat.id.trim() })),
  };
}

function validateCategory(c: RegistryCategory, seen: Set<string>): CatError[] {
  const errs: CatError[] = [];
  const id = c.id.trim();
  if (!id) errs.push({ key: "configView.registryErrIdEmpty" });
  else if (seen.has(id)) errs.push({ key: "configView.registryErrIdDup", params: { id } });
  else seen.add(id);
  if (!(typeof c.sampling_rate === "number" && c.sampling_rate >= 0 && c.sampling_rate <= 1))
    errs.push({ key: "configView.registryErrSampling" });
  if (c.tier !== "human") {
    if (!c.questions.accountable.trim()) errs.push({ key: "configView.registryErrAccountable" });
    if (!c.evidence || !c.evidence.trim()) errs.push({ key: "configView.registryErrEvidence" });
    const d = c.expiry ? new Date(c.expiry) : null;
    if (!d || Number.isNaN(d.getTime()) || d.getTime() <= Date.now())
      errs.push({ key: "configView.registryErrExpiry" });
    if (c.tier === "owned" && (!c.emergency_stop || !c.emergency_stop.trim()))
      errs.push({ key: "configView.registryErrStop" });
  }
  return errs;
}

async function loadRegistry() {
  registryLoadError.value = null;
  registryError.value = null;
  registrySaved.value = false;
  try {
    const snap = await agentRegistryLoad();
    snapPath.value = snap.path;
    registryVersion.value = snap.registry?.version ?? null;
    registryDivergence.value = snap.divergence ?? null;
    hydrate(
      snap.registry ?? {
        version: 0,
        fuse_keywords: [],
        weekly_proposal_budget: null,
        divergence_threshold: 0.2,
        categories: [],
      },
    );
  } catch (e) {
    registryLoadError.value = formatError(e);
  }
}

async function saveRegistry() {
  registryError.value = null;
  registrySaved.value = false;
  rawApplied.value = false;

  const inputErrs: string[] = [];
  const budgetRaw = budgetInput.value.trim();
  if (budgetRaw !== "" && !/^\d+$/.test(budgetRaw)) inputErrs.push(t("configView.registryErrBudget"));
  const div = Number(divergenceInput.value);
  if (!Number.isFinite(div) || div < 0 || div > 1) inputErrs.push(t("configView.registryErrDivergence"));

  const errs = new Map<number, CatError[]>();
  const seen = new Set<string>();
  for (const d of drafts.value) {
    const list = validateCategory(d.cat, seen);
    if (list.length) errs.set(d.key, list);
  }
  const cardErrObj: Record<number, CatError[]> = {};
  for (const [k, v] of errs) cardErrObj[k] = v;
  cardErrors.value = cardErrObj;
  // 有錯的卡片自動展開，方便修正
  if (errs.size) {
    for (const k of errs.keys()) {
      const d = drafts.value.find((x) => x.key === k);
      if (d) d.open = true;
    }
  }

  const parts: string[] = [];
  if (errs.size) parts.push(t("configView.registryErrFixFirst", { n: errs.size }));
  parts.push(...inputErrs);
  if (parts.length) {
    registryError.value = parts.join(" ");
    return;
  }

  try {
    const r = await agentRegistrySave(JSON.stringify(buildRegistry()));
    hydrate(r.registry);
    registryVersion.value = r.registry.version;
    if (rawOpen.value) syncRaw();
    registrySaved.value = true;
  } catch (e) {
    registryError.value = formatError(e);
  }
}

// ---- 原始 JSON（進階收合區）：由表單生成、可編輯後套用回表單；存檔一律以表單為準 ----
function toggleRaw() {
  rawOpen.value = !rawOpen.value;
  if (rawOpen.value) syncRaw();
}
function syncRaw() {
  rawText.value = JSON.stringify(buildRegistry(), null, 2);
  rawError.value = null;
  rawApplied.value = false;
}
function applyRaw() {
  rawError.value = null;
  rawApplied.value = false;
  try {
    hydrate(normalizeRegistry(JSON.parse(rawText.value)));
    rawApplied.value = true;
  } catch (e) {
    rawError.value = t("configView.jsonParseFail", { e: String(e) });
  }
}

onMounted(loadRegistry);
</script>

<template>
  <section class="rounded-xl border border-border bg-card/40 p-5">
    <h2 class="mb-2 text-sm font-semibold">{{ $t("configView.registrySection") }}</h2>
    <p class="mb-3 text-xs text-muted-foreground">{{ $t("configView.registryDesc") }}</p>

    <p v-if="lapsedIds.size" class="mb-3 flex items-center gap-1 text-xs text-amber-500">
      <AlertTriangle :size="13" />
      {{ $t("configView.registryLapsedWarn", { n: lapsedIds.size }) }}
    </p>
    <p v-if="registryDivergence && registryDivergence.samples" class="mb-3 text-xs text-muted-foreground">
      {{
        $t("configView.registryDivergence", {
          ratio: Math.round((registryDivergence.ratio_30d ?? 0) * 100),
          n: registryDivergence.samples,
        })
      }}
    </p>
    <p v-if="registryLoadError" class="text-xs text-destructive">
      {{ registryLoadError }}
      <button
        class="ml-2 inline-flex items-center gap-1 rounded border border-border px-2 py-0.5 hover:opacity-80"
        @click="loadRegistry"
      >
        <RefreshCw :size="12" /> {{ $t("configView.registryReload") }}
      </button>
    </p>

    <template v-else>
      <!-- 委任三層解說 -->
      <TierGuide class="mb-3" />

      <!-- 全域設定（保險絲／預算／分歧門檻） -->
      <div class="mb-3 rounded-lg border border-border/60 bg-background/40 p-3 text-sm">
        <div class="mb-2 font-medium">{{ $t("configView.registryGlobalTitle") }}</div>

        <div class="mb-3">
          <div class="mb-1 text-xs text-muted-foreground">{{ $t("configView.registryFuseLabel") }}</div>
          <div class="flex flex-wrap items-center gap-1.5">
            <span
              v-for="(kw, i) in fuseKeywords"
              :key="kw + i"
              class="flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 text-xs"
            >
              {{ kw }}
              <button
                class="text-muted-foreground hover:text-destructive"
                :title="$t('common.remove')"
                @click="fuseKeywords.splice(i, 1)"
              >
                <X :size="11" />
              </button>
            </span>
            <input
              v-model="fuseInput"
              :placeholder="$t('configView.registryFusePlaceholder')"
              class="w-44 rounded-md border border-border bg-background px-2 py-1 text-xs"
              @keydown.enter.prevent="addFuse"
            />
            <button
              class="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-accent"
              @click="addFuse"
            >
              <Plus :size="12" /> {{ $t("common.add") }}
            </button>
          </div>
          <p class="mt-1 text-[11px] text-muted-foreground/70">{{ $t("configView.registryFuseHint") }}</p>
        </div>

        <div class="grid gap-3 sm:grid-cols-2">
          <label class="block text-xs">
            <span class="text-muted-foreground">{{ $t("configView.registryBudgetLabel") }}</span>
            <input
              v-model="budgetInput"
              type="number"
              min="0"
              step="1"
              class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            />
            <span class="mt-0.5 block text-[11px] text-muted-foreground/70">
              {{ $t("configView.registryBudgetHint") }}
            </span>
          </label>
          <label class="block text-xs">
            <span class="text-muted-foreground">{{ $t("configView.registryDivergenceLabel") }}</span>
            <input
              v-model="divergenceInput"
              type="number"
              min="0"
              max="1"
              step="0.05"
              class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            />
            <span class="mt-0.5 block text-[11px] text-muted-foreground/70">
              {{ $t("configView.registryDivergenceHint") }}
            </span>
          </label>
        </div>
      </div>

      <!-- 類別列表（手風琴卡片） -->
      <div class="mb-3">
        <div class="mb-2 flex flex-wrap items-center justify-between gap-2">
          <span class="text-sm font-medium">{{ $t("configView.registryCatTitle") }}（{{ drafts.length }}）</span>
          <button
            class="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-accent"
            @click="addCategory"
          >
            <Plus :size="12" /> {{ $t("configView.registryAdd") }}
          </button>
        </div>
        <p class="mb-2 text-xs text-muted-foreground">{{ $t("configView.registryCatDesc") }}</p>
        <div class="flex flex-col gap-2">
          <CategoryCard
            v-for="d in drafts"
            :key="d.key"
            v-model:cat="d.cat"
            v-model:open="d.open"
            :is-new="d.isNew"
            :lapsed="lapsedIds.has(d.cat.id.trim())"
            :errors="cardErrors[d.key]"
            @remove="removeCategory(d.key)"
          />
        </div>
        <p
          v-if="!drafts.length"
          class="rounded-lg border border-dashed border-border p-4 text-center text-xs text-muted-foreground"
        >
          {{ $t("configView.registryEmpty") }}
        </p>
      </div>

      <!-- 存檔列 -->
      <div class="flex flex-wrap items-center gap-3">
        <button
          class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
          @click="saveRegistry"
        >
          <Save :size="14" /> {{ $t("common.save") }}
        </button>
        <button
          class="flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs hover:opacity-80"
          @click="loadRegistry"
        >
          <RefreshCw :size="13" /> {{ $t("configView.registryReload") }}
        </button>
        <span v-if="registryVersion !== null" class="text-xs text-muted-foreground">v{{ registryVersion }}</span>
        <span v-if="registryError" class="text-xs text-destructive">{{ registryError }}</span>
        <span v-else-if="registrySaved" class="flex items-center gap-1 text-xs text-green-500">
          <CheckCircle2 :size="13" /> {{ $t("configView.registrySaved") }}
        </span>
      </div>

      <!-- 原始 JSON（進階收合區） -->
      <div class="mt-4 border-t border-border pt-3">
        <button class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" @click="toggleRaw">
          <ChevronDown :size="13" class="transition-transform" :class="rawOpen ? '' : '-rotate-90'" />
          {{ $t("configView.registryRawToggle") }}
        </button>
        <div v-if="rawOpen" class="mt-2">
          <p class="mb-1 break-all text-[11px] text-muted-foreground/70">
            {{ $t("configView.registryRawHint") }}
            <code>{{ snapPath }}</code>
          </p>
          <textarea
            v-model="rawText"
            class="h-72 w-full rounded-md border border-border bg-background p-2 font-mono text-xs"
            spellcheck="false"
          />
          <div class="mt-2 flex flex-wrap items-center gap-2">
            <button
              class="rounded-md border border-border px-2 py-1 text-xs hover:bg-accent"
              @click="applyRaw"
            >
              {{ $t("configView.registryRawApply") }}
            </button>
            <button
              class="rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
              @click="syncRaw"
            >
              {{ $t("configView.registryRawResync") }}
            </button>
            <span v-if="rawError" class="text-xs text-destructive">{{ rawError }}</span>
            <span v-else-if="rawApplied" class="text-xs text-green-500">
              {{ $t("configView.registryRawApplied") }}
            </span>
          </div>
        </div>
      </div>
    </template>
  </section>
</template>
