import { computed, ref } from 'vue'
import { session } from '@front/api-client'

// 訂閱 api-client 的 session（ref 化供模板/watch 使用）。
const tick = ref(0)
export function useSessionTick(): number {
  return tick.value
}
export function sessionActive(): boolean {
  void tick.value
  return session() !== null
}
export const sessionPresent = computed(() => {
  void tick.value
  return session()
})
export function bumpSessionTick(): void {
  tick.value += 1
}

