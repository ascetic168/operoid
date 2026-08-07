<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { RouterView } from "vue-router";
import { Factory, Wrench, Settings, Brain, Boxes, Users, UserSquare, AlertTriangle, ExternalLink, Terminal, X, Bell, Inbox, Activity } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { useInboxStore } from "@/stores/inbox";
import { checkPrerequisites, openUrl, tL10n, type DepStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import ClaudeCodeDialog from "@/components/ClaudeCodeDialog.vue";

const config = useConfigStore();
const inbox = useInboxStore();
const missingDeps = ref<DepStatus[]>([]);
const claudeOpen = ref(false);
const bellOpen = ref(false);

onMounted(async () => {
  config.load();
  // 動詞軌：啟動全域輪詢（跨員工待辦聚合）。
  inbox.start();
  // 啟動時檢查前置程式;缺漏則彈出說明視窗。
  try {
    const deps = await checkPrerequisites();
    missingDeps.value = deps.filter((d) => !d.available);
  } catch {
    // 檢查本身失敗不阻擋使用
  }
});
onUnmounted(() => {
  inbox.stop();
});

const nav = [
  { to: "/factories", labelKey: "app.nav.factories", icon: Factory },
  { to: "/templates", labelKey: "app.nav.employeeTemplate", icon: Users },
  { to: "/instances", labelKey: "app.nav.employeeInstance", icon: UserSquare },
  { to: "/operations", labelKey: "app.nav.operations", icon: Wrench },
  { to: "/brains", labelKey: "app.nav.brains", icon: Boxes },
  { to: "/config", labelKey: "app.nav.config", icon: Settings },
];
</script>

<template>
  <div class="flex h-full w-full flex-col overflow-hidden">
    <!-- 頂列（動詞軌）：跨員工待辦與事件入口 -->
    <header
      class="flex h-9 shrink-0 items-center gap-1 border-b border-border bg-card/60 px-2 text-muted-foreground"
      data-tauri-drag-region
    >
      <div class="flex items-center gap-2 px-1 text-foreground">
        <Brain :size="15" />
      </div>
      <!-- 鈴鐺（待辦計數徽章；點擊展開 popover）-->
      <div class="relative">
        <button
          class="relative flex h-7 items-center gap-1 rounded-md px-2 text-xs hover:bg-accent hover:text-foreground"
          :title="$t('topbar.inbox')"
          @click="bellOpen = !bellOpen"
        >
          <Bell :size="14" />
          <span
            v-if="inbox.pendingCount > 0"
            class="ml-0.5 rounded-full bg-destructive px-1.5 text-[10px] font-medium leading-4 text-destructive-foreground"
          >{{ inbox.pendingCount }}</span>
        </button>
        <!-- 待辦 popover（Teleport 脫離 drag-region）-->
        <Teleport to="body">
          <div v-if="bellOpen" class="fixed inset-0 z-50" @click="bellOpen = false">
            <div
              class="absolute left-2 top-9 w-80 rounded-lg border border-border bg-card shadow-2xl"
              @click.stop
            >
              <div class="border-b border-border px-3 py-2 text-xs font-medium text-foreground">{{ $t('topbar.inbox') }}</div>
              <div class="max-h-80 overflow-y-auto p-2 text-xs">
                <div v-if="inbox.pendingCount === 0" class="py-4 text-center text-muted-foreground">{{ $t('inbox.empty') }}</div>
                <!-- 待核可提案 -->
                <div v-for="p in inbox.summary?.proposals ?? []" :key="p.commitment_id" class="mb-1 rounded-md border border-border bg-background p-2">
                  <div class="truncate font-medium text-foreground">{{ p.title }}</div>
                  <div class="truncate text-muted-foreground">{{ p.employee_name }}</div>
                </div>
                <!-- 異常員工 -->
                <div v-for="f in inbox.summary?.flagged_employees ?? []" :key="f.employee_id" class="mb-1 rounded-md border border-warning/40 bg-warning/5 p-2">
                  <div class="truncate font-medium text-foreground">{{ f.employee_name }}</div>
                  <div class="text-muted-foreground">[{{ f.state }}]</div>
                </div>
              </div>
              <RouterLink
                v-if="inbox.pendingCount > 0"
                :to="'/inbox'"
                class="block border-t border-border px-3 py-2 text-center text-xs text-primary hover:bg-accent"
                @click="bellOpen = false"
              >{{ $t('topbar.inbox') }} →</RouterLink>
            </div>
          </div>
        </Teleport>
      </div>
      <!-- 待辦入口 -->
      <RouterLink
        :to="'/inbox'"
        class="flex h-7 items-center gap-1 rounded-md px-2 text-xs hover:bg-accent hover:text-foreground"
        :class="$route.path === '/inbox' ? 'text-foreground' : 'text-muted-foreground'"
      >
        <Inbox :size="14" />
        <span>{{ $t('topbar.inbox') }}</span>
      </RouterLink>
      <!-- 事件入口 -->
      <RouterLink
        :to="'/events'"
        class="flex h-7 items-center gap-1 rounded-md px-2 text-xs hover:bg-accent hover:text-foreground"
        :class="$route.path === '/events' ? 'text-foreground' : 'text-muted-foreground'"
      >
        <Activity :size="14" />
        <span>{{ $t('topbar.events') }}</span>
      </RouterLink>
    </header>

    <!-- 名詞軌 + 主內容區 -->
    <div class="flex min-h-0 flex-1 overflow-hidden">
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

      <!-- 開啟 Claude Code（帶作用中腦的 gbrain MCP）-->
      <button
        class="group mt-auto flex w-full flex-col items-center gap-1 py-2 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        @click="claudeOpen = true"
      >
        <div
          class="flex h-9 w-9 items-center justify-center rounded-lg transition-colors group-hover:bg-accent group-hover:text-foreground"
        >
          <Terminal :size="18" />
        </div>
        <span>Claude Code</span>
      </button>
    </aside>

    <!-- 主內容 -->
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <RouterView />
    </main>
    </div><!-- /名詞軌 + 主內容區 -->

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

    <!-- 開啟 Claude Code 對話框 -->
    <ClaudeCodeDialog :open="claudeOpen" @close="claudeOpen = false" @launched="claudeOpen = false" />
  </div>
</template>
