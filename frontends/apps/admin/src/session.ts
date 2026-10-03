import { configureServer, onUnauthorized } from '@front/api-client'
import { ref } from 'vue'

// 伺服器位址偏好（非 token——可入 storage；token 恆在記憶體）。
const SERVER_KEY = 'operoid.server_url'
const server = ref(localStorage.getItem(SERVER_KEY) ?? '')
configureServer(server.value)

export function serverUrl(): string {
  return server.value
}
export function setServerUrl(url: string): void {
  server.value = url
  localStorage.setItem(SERVER_KEY, url)
  configureServer(url)
}

// 401（token 過期/撤銷）→ 由 api-client 清 session；router 藉此導回登入。
export const sessionLost = ref(false)
export function watchSessionLost(): void {
  onUnauthorized(() => {
    sessionLost.value = true
  })
}
