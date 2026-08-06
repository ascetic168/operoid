import { defineStore } from "pinia";
import { ref } from "vue";
import {
  appInfo,
  getGbrainConfig,
  getAppConfig,
  saveAppConfig as saveAppConfigApi,
  saveGbrainConfigRaw,
  setGbrainModel as setGbrainModelApi,
  setGbrainModelsAll as setGbrainModelsAllApi,
  unsetGbrainModel as unsetGbrainModelApi,
  clearDbOverrides as clearDbOverridesApi,
  setProviderBaseUrl as setProviderBaseUrlApi,
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

  /** 設單一 model/tier 鍵（DB plane），完成後重抓以反映新的 tiers/db_overrides。 */
  async function setGbrainModel(key: string, value: string) {
    await setGbrainModelApi(key, value);
    await loadGbrain();
  }

  /** 單一模型同步到全部 tier（勾選同步用）。 */
  async function setGbrainModelsAll(model: string) {
    await setGbrainModelsAllApi(model);
    await loadGbrain();
  }

  /** 移除單一 model/tier 鍵的 DB 覆寫。 */
  async function unsetGbrainModel(key: string) {
    await unsetGbrainModelApi(key);
    await loadGbrain();
  }

  /** 清除所有 DB-plane model/tier 覆寫（修復用）。 */
  async function clearDbOverrides() {
    await clearDbOverridesApi();
    await loadGbrain();
  }

  /** 設 provider_base_url（直寫檔案）。 */
  async function setProviderBaseUrl(provider: string, baseUrl: string | null) {
    await setProviderBaseUrlApi(provider, baseUrl);
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
    setGbrainModel,
    setGbrainModelsAll,
    unsetGbrainModel,
    clearDbOverrides,
    setProviderBaseUrl,
    setLocale,
  };
});
