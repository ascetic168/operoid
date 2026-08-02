<script setup lang="ts">
export interface MenuItem {
  key: string;
  label: string;
  danger?: boolean;
}

defineProps<{ x: number; y: number; items: MenuItem[] }>();
const emit = defineEmits<{ select: [key: string]; close: [] }>();

function onKey(key: string) {
  emit("select", key);
  emit("close");
}
</script>

<template>
  <!-- 透明全罩：點外／右鍵外 → 關閉 -->
  <div
    class="fixed inset-0 z-50"
    @click="emit('close')"
    @contextmenu.prevent="emit('close')"
  >
    <div
      class="fixed z-[60] min-w-[150px] rounded-md border border-border bg-card py-1 text-xs shadow-2xl"
      :style="{ left: x + 'px', top: y + 'px' }"
      @click.stop
      @contextmenu.prevent.stop
    >
      <button
        v-for="it in items"
        :key="it.key"
        type="button"
        class="flex w-full items-center px-3 py-1.5 text-left transition-colors hover:bg-accent"
        :class="it.danger ? 'text-destructive' : 'text-foreground'"
        @click="onKey(it.key)"
      >
        {{ it.label }}
      </button>
    </div>
  </div>
</template>
