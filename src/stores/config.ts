import { defineStore } from "pinia";
import { ref } from "vue";
import {
  appInfo,
  getGbrainConfig,
  getAppConfig,
  saveAppConfig as saveAppConfigApi,
  saveGbrainConfigRaw,
  setLocale as setLocaleApi,
  formatError,
  type AppInfo,
  type AppConfig,
  type GBrainConfigView,
} from "@/lib/tauri";
import { applyLocale } from "@/i18n";

/** 全域設定 store：環境資訊 + GBrain config（權威）+ 本系統 app config。 */
export const useConfigStore = defineStore("config", () => {
  const info = ref<AppInfo | null>(null);
  const gbrain = ref<GBrainConfigView | null>(null);
  const app = ref<AppConfig | null>(null);

  const ready = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function loadInfo() {
    info.value = await appInfo();
  }

  async function loadGbrain() {
    gbrain.value = await getGbrainConfig();
  }

  async function loadApp() {
    app.value = await getAppConfig();
  }

  async function load() {
    if (loading.value) return; // 避免並發重複載入
    loading.value = true;
    try {
      await loadInfo();
      await Promise.all([loadGbrain(), loadApp()]);
      // 載入後套用使用者釘選的 locale（null → 系統偵測）
      applyLocale(app.value?.locale ?? null);
      ready.value = true;
    } catch (e) {
      error.value = formatError(e);
    } finally {
      loading.value = false;
    }
  }

  async function saveAppConfig(cfg: AppConfig) {
    await saveAppConfigApi(cfg);
    app.value = cfg;
  }

  async function saveGbrainRaw(raw: unknown) {
    await saveGbrainConfigRaw(raw);
    await loadGbrain();
  }

  /** 切換介面語言：持久化 + 即時套用。`null` = 回到自動偵測。 */
  async function setLocale(locale: string | null) {
    const eff = await setLocaleApi(locale);
    if (app.value) app.value = { ...app.value, locale: eff };
    applyLocale(eff);
  }

  return {
    info,
    gbrain,
    app,
    ready,
    loading,
    error,
    load,
    loadGbrain,
    loadApp,
    saveAppConfig,
    saveGbrainRaw,
    setLocale,
  };
});
