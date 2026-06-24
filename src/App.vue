<script setup lang="ts">
import { onMounted, ref } from "vue";
import { RouterView } from "vue-router";
import { Factory, Wrench, Settings, Brain, Boxes, AlertTriangle, ExternalLink, X } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { checkPrerequisites, openUrl, tL10n, type DepStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";

const config = useConfigStore();
const missingDeps = ref<DepStatus[]>([]);

onMounted(async () => {
  config.load();
  // 啟動時檢查前置程式;缺漏則彈出說明視窗。
  try {
    const deps = await checkPrerequisites();
    missingDeps.value = deps.filter((d) => !d.available);
  } catch {
    // 檢查本身失敗不阻擋使用
  }
});

const nav = [
  { to: "/", labelKey: "app.nav.factories", icon: Factory },
  { to: "/operations", labelKey: "app.nav.operations", icon: Wrench },
  { to: "/brains", labelKey: "app.nav.brains", icon: Boxes },
  { to: "/config", labelKey: "app.nav.config", icon: Settings },
];
</script>

<template>
  <div class="flex h-full w-full overflow-hidden">
    <!-- 側邊導覽 -->
    <aside
      class="flex w-16 flex-col items-center gap-2 border-r border-border bg-card/40 py-4"
      data-tauri-drag-region
    >
      <div class="mb-4 flex flex-col items-center text-muted-foreground">
        <Brain :size="22" />
      </div>
      <RouterLink
        v-for="item in nav"
        :key="item.to"
        :to="item.to"
        v-slot="{ isActive }"
        class="group flex w-full flex-col items-center gap-1 py-2 text-[11px] transition-colors"
      >
        <div
          :class="
            cn(
              'flex h-9 w-9 items-center justify-center rounded-lg transition-colors',
              isActive
                ? 'bg-primary text-primary-foreground'
                : 'text-muted-foreground group-hover:bg-accent group-hover:text-foreground',
            )
          "
        >
          <component :is="item.icon" :size="18" />
        </div>
        <span :class="isActive ? 'text-foreground' : 'text-muted-foreground'">{{ $t(item.labelKey) }}</span>
      </RouterLink>
    </aside>

    <!-- 主內容 -->
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <RouterView />
    </main>

    <!-- 缺漏前置程式彈窗 -->
    <div
      v-if="missingDeps.length"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    >
      <div class="w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-2xl">
        <div class="mb-3 flex items-start justify-between">
          <div class="flex items-center gap-2 text-warning">
            <AlertTriangle :size="20" />
            <h2 class="text-base font-semibold text-foreground">{{ $t("app.prereq.title") }}</h2>
          </div>
          <button class="text-muted-foreground hover:text-foreground" @click="missingDeps = []">
            <X :size="18" />
          </button>
        </div>
        <p class="mb-4 text-sm text-muted-foreground">
          {{ $t("app.prereq.desc") }}
        </p>
        <div class="space-y-3">
          <div
            v-for="d in missingDeps"
            :key="d.name"
            class="rounded-lg border border-border bg-background/40 p-3"
          >
            <div class="flex items-center justify-between">
              <span class="font-mono text-sm font-medium">{{ d.name }}</span>
              <button
                class="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-accent"
                @click="openUrl(d.url)"
              >
                <ExternalLink :size="12" /> {{ $t("app.prereq.installHint") }}
              </button>
            </div>
            <div class="mt-1 text-xs text-muted-foreground">{{ tL10n(d.install_hint) }}</div>
            <div class="mt-1 text-[11px] text-muted-foreground/70">
              {{ d.detail ?? $t("app.prereq.notFound", { name: d.name }) }}
            </div>
          </div>
        </div>
        <div class="mt-5 flex justify-end">
          <button
            class="rounded-md bg-primary px-4 py-1.5 text-xs text-primary-foreground hover:opacity-90"
            @click="missingDeps = []"
          >
            {{ $t("app.prereq.ack") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
