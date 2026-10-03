import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

// dev：vite proxy 轉發 API 到本機 oserver；build：產出掛 oserver /admin/ 靜態服務。
export default defineConfig({
  base: "/admin/",
  plugins: [vue()],
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:7340',
      '/event': 'http://127.0.0.1:7340',
    },
  },
  optimizeDeps: { exclude: ['@front/api-client', '@front/ui'] },
})
