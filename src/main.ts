// [BOOT] 前端入口打点：记录 JS 开始执行的时间，用于切割 Android 52s 启动空档
const _bootT0 = Date.now();
log.info("App", "main.ts 开始执行", { bootT0: _bootT0 });
window.__LEGADO_SET_BOOT_STAGE?.("main-ts-started");

import naive from "naive-ui";
import { createPinia } from "pinia";
import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import "./styles/tokens.css";
import "./styles/theme.css";
import "./styles/base.css";
import "./styles/responsive.css";
import "./styles/reader.css";
import "./styles/focus.css";
import "./styles/remote.css";
import "./styles/components.css";
import { initFrontendStorage } from "./composables/useFrontendStorage";
import { analyticsPlugin } from "./plugins/analytics";
import { log } from "@/utils/logger";

// 当前保留 Naive UI 全量注册；若后续恢复 unplugin 按需导入，再同步移除此处。
const app = createApp(App);
app.use(createPinia());
app.use(analyticsPlugin);
app.config.errorHandler = (err, instance, info) => {
  const details = err instanceof Error ? (err.stack ?? err.message) : String(err);
  log.error("App", err instanceof Error ? err : details, { info, instance });
  window.__LEGADO_SHOW_BOOT_ERROR?.(`Vue 渲染异常 (${info}):\n${details}`);
};
// 挂载后记录首屏到达时间，并移除骨架屏
app.use(naive);

// 在挂载前从后端预取所有持久化数据，确保 useDynamicConfig 的 ready 可立即 resolve，
// 不再依赖 localStorage 作为同步备份，彻底消除脏数据干扰。
await initFrontendStorage();
log.info("App", "前端存储预取完成", { cost: Date.now() - _bootT0 });

try {
  app.mount("#app");
  window.__LEGADO_SET_BOOT_STAGE?.("app-mounted");
  log.info("App", "App 挂载完成", { cost: Date.now() - _bootT0 });
} catch (err) {
  const details = err instanceof Error ? (err.stack ?? err.message) : String(err);
  window.__LEGADO_SHOW_BOOT_ERROR?.(`App 挂载失败:\n${details}`);
  throw err;
}

// 隐藏首屏骨架屏（过渡动画后移除）
const skeleton = document.getElementById("app-skeleton");
if (skeleton) {
  skeleton.classList.add("hidden");
  const removeSkeleton = () => {
    if (skeleton.parentNode) {
      skeleton.remove();
    }
  };
  skeleton.addEventListener("transitionend", removeSkeleton, { once: true });
  // Android WebView 有时不触发 transitionend，500ms 后强制移除
  setTimeout(removeSkeleton, 500);
}
