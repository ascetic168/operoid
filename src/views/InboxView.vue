<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Loader2 } from "lucide-vue-next";
import { useInboxStore } from "@/stores/inbox";
import { agentApproveCommitment, agentRejectCommitment, formatError } from "@/lib/tauri";

const { t } = useI18n();
const router = useRouter();
const inbox = useInboxStore();

const pending = ref<string | null>(null);
const error = ref<string | null>(null);

async function approve(cid: string) {
  if (pending.value) return;
  pending.value = cid;
  try {
    await agentApproveCommitment(cid);
    await inbox.refresh();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    pending.value = null;
  }
}
async function reject(cid: string) {
  if (pending.value) return;
  pending.value = cid;
  try {
    await agentRejectCommitment(cid);
    await inbox.refresh();
  } catch (e) {
    error.value = formatError(e);
  } finally {
    pending.value = null;
  }
}
</script>

<template>
  <div class="flex h-full w-full flex-col overflow-hidden">
    <!-- 標題列 -->
    <div class="flex items-center gap-2 border-b border-border px-4 py-2.5">
      <h2 class="text-sm font-semibold">{{ t("inbox.title") }}</h2>
    </div>

    <!-- 內容區 -->
    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      <p v-if="error" class="mb-3 text-xs text-destructive">{{ error }}</p>

      <div v-if="inbox.pendingCount === 0" class="py-10 text-center text-sm text-muted-foreground">
        {{ t("inbox.empty") }}
      </div>

      <!-- 待核可提案 -->
      <section v-if="(inbox.summary?.proposals.length ?? 0) > 0" class="mb-6">
        <h3 class="mb-2 text-xs font-medium text-muted-foreground">{{ t("inbox.proposals") }}</h3>
        <div class="flex flex-col gap-2">
          <div
            v-for="p in inbox.summary?.proposals ?? []"
            :key="p.commitment_id"
            class="rounded-lg border border-border bg-card p-3"
          >
            <div class="flex items-center justify-between gap-2">
              <span class="truncate text-sm font-medium">{{ p.title }}</span>
              <span class="shrink-0 text-xs text-muted-foreground">{{ p.employee_name }}</span>
            </div>
            <p class="mt-1 text-xs text-muted-foreground">{{ p.completion_condition }}</p>
            <div class="mt-2 flex gap-2">
              <button
                class="flex items-center gap-1 rounded bg-emerald-600 px-2.5 py-1 text-xs text-white hover:opacity-90 disabled:opacity-50"
                :disabled="pending !== null"
                @click="approve(p.commitment_id)"
              >
                <Loader2 v-if="pending === p.commitment_id" :size="12" class="animate-spin" />
                ✓ {{ t("approval.approve") }}
              </button>
              <button
                class="flex items-center gap-1 rounded border border-border px-2.5 py-1 text-xs hover:bg-accent disabled:opacity-50"
                :disabled="pending !== null"
                @click="reject(p.commitment_id)"
              >
                <Loader2 v-if="pending === p.commitment_id" :size="12" class="animate-spin" />
                ✗ {{ t("approval.reject") }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <!-- 異常員工 -->
      <section v-if="(inbox.summary?.flagged_employees.length ?? 0) > 0">
        <h3 class="mb-2 text-xs font-medium text-muted-foreground">{{ t("inbox.flagged") }}</h3>
        <div class="flex flex-col gap-2">
          <div
            v-for="f in inbox.summary?.flagged_employees ?? []"
            :key="f.employee_id"
            class="flex items-center justify-between gap-2 rounded-lg border border-warning/40 bg-warning/5 p-3"
          >
            <div class="flex items-center gap-2">
              <span class="h-2 w-2 rounded-full bg-amber-500" />
              <span class="text-sm font-medium">{{ f.employee_name }}</span>
              <span class="text-xs text-muted-foreground">[{{ f.state }}]</span>
            </div>
            <button
              class="rounded border border-border px-2 py-1 text-xs hover:bg-accent"
              @click="router.push('/instances')"
            >
              {{ t("inbox.viewEmployee") }}
            </button>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
