import { resolve } from "node:path";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
  // Tauri 期望固定 port；在同時跑多個 Tauri 專案時可避免衝突
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // 固定綁 IPv4：Node 17+ 對 "localhost" 依 OS DNS 序解析，Windows 上常解析成
    // ::1 只綁 IPv6，而 Tauri CLI 以 127.0.0.1 輪詢 devUrl → 永遠 Waiting。
    // TAURI_DEV_HOST（行動裝置實機測試）設定時仍優先採用。
    host: host || "127.0.0.1",
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
});
