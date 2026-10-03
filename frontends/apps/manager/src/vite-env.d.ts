/// <reference types="vite/client" />

// 全域擴充（script 檔——直接介面合併，無需 declare global）
interface Window {
  /** R4 診斷：app.config.errorHandler 捕捉的渲染錯誤（瀏覽器測試用）。 */
  __errs?: string[]
}

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<object, object, unknown>
  export default component
}
