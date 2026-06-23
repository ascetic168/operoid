<script setup lang="ts">
import { onMounted } from "vue";
import { RouterView } from "vue-router";
import { Factory, Wrench, Settings, Brain } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { cn } from "@/lib/utils";

const config = useConfigStore();
onMounted(() => config.load());

const nav = [
  { to: "/", label: "工廠", icon: Factory },
  { to: "/operations", label: "操作", icon: Wrench },
  { to: "/config", label: "設定", icon: Settings },
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
        <span :class="isActive ? 'text-foreground' : 'text-muted-foreground'">{{ item.label }}</span>
      </RouterLink>
    </aside>

    <!-- 主內容 -->
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <RouterView />
    </main>
  </div>
</template>
