import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'
import App from './App.vue'
import { router } from './router'
import '@front/ui/tokens.css'
import '@front/ui/base.css'

import en from './locales/en.json'
import zhCN from './locales/zh-CN.json'
import zhTW from './locales/zh-TW.json'

const nav = navigator.language.toLowerCase()
const lang = nav.startsWith('zh') ? (nav.includes('cn') ? 'zh-CN' : 'zh-TW') : 'en'

const i18n = createI18n({
  legacy: false,
  locale: lang,
  fallbackLocale: 'en',
  messages: { en, 'zh-CN': zhCN, 'zh-TW': zhTW },
})

const app = createApp(App)
// R4 診斷：渲染錯誤浮出到 window（暫時性——定位後移除）。
app.config.errorHandler = (err, _inst, info) => {
  ;(window.__errs = window.__errs || []).push('handler:' + String(err) + ' @' + info)
}
app.use(i18n).use(router).mount('#app')
