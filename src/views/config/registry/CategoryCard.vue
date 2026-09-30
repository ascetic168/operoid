<script setup lang="ts">
// ---- 登記表單一類別的手風琴卡片：摘要列＋可展開的完整編輯表單 ----
// 修改直接寫入 v-model 綁定的類別物件（父層持有狀態，存檔才落盤）。
import { computed } from "vue";
import { AlertTriangle, ChevronDown, Trash2 } from "lucide-vue-next";
import type { RegistryCategory } from "@/lib/tauri";

defineProps<{
  isNew?: boolean;
  lapsed?: boolean;
  errors?: { key: string; params?: Record<string, unknown> }[];
}>();
const emit = defineEmits<{ remove: [] }>();

const cat = defineModel<RegistryCategory>("cat", { required: true });
const open = defineModel<boolean>("open", { default: false });

const TIERS = [
  {
    value: "human",
    label: "configView.registryTierHuman",
    desc: "configView.registryTierHumanDesc",
    cond: "configView.registryTierHumanCond",
  },
  {
    value: "fenced",
    label: "configView.registryTierFenced",
    desc: "configView.registryTierFencedDesc",
    cond: "configView.registryTierFencedCond",
  },
  {
    value: "owned",
    label: "configView.registryTierOwned",
    desc: "configView.registryTierOwnedDesc",
    cond: "configView.registryTierOwnedCond",
  },
] as const;
const tierMeta = computed(() => TIERS.find((x) => x.value === cat.value.tier) ?? TIERS[0]);

const TIER_BADGE: Record<RegistryCategory["tier"], string> = {
  human: "bg-amber-500/15 text-amber-500",
  fenced: "bg-blue-500/15 text-blue-500",
  owned: "bg-green-500/15 text-green-500",
};

const BLASTS = [
  { value: "internal", key: "configView.registryBlastInternal" },
  { value: "customer", key: "configView.registryBlastCustomer" },
  { value: "spend", key: "configView.registryBlastSpend" },
  { value: "safety", key: "configView.registryBlastSafety" },
  { value: "rights", key: "configView.registryBlastRights" },
] as const;

// 屆期：RFC3339 ↔ datetime-local（本地時區）
const expiryLocal = computed({
  get: () => {
    if (!cat.value.expiry) return "";
    const d = new Date(cat.value.expiry);
    if (Number.isNaN(d.getTime())) return "";
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  },
  set: (v: string) => {
    if (!v) {
      cat.value.expiry = null;
      return;
    }
    const d = new Date(v);
    cat.value.expiry = Number.isNaN(d.getTime()) ? cat.value.expiry : d.toISOString();
  },
});

// 圍欄工具白名單：逗號分隔字串 ↔ string[] | null（null＝沿用員工允許清單）
const toolsText = computed({
  get: () => cat.value.fences.tools?.join(", ") ?? "",
  set: (v: string) => {
    const parts = v
      .split(/[,，]/)
      .map((s) => s.trim())
      .filter(Boolean);
    cat.value.fences.tools = parts.length ? parts : null;
  },
});
</script>

<template>
  <div>
    <!-- 摘要列（點擊展開／收合） -->
    <button
      class="flex w-full items-center gap-2 rounded-lg border border-border/60 bg-background/40 px-3 py-2 text-left hover:bg-accent/40"
      @click="open = !open"
    >
      <ChevronDown
        :size="14"
        class="shrink-0 text-muted-foreground transition-transform"
        :class="open ? '' : '-rotate-90'"
      />
      <span class="shrink-0 text-sm font-medium">{{ cat.id.trim() || $t("configView.registryUntitled") }}</span>
      <span v-if="cat.description" class="truncate text-xs text-muted-foreground">{{ cat.description }}</span>
      <span class="ml-auto flex shrink-0 items-center gap-1">
        <span
          v-if="lapsed"
          class="flex items-center gap-0.5 rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] text-amber-500"
        >
          <AlertTriangle :size="10" /> {{ $t("configView.registryLapsedBadge") }}
        </span>
        <span v-if="isNew" class="rounded bg-blue-500/15 px-1.5 py-0.5 text-[10px] text-blue-500">
          {{ $t("configView.registryNewBadge") }}
        </span>
        <span class="rounded px-1.5 py-0.5 text-[10px]" :class="TIER_BADGE[cat.tier]">
          {{ $t(tierMeta.label) }}
        </span>
      </span>
    </button>

    <!-- 展開的編輯表單 -->
    <div v-show="open" class="mt-1 rounded-lg border border-border/60 bg-background/40 p-3">
      <div class="grid gap-3 sm:grid-cols-2">
        <label class="block text-xs">
          <span class="text-muted-foreground">{{ $t("configView.registryFieldId") }}</span>
          <input
            v-model="cat.id"
            :placeholder="$t('configView.registryFieldIdPh')"
            class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
          />
        </label>
        <label class="block text-xs">
          <span class="text-muted-foreground">{{ $t("configView.registryFieldTier") }}</span>
          <select v-model="cat.tier" class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs">
            <option v-for="tr in TIERS" :key="tr.value" :value="tr.value">{{ $t(tr.label) }}</option>
          </select>
        </label>

        <p class="text-[11px] text-muted-foreground sm:col-span-2">
          {{ $t(tierMeta.desc) }} {{ $t(tierMeta.cond) }}
        </p>

        <label class="block text-xs sm:col-span-2">
          <span class="text-muted-foreground">{{ $t("configView.registryFieldDesc") }}</span>
          <textarea
            v-model="cat.description"
            :placeholder="$t('configView.registryFieldDescPh')"
            rows="2"
            class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
          />
        </label>

        <!-- 三問 -->
        <div class="sm:col-span-2">
          <div class="mb-1 text-xs text-muted-foreground">{{ $t("configView.registryFieldThreeQuestions") }}</div>
          <div class="grid gap-3 sm:grid-cols-3">
            <label class="flex items-center gap-1.5 text-xs">
              <input v-model="cat.questions.reversible" type="checkbox" class="h-3.5 w-3.5" />
              <span>{{ $t("configView.registryFieldReversible") }}</span>
            </label>
            <label class="block text-xs">
              <span class="text-muted-foreground">{{ $t("configView.registryFieldBlastRadius") }}</span>
              <select
                v-model="cat.questions.blast_radius"
                class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
              >
                <option v-for="b in BLASTS" :key="b.value" :value="b.value">{{ $t(b.key) }}</option>
              </select>
            </label>
            <label class="block text-xs">
              <span class="text-muted-foreground">{{ $t("configView.registryFieldAccountable") }}</span>
              <input
                v-model="cat.questions.accountable"
                :placeholder="$t('configView.registryFieldAccountablePh')"
                class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
              />
            </label>
          </div>
        </div>

        <!-- 放寬層級（fenced/owned）專屬欄位 -->
        <template v-if="cat.tier !== 'human'">
          <label class="block text-xs">
            <span class="text-muted-foreground">{{ $t("configView.registryFieldExpiry") }}</span>
            <input
              v-model="expiryLocal"
              type="datetime-local"
              class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            />
          </label>
          <label class="block text-xs">
            <span class="text-muted-foreground">{{ $t("configView.registryFieldSampling") }}</span>
            <input
              v-model.number="cat.sampling_rate"
              type="number"
              min="0"
              max="1"
              step="0.05"
              class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            />
            <span class="mt-0.5 block text-[11px] text-muted-foreground/70">
              {{ $t("configView.registryFieldSamplingHint") }}
            </span>
          </label>
          <label class="block text-xs">
            <span class="text-muted-foreground">{{ $t("configView.registryFieldEvidence") }}</span>
            <input
              v-model="cat.evidence"
              :placeholder="$t('configView.registryFieldEvidencePh')"
              class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            />
          </label>
          <label v-if="cat.tier === 'owned'" class="block text-xs">
            <span class="text-muted-foreground">{{ $t("configView.registryFieldStop") }}</span>
            <input
              v-model="cat.emergency_stop"
              :placeholder="$t('configView.registryFieldStopPh')"
              class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 text-xs"
            />
          </label>

          <!-- 圍欄 -->
          <div class="rounded-md border border-border/60 p-2.5 sm:col-span-2">
            <div class="mb-2 text-xs font-medium text-muted-foreground">{{ $t("configView.registryFieldFences") }}</div>
            <label class="block text-xs">
              <span class="text-muted-foreground">{{ $t("configView.registryFieldFenceTools") }}</span>
              <input
                v-model="toolsText"
                :placeholder="$t('configView.registryFieldFenceToolsPh')"
                class="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
              />
              <span class="mt-0.5 block text-[11px] text-muted-foreground/70">
                {{ $t("configView.registryFieldFenceToolsHint") }}
              </span>
            </label>
            <div class="mt-2 flex flex-wrap gap-4">
              <label class="flex items-center gap-1.5 text-xs">
                <input v-model="cat.fences.no_outbound" type="checkbox" class="h-3.5 w-3.5" />
                <span>{{ $t("configView.registryFieldNoOutbound") }}</span>
              </label>
              <label class="flex items-center gap-1.5 text-xs">
                <input v-model="cat.fences.template_only" type="checkbox" class="h-3.5 w-3.5" />
                <span>{{ $t("configView.registryFieldTemplateOnly") }}</span>
              </label>
            </div>
          </div>
        </template>

        <!-- 驗證錯誤（存檔前鏡射 V1–V5） -->
        <div
          v-if="errors?.length"
          class="rounded-md border border-destructive/40 bg-destructive/10 p-2 text-xs text-destructive sm:col-span-2"
        >
          <p v-for="(er, i) in errors" :key="i" class="flex items-start gap-1">
            <AlertTriangle :size="12" class="mt-0.5 shrink-0" />
            <span>{{ $t(er.key, er.params ?? {}) }}</span>
          </p>
        </div>

        <div class="flex justify-end sm:col-span-2">
          <button
            class="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs text-destructive hover:bg-destructive/10"
            @click="emit('remove')"
          >
            <Trash2 :size="12" /> {{ $t("common.remove") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
