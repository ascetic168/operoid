<script setup lang="ts">
import { ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { Save, CheckCircle2 } from "lucide-vue-next";
import { useConfigStore } from "@/stores/config";
import { formatError } from "@/lib/tauri";

const config = useConfigStore();
const { t } = useI18n();

// ---- GBrain raw JSON editor（進階；model/tier 鍵可能被 DB plane 蓋過） ----
const rawText = ref("");
const rawError = ref<string | null>(null);
const rawSaved = ref(false);

watchEffect(() => {
  if (config.gbrain) rawText.value = JSON.stringify(config.gbrain.raw, null, 2);
});

async function saveRaw() {
  rawError.value = null;
  rawSaved.value = false;
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawText.value);
  } catch (e) {
    rawError.value = t("configView.jsonParseFail", { e: String(e) });
    return;
  }
  try {
    await config.saveGbrainRaw(parsed);
    rawSaved.value = true;
  } catch (e) {
    rawError.value = formatError(e);
  }
}
</script>

<template>
  <section class="rounded-xl border border-border bg-card/40 p-5">
    <h2 class="mb-2 text-sm font-semibold">{{ $t("configView.advancedSection") }}</h2>
    <label class="mb-3 block text-xs text-muted-foreground">{{ $t("configView.rawLabel") }}</label>
    <textarea
      v-model="rawText"
      spellcheck="false"
      class="h-64 w-full resize-y rounded-md border border-border bg-background p-2 font-mono text-xs"
    />
    <div class="mt-2 flex items-center gap-3">
      <button
        class="flex items-center gap-1 rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:opacity-90"
        @click="saveRaw"
      >
        <Save :size="14" /> {{ $t("configView.writeBack") }}
      </button>
      <span v-if="rawError" class="text-xs text-destructive">{{ rawError }}</span>
      <span v-else-if="rawSaved" class="flex items-center gap-1 text-xs text-green-500">
        <CheckCircle2 :size="13" /> {{ $t("configView.saved") }}
      </span>
    </div>
  </section>
</template>
