import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import "./style.css";

const app = createApp(App);

// 全域錯誤兜底：任何 Vue 渲染/生命週期錯誤都顯示成一個「蓋在 Vue 之外」的紅色覆蓋層，
// 這樣即使根元件渲染失敗導致整個 Vue 樹空白，使用者仍看得到實際錯誤訊息。
function showCrashOverlay(msg: string) {
  let el = document.getElementById("gbs-crash");
  if (!el) {
    el = document.createElement("div");
    el.id = "gbs-crash";
    Object.assign(el.style, {
      position: "fixed",
      inset: "0",
      background: "rgba(0,0,0,0.92)",
      color: "#f87171",
      padding: "24px",
      fontFamily: "ui-monospace, monospace",
      fontSize: "13px",
      lineHeight: "1.5",
      whiteSpace: "pre-wrap",
      overflow: "auto",
      zIndex: "99999",
    } satisfies Partial<CSSStyleDeclaration>);
    document.body.appendChild(el);
  }
  const stamp = new Date().toLocaleTimeString();
  el.textContent = (el.textContent ? el.textContent + "\n\n" : "") + `[${stamp}] ${msg}`;
}

app.config.errorHandler = (err, _instance, info) => {
  console.error("[Vue error]", err, "| info:", info);
  showCrashOverlay(`Vue error: ${err}\n(info: ${info})`);
};
window.addEventListener("unhandledrejection", (e) => {
  console.error("[unhandled rejection]", e.reason);
  showCrashOverlay(`Unhandled rejection: ${e.reason}`);
});
window.addEventListener("error", (e) => {
  console.error("[window error]", e.error ?? e.message);
  showCrashOverlay(`Window error: ${e.error ?? e.message}`);
});

app.use(createPinia()).use(router).mount("#app");
