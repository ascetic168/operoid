<script setup lang="ts">
import { computed } from 'vue'
const props = defineProps<{
  /** 後端錯誤碼（搭配呼叫端 i18n 翻譯）或直接給文字。 */
  code?: string
  message?: string
}>()
const text = computed(() => props.message ?? props.code ?? '')
</script>

<template>
  <p v-if="text" class="err" data-test="error">{{ text }}</p>
</template>

<style scoped>
.err {
  margin: 0.6rem 0 0;
  color: var(--danger);
  font-size: 0.88rem;
}
</style>
