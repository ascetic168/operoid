<script setup lang="ts">
import { RouterLink, RouterView } from "vue-router";
import { useConfigStore } from "@/stores/config";

const config = useConfigStore();
if (!config.ready && !config.loading) config.load();

// TAB 導覽：路由即 TAB（可深連結），active 慣例沿用 FactoriesView 類型 chips。
const tabs = [
  { to: "/config/app", labelKey: "configView.tabs.app" },
  { to: "/config/models", labelKey: "configView.tabs.models" },
  { to: "/config/services", labelKey: "configView.tabs.services" },
  { to: "/config/registry", labelKey: "configView.tabs.registry" },
  { to: "/config/advanced", labelKey: "configView.tabs.advanced" },
] as const;
</script>

<template>
  <div class="flex h-full flex-col overflow-y-auto p-6">
    <header class="mb-6">
      <h1 class="text-xl font-semibold">{{ $t("configView.title") }}</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        {{ $t("configView.desc") }}
      </p>
    </header>

    <nav class="mb-6 flex flex-wrap gap-2" role="tablist" :aria-label="$t('configView.title')">
      <RouterLink
        v-for="tab in tabs"
        :key="tab.to"
        v-slot="{ isActive, navigate }"
        :to="tab.to"
        custom
      >
        <button
          type="button"
          role="tab"
          :aria-selected="isActive"
          :class="[
            'rounded-full border px-3 py-1 text-xs transition-colors',
            isActive
              ? 'border-primary bg-primary/15 text-foreground'
              : 'border-border text-muted-foreground hover:bg-accent hover:text-foreground',
          ]"
          @click="navigate"
        >
          {{ $t(tab.labelKey) }}
        </button>
      </RouterLink>
    </nav>

    <RouterView />
  </div>
</template>
