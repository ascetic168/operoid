/**
 * 語言設定：支援語言清單 + 跨平台系統語言偵測。
 *
 * webview 的 `navigator.languages` 會跟隨 OS 系統語言（Tauri v2 webview，
 * Windows/macOS/Linux 皆是）—— 這是跨平台偵測的來源。`detectSystemLocale()`
 * 為同步函式，可在 Vue mount 前定好初始 locale（首繪即正確語言，無閃爍）。
 */

export type SupportedLocale = "zh-TW" | "zh-CN" | "en";

/** 偵測不到（或非中日韓語系）時的退路語言。 */
export const FALLBACK_LOCALE: SupportedLocale = "en";

/** Rust 端 `app_config::SUPPORTED_LOCALES` 必須與此一致。 */
export const SUPPORTED_LOCALES: SupportedLocale[] = ["zh-TW", "zh-CN", "en"];

export interface LanguageOption {
  locale: SupportedLocale;
  /** 顯示名稱（各語言中以其原生寫法顯示，不隨 locale 變動）。 */
  displayName: string;
  /** `<html lang>` 值。 */
  htmlLang: string;
  /** 用於比對 `navigator.languages` 的 pattern（由精確到寬鬆）。 */
  navigatorPatternList: string[];
}

export const LANGUAGE_OPTIONS: LanguageOption[] = [
  {
    locale: "zh-TW",
    displayName: "繁體中文",
    htmlLang: "zh-Hant",
    navigatorPatternList: ["zh-Hant-TW", "zh-Hant", "zh-TW"],
  },
  {
    locale: "zh-CN",
    displayName: "简体中文",
    htmlLang: "zh-Hans",
    navigatorPatternList: ["zh-Hans-CN", "zh-Hans", "zh-CN"],
  },
  {
    locale: "en",
    displayName: "English",
    htmlLang: "en",
    navigatorPatternList: ["en"],
  },
];

/**
 * 由 `navigator.languages` 偵測系統語言。比對順序（對每個 navigator 語言）：
 * 1. 精確（zh-Hant-TW / zh-TW）
 * 2. script subtag（zh-Hant → zh-TW；zh-Hans → zh-CN）
 * 3. prefix（en-US → en）
 * 4. 裸 `zh` → zh-TW（保護繁中使用者）
 * 5. 退路 FALLBACK_LOCALE
 */
export function detectSystemLocale(): SupportedLocale {
  const browserLanguageList =
    typeof navigator !== "undefined" ? navigator.languages : [];

  for (const browserLang of browserLanguageList) {
    // 1. 精確
    for (const option of LANGUAGE_OPTIONS) {
      if (
        option.navigatorPatternList.some(
          (p) => p.toLowerCase() === browserLang.toLowerCase(),
        )
      ) {
        return option.locale;
      }
    }
    // 2. script subtag
    for (const option of LANGUAGE_OPTIONS) {
      if (
        option.navigatorPatternList.some((p) =>
          browserLang.toLowerCase().startsWith(p.toLowerCase() + "-"),
        )
      ) {
        return option.locale;
      }
    }
    // 3. prefix
    const langPrefix = browserLang.split("-")[0].toLowerCase();
    for (const option of LANGUAGE_OPTIONS) {
      if (option.locale.toLowerCase() === langPrefix) {
        return option.locale;
      }
    }
    // 4. 裸 zh → zh-TW
    if (langPrefix === "zh") {
      return "zh-TW";
    }
  }

  return FALLBACK_LOCALE;
}

export function getHtmlLangForLocale(locale: SupportedLocale): string {
  return LANGUAGE_OPTIONS.find((o) => o.locale === locale)?.htmlLang ?? "en";
}

export function isSupportedLocale(s: string | null | undefined): s is SupportedLocale {
  return !!s && (SUPPORTED_LOCALES as string[]).includes(s);
}
