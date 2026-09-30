<script setup lang="ts">
// ---- 委任三層解說（Ch.20 §5.1）——純靜態說明，內容全走 i18n ----
import { useI18n } from "vue-i18n";
import { AlertTriangle } from "lucide-vue-next";

useI18n();

const TIERS = [
  {
    name: "configView.registryTierHuman",
    desc: "configView.registryTierHumanDesc",
    cond: "configView.registryTierHumanCond",
    dot: "bg-amber-500",
  },
  {
    name: "configView.registryTierFenced",
    desc: "configView.registryTierFencedDesc",
    cond: "configView.registryTierFencedCond",
    dot: "bg-blue-500",
  },
  {
    name: "configView.registryTierOwned",
    desc: "configView.registryTierOwnedDesc",
    cond: "configView.registryTierOwnedCond",
    dot: "bg-green-500",
  },
] as const;
</script>

<template>
  <div class="rounded-lg border border-border/60 bg-background/40 p-3">
    <div class="mb-2 text-xs font-medium">{{ $t("configView.registryGuideTitle") }}</div>
    <div class="grid gap-2 sm:grid-cols-3">
      <div v-for="tr in TIERS" :key="tr.name" class="rounded-md border border-border/60 p-2.5">
        <div class="mb-1 flex items-center gap-1.5 text-xs font-semibold">
          <span class="h-2 w-2 shrink-0 rounded-full" :class="tr.dot" />
          {{ $t(tr.name) }}
        </div>
        <p class="text-xs">{{ $t(tr.desc) }}</p>
        <p class="mt-1 text-[11px] text-muted-foreground">{{ $t(tr.cond) }}</p>
      </div>
    </div>
    <p class="mt-2 flex items-start gap-1 text-[11px] text-muted-foreground">
      <AlertTriangle :size="12" class="mt-0.5 shrink-0 text-warning" />
      <span>{{ $t("configView.registryStrictDefault") }}</span>
    </p>
  </div>
</template>
