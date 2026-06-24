import { createI18n } from "vue-i18n";
import {
  detectSystemLocale,
  FALLBACK_LOCALE,
  getHtmlLangForLocale,
  isSupportedLocale,
} from "./languageConfig";
import zhTW from "./locales/zh-TW.json";
import zhCN from "./locales/zh-CN.json";
import en from "./locales/en.json";

/**
 * 初始 locale 在模組載入時就由 `detectSystemLocale()` 同步決定 → 首繪即正確語言。
 * 之後 `main.ts`/config store 載入 AppConfig 後，若有使用者手動釘選的 locale 會再覆寫。
 */
const i18n = createI18n({
  legacy: false,
  locale: detectSystemLocale(),
  fallbackLocale: FALLBACK_LOCALE,
  messages: {
    "zh-TW": zhTW,
    "zh-CN": zhCN,
    en,
  },
});

/**
 * 套用 locale：設 vue-i18n 當前語言 + `<html lang>`。
 * `locale=null`（或非支援值）→ 回到系統偵測語言。
 */
export function applyLocale(locale: string | null | undefined): void {
  const loc = isSupportedLocale(locale) ? locale : detectSystemLocale();
  i18n.global.locale.value = loc;
  document.documentElement.lang = getHtmlLangForLocale(loc);
}

export default i18n;
