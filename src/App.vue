<!-- App.vue：应用根组件，负责全局 Provider、主视图保活、桌面/移动主布局与顶层返回事件派发。 -->
<script setup lang="ts">
import { darkTheme, type GlobalTheme, type GlobalThemeOverrides } from "naive-ui";
import { storeToRefs } from "pinia";
import { ref, computed, defineAsyncComponent, watch, reactive, onMounted, onUnmounted } from "vue";
import type { NavItem } from "@/types";
import packageJson from "../package.json";
import tauriConfig from "../src-tauri/tauri.conf.json";
import GlobalFeedbackMirror from "./components/GlobalFeedbackMirror.vue";
import BottomNav from "./components/layout/BottomNav.vue";
import LogWindowPanel from "./components/layout/LogWindowPanel.vue";
import MainContent from "./components/layout/MainContent.vue";
import SideBar from "./components/layout/SideBar.vue";
import TaskBar from "./components/layout/TaskBar.vue";
import TaskCenterDrawer from "./components/layout/TaskCenterDrawer.vue";
import TitleBar from "./components/layout/TitleBar.vue";
import LegadoDeepLinkDialog from "./components/LegadoDeepLinkDialog.vue";
import {
  isMobile,
  setLayoutMode,
  isTauri,
  hasNativeTransport,
  platform,
  initPlatformFromRust,
} from "./composables/useEnv";
import { eventEmit } from "./composables/useEventBus";
import { installGlobalFocusNavigation } from "./composables/useFocusNavigation";
import { useInputMode } from "./composables/useInputMode";
import { useLogZonePref } from "./composables/useLogZonePref";
import { useRemoteDebug } from "./composables/useRemoteDebug";
import { installSyncClientStateListener, useSync } from "./composables/useSync";
import { useTtsPlaybackState } from "./composables/useTtsState";
import { useVConsole } from "./composables/useVConsole";
import { useReaderSettingsStore } from "@/features/reader/stores/readerSettings";
import { useReaderUiStore } from "@/features/reader/stores/readerUi";
import { useAppConfigStore } from "./stores/appConfig";
import { useBackStackStore } from "./stores/backStack";
import { useNavigationStore } from "./stores/navigation";
import { usePrivacyModeStore } from "./stores/privacyMode";
import { useShellStatusStore } from "./stores/shellStatus";
import { log } from "@/utils/logger";
// ScriptDialog 按需懒加载：仅在 Boa 引擎触发弹窗时才加载，不阻塞首屏
const ScriptDialog = defineAsyncComponent(() => import("./components/ScriptDialog.vue"));
const FrontendPluginDialog = defineAsyncComponent(
  () => import("./components/FrontendPluginDialog.vue"),
);
// WsConnectDialog：非 Tauri 环境下后端连接失败时弹出地址输入框
const WsConnectDialog = defineAsyncComponent(() => import("./components/WsConnectDialog.vue"));
const MiniPlayerBar = defineAsyncComponent(() => import("./components/music/MiniPlayerBar.vue"));
const MusicPlayerOverlay = defineAsyncComponent(
  () => import("./components/music/MusicPlayerOverlay.vue"),
);
import BookSourceLimitWarningDialog from "./components/BookSourceLimitWarningDialog.vue";
import PrefetchProgressBar from "./components/PrefetchProgressBar.vue";

// ── 主窗口视图 ───────────────────────────────────────────────────────────

const BookshelfView = defineAsyncComponent(() => import("./views/BookshelfView.vue"));
const ExploreView = defineAsyncComponent(() => import("./views/ExploreView.vue"));
const SearchView = defineAsyncComponent(() => import("./views/SearchView.vue"));
const BookSourceView = defineAsyncComponent(() => import("./views/BookSourceView.vue"));
const ExtensionsView = defineAsyncComponent(() => import("./views/ExtensionsView.vue"));
const SettingsView = defineAsyncComponent(() => import("./views/SettingsView.vue"));

const viewMap: Record<string, ReturnType<typeof defineAsyncComponent>> = {
  bookshelf: BookshelfView,
  explore: ExploreView,
  search: SearchView,
  booksource: BookSourceView,
  extensions: ExtensionsView,
  settings: SettingsView,
};

/** 桌面端导航项 */
const desktopNavItems: NavItem[] = [
  { id: "bookshelf", icon: "bookshelf", label: "书架" },
  { id: "explore", icon: "explore", label: "发现" },
  { id: "search", icon: "search", label: "搜索" },
  { id: "booksource", icon: "booksource", label: "书源管理" },
  { id: "extensions", icon: "extensions", label: "插件管理" },
  { id: "settings", icon: "settings", label: "设置" },
];

/** 移动端底部导航项（精简六项） */
const mobileNavItems: NavItem[] = [
  { id: "bookshelf", icon: "bookshelf", label: "书架" },
  { id: "explore", icon: "explore", label: "发现" },
  { id: "search", icon: "search", label: "搜索" },
  { id: "booksource", icon: "booksource", label: "书源管理" },
  { id: "extensions", icon: "extensions", label: "扩展" },
  { id: "settings", icon: "settings", label: "设置" },
];

const navItems = computed(() => (isMobile.value ? mobileNavItems : desktopNavItems));
const activeNavLabel = computed(
  () => navItems.value.find((n) => n.id === navigationStore.activeView)?.label ?? "",
);

// ── 视图缓存（惰性加载 + 保持挂载）────────────────────────────────────────
const ALL_VIEW_IDS = Object.keys(viewMap);

const navigationStore = useNavigationStore();
const { activeView } = storeToRefs(navigationStore);
const appConfigStore = useAppConfigStore();
const { setupAutoExit: setupPrivacyModeAutoExit } = usePrivacyModeStore();
const readerSettingsStore = useReaderSettingsStore();
const readerUiStore = useReaderUiStore();
const tts = useTtsPlaybackState();
const shellStatusStore = useShellStatusStore();
const backStackStore = useBackStackStore();
useInputMode();

// 兼容原有代码对 appConfig 的直接访问
const appConfig = computed(() => appConfigStore.config);
const ensureAppConfig = () => appConfigStore.ensureLoaded();

/** 已被激活过的视图（v-if 为 true 后永不移除，实现 keep-alive） */
const mountedViews = reactive(new Set<string>([navigationStore.activeView]));

/** Suspense 已 resolve 的视图 */
const resolvedViews = reactive(new Set<string>());

/** 移动端加载遮罩状态 */
const showLoadingMask = ref(false);
let _maskMinElapsed = false;
let _pendingViewId = "";
let _maskTimer: ReturnType<typeof setTimeout> | null = null;
let _maskSafetyTimer: ReturnType<typeof setTimeout> | null = null;

// ── 书源超限警告对话框 ────────────────────────────────────────────────────
const showBookSourceLimitWarning = ref(false);
const enabledBookSourceCount = ref(0);

const bookSourceStore = {
  enabledSources: [] as unknown[],
  async loadSources() {
    const { useBookSourceStore } = await import("./stores/bookSource");
    const store = useBookSourceStore();
    await store.loadSources();
    this.enabledSources = store.enabledSources;
  },
};

function onSuspenseResolve(viewId: string) {
  resolvedViews.add(viewId);
  if (showLoadingMask.value && viewId === _pendingViewId) {
    _pendingViewId = "";
    if (_maskMinElapsed) {
      showLoadingMask.value = false;
    }
  }
}

watch(
  () => navigationStore.activeView,
  (newId) => {
    const alreadyMounted = mountedViews.has(newId);
    mountedViews.add(newId);

    // 移动端：首次加载该视图时显示全屏遮罩，强制最少显示 300ms
    if (isMobile.value && !alreadyMounted) {
      showLoadingMask.value = true;
      _maskMinElapsed = false;
      _pendingViewId = newId;
      if (_maskTimer) {
        clearTimeout(_maskTimer);
      }
      if (_maskSafetyTimer) {
        clearTimeout(_maskSafetyTimer);
      }
      _maskTimer = setTimeout(() => {
        _maskMinElapsed = true;
        // 如果 Suspense 已经 resolve，现在才真正隐藏遮罩
        if (!_pendingViewId) {
          showLoadingMask.value = false;
        }
      }, 100);
      // 安全兜底：最长 15s 后强制移除遮罩，防止异步组件加载失败导致永久转圈
      _maskSafetyTimer = setTimeout(() => {
        if (showLoadingMask.value) {
          log.warn("App", "视图加载超时，强制移除遮罩", { viewId: _pendingViewId });
          _pendingViewId = "";
          showLoadingMask.value = false;
        }
      }, 15000);
    }
  },
);

/** Windows 桌面端即使强制手机布局，也保留完整标题栏（拖拽 + 窗口控制） */
const forceDesktopBar = computed(() => isTauri && platform.value === "Windows");
const isAndroidPlatform = computed(() => platform.value === "Android");
const appForeground = ref(
  typeof document === "undefined" || document.visibilityState === "visible",
);
const readerImmersiveModeEnabled = computed(
  () =>
    isMobile.value &&
    readerUiStore.readerVisible &&
    readerSettingsStore.settings.hideTopBarOnMobile,
);

function syncAndroidReaderImmersiveMode(enabled: boolean) {
  window.LegadoAndroidInput?.setReaderImmersiveModeEnabled?.(enabled);
}

watch(readerImmersiveModeEnabled, syncAndroidReaderImmersiveMode, {
  immediate: true,
});

function clampBrightnessPercent(value: number): number {
  if (!Number.isFinite(value)) {
    return 100;
  }
  return Math.min(100, Math.max(1, Math.round(value)));
}

const readerBrightnessPercent = computed(() =>
  clampBrightnessPercent(readerSettingsStore.settings.brightness),
);
const readerNativeBrightnessEnabled = computed(
  () =>
    isAndroidPlatform.value &&
    appForeground.value &&
    readerUiStore.readerVisible &&
    readerSettingsStore.settings.brightnessMode === "custom",
);

function syncAndroidReaderBrightness(enabled: boolean, percent = readerBrightnessPercent.value) {
  if (enabled) {
    window.LegadoAndroidInput?.setReaderBrightnessPercent?.(percent);
    return;
  }
  window.LegadoAndroidInput?.clearReaderBrightnessOverride?.();
}

watch(
  () => [readerNativeBrightnessEnabled.value, readerBrightnessPercent.value] as const,
  ([enabled, percent]) => syncAndroidReaderBrightness(enabled, percent),
  { immediate: true },
);

type ReaderAwakeMode = "off" | "always" | "timeout";

const READER_AWAKE_ACTIVITY_EVENTS = [
  "pointerdown",
  "touchstart",
  "keydown",
  "wheel",
  "scroll",
] as const;

const readerAwakeTimerActive = ref(false);
let readerAwakeTimer: ReturnType<typeof setTimeout> | null = null;

function resolveReaderAwakeMode(value: string | undefined): ReaderAwakeMode {
  return value === "always" || value === "timeout" ? value : "off";
}

const readerAwakeMode = computed(() =>
  resolveReaderAwakeMode(appConfig.value.power_reader_awake_mode),
);
const readerAwakeTimeoutMs = computed(() => {
  const seconds = Number(appConfig.value.power_reader_awake_timeout_secs ?? 600);
  const safeSeconds = Number.isFinite(seconds)
    ? Math.min(7200, Math.max(60, Math.round(seconds)))
    : 600;
  return safeSeconds * 1000;
});

function clearReaderAwakeTimer() {
  if (readerAwakeTimer) {
    clearTimeout(readerAwakeTimer);
    readerAwakeTimer = null;
  }
  readerAwakeTimerActive.value = false;
}

function armReaderAwakeTimer() {
  if (!readerUiStore.readerVisible || readerAwakeMode.value !== "timeout" || !appForeground.value) {
    clearReaderAwakeTimer();
    return;
  }
  if (readerAwakeTimer) {
    clearTimeout(readerAwakeTimer);
  }
  readerAwakeTimerActive.value = true;
  readerAwakeTimer = setTimeout(() => {
    readerAwakeTimer = null;
    readerAwakeTimerActive.value = false;
  }, readerAwakeTimeoutMs.value);
}

function handleReaderAwakeActivity() {
  armReaderAwakeTimer();
}

function installReaderAwakeActivityListeners() {
  for (const eventName of READER_AWAKE_ACTIVITY_EVENTS) {
    window.addEventListener(eventName, handleReaderAwakeActivity, {
      capture: true,
      passive: true,
    });
  }
}

function removeReaderAwakeActivityListeners() {
  for (const eventName of READER_AWAKE_ACTIVITY_EVENTS) {
    window.removeEventListener(eventName, handleReaderAwakeActivity, {
      capture: true,
    });
  }
}

watch(
  () =>
    [
      readerUiStore.readerVisible,
      readerAwakeMode.value,
      readerAwakeTimeoutMs.value,
      appForeground.value,
    ] as const,
  ([visible, mode, timeoutMs, foreground]) => {
    void timeoutMs;
    if (visible && mode === "timeout" && foreground) {
      armReaderAwakeTimer();
      return;
    }
    clearReaderAwakeTimer();
  },
  { immediate: true },
);

const ttsKeepAwakeEnabled = computed(
  () => appConfig.value.power_keep_awake_on_tts === true && tts.isPlaying.value,
);
const readerAlwaysAwakeEnabled = computed(
  () => readerUiStore.readerVisible && readerAwakeMode.value === "always",
);
const readerTimedAwakeEnabled = computed(
  () =>
    readerUiStore.readerVisible &&
    readerAwakeMode.value === "timeout" &&
    readerAwakeTimerActive.value,
);
const keepScreenOnEnabled = computed(
  () =>
    isAndroidPlatform.value &&
    appForeground.value &&
    (ttsKeepAwakeEnabled.value || readerAlwaysAwakeEnabled.value || readerTimedAwakeEnabled.value),
);

function syncAndroidKeepScreenOn(enabled: boolean) {
  window.LegadoAndroidInput?.setKeepScreenOn?.(enabled);
}

watch(keepScreenOnEnabled, syncAndroidKeepScreenOn, {
  immediate: true,
});

function setAppForeground(nextForeground: boolean) {
  appForeground.value = nextForeground;
  if (!nextForeground) {
    clearReaderAwakeTimer();
    syncAndroidKeepScreenOn(false);
    syncAndroidReaderBrightness(false);
  }
}

// ── 布局模式同步 ─────────────────────────────────────────────────────────
const sync = useSync();
// ── 主题系统 ─────────────────────────────────────────────────────────────────
// 监听系统静态主题偏好
// 初始化时尝试读取，不支持的环境则回落false
const systemPrefersDark = ref(
  typeof window !== "undefined" && window.matchMedia
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
    : false,
);

let _mq: MediaQueryList | null = null;
let _unlistenSyncClientState: (() => void) | null = null;
let _uninstallGlobalFocus: (() => void) | null = null;
function _onMqChange(e: MediaQueryListEvent) {
  systemPrefersDark.value = e.matches;
}
function _onVisibilityChange() {
  setAppForeground(document.visibilityState === "visible");
  const event = document.visibilityState === "visible" ? "resume" : "background";
  void sync.notifyLifecycle(event).catch(() => {});
}
function _onAndroidAppPaused() {
  setAppForeground(false);
}
function _onAndroidAppResumed() {
  setAppForeground(true);
}

function dispatchSyntheticOutsideClick(): boolean {
  if (typeof document === "undefined" || !document.body) {
    return false;
  }
  const init = {
    bubbles: true,
    cancelable: true,
    composed: true,
    view: window,
  };
  document.body.dispatchEvent(new MouseEvent("mousedown", init));
  document.body.dispatchEvent(new MouseEvent("mouseup", init));
  document.body.dispatchEvent(new MouseEvent("click", init));
  return true;
}

function closeTopOverlay(): boolean {
  const overlayCloseBtn = document.querySelector<HTMLElement>(
    ".n-modal .n-base-close, .n-drawer .n-base-close, .n-popover .n-base-close, .lw-panel .lw-ctrl-btn--close, .tc-panel .tc-close",
  );
  if (overlayCloseBtn) {
    overlayCloseBtn.click();
    return true;
  }

  const topMask = document.querySelector<HTMLElement>(
    ".n-modal-mask, .n-drawer-mask, .app-sheet-backdrop, .lw-backdrop, .tc-backdrop, .reader-top-bar__overlay, .reader-toc__overlay",
  );
  if (topMask) {
    topMask.click();
    return true;
  }

  const floatingOverlay = document.querySelector<HTMLElement>(
    ".n-popover, .n-dropdown-menu, .n-base-select-menu",
  );
  if (floatingOverlay) {
    return dispatchSyntheticOutsideClick();
  }

  return false;
}

function handleGlobalDismiss(): boolean {
  if (shellStatusStore.state.showLogWindow) {
    shellStatusStore.closeLogWindow();
    return true;
  }
  if (shellStatusStore.state.showTaskCenter) {
    shellStatusStore.closeTaskCenter();
    return true;
  }
  if (closeTopOverlay()) {
    return true;
  }
  if (navigationStore.activeView !== "bookshelf") {
    navigationStore.setActiveView("bookshelf");
    return true;
  }
  return false;
}

function handleGlobalBack(): boolean {
  // 优先交由全局返回键堆栈处理（各组件注册的 handler）
  if (backStackStore.onKeyBack()) {
    return true;
  }
  return handleGlobalDismiss();
}

function _onPopState() {
  if (backStackStore.onPopState()) {
    return;
  }
  handleGlobalDismiss();
}

onMounted(() => {
  // [BOOT] App 首屏 mounted 打点，配合 main.ts 的 _bootT0 计算前端首帧耗时
  log.info("App", "App.vue onMounted", { t: Date.now() });
  void ensureAppConfig();
  void shellStatusStore.install();
  // 全局移动端返回（Android/Tauri/Harmony 映射到 popstate）与 Esc/BrowserBack 共用同一套关闭链
  window.addEventListener("popstate", _onPopState);
  window.addEventListener("legado:app-paused", _onAndroidAppPaused);
  window.addEventListener("legado:app-resumed", _onAndroidAppResumed);
  installReaderAwakeActivityListeners();
  _uninstallGlobalFocus = installGlobalFocusNavigation({
    onBack: handleGlobalBack,
    onEscape: handleGlobalBack,
  });
  if (typeof window !== "undefined" && window.matchMedia) {
    _mq = window.matchMedia("(prefers-color-scheme: dark)");
    _mq.addEventListener("change", _onMqChange);
  }
  setupPrivacyModeAutoExit();
  _unlistenSyncClientState = installSyncClientStateListener();
  appConfigStore.installChangedListener();
  void sync.syncNow("sync").catch(() => {});
  // 通知 Rust 端应用已启动（触发 startup 同步策略）
  void sync.notifyLifecycle("startup").catch(() => {});
  // 从 Rust 侧获取准确平台信息（修复 Android 被识别为 Linux 的问题）
  void initPlatformFromRust();
  // 监听页面可见性变化，通知 Rust 端 resume/background 生命周期事件
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", _onVisibilityChange);
  }

  // ── 书源超限检查（启动时触发，非阻塞） ──────────────────────────────────
  void bookSourceStore
    .loadSources()
    .then(() => {
      const enabledCount = bookSourceStore.enabledSources.length;
      if (enabledCount > 30) {
        enabledBookSourceCount.value = enabledCount;
        showBookSourceLimitWarning.value = true;
      }
    })
    .catch(() => {
      // 加载失败时忽略，不影响启动流程
    });
});
onUnmounted(() => {
  syncAndroidReaderImmersiveMode(false);
  syncAndroidReaderBrightness(false);
  syncAndroidKeepScreenOn(false);
  clearReaderAwakeTimer();
  removeReaderAwakeActivityListeners();
  window.removeEventListener("popstate", _onPopState);
  window.removeEventListener("legado:app-paused", _onAndroidAppPaused);
  window.removeEventListener("legado:app-resumed", _onAndroidAppResumed);
  _mq?.removeEventListener("change", _onMqChange);
  _unlistenSyncClientState?.();
  if (typeof document !== "undefined") {
    document.removeEventListener("visibilitychange", _onVisibilityChange);
  }
  _uninstallGlobalFocus?.();
  _uninstallGlobalFocus = null;
});

/** 当前实际生效的暗/亮状态 */
const effectiveDark = computed(() => {
  const mode = appConfig.value.ui_theme ?? "auto";
  if (mode === "dark") {
    return true;
  }
  if (mode === "light") {
    return false;
  }
  return systemPrefersDark.value;
});

// ── 主题颜色工具函数 ─────────────────────────────────────────────────────
/** 将 hex 颜色解析为 {r, g, b}（0-255） */
function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const m = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex.trim());
  if (!m) return null;
  return { r: parseInt(m[1]!, 16), g: parseInt(m[2]!, 16), b: parseInt(m[3]!, 16) };
}

/** 将 RGB 转为 hex */
function rgbToHex(r: number, g: number, b: number): string {
  const clamp = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
  return "#" + [clamp(r), clamp(g), clamp(b)].map((v) => v.toString(16).padStart(2, "0")).join("");
}

/** 调整亮度：向白（amount>0）或向黑（amount<0）线性移动，返回新 hex */
function adjustLightness(hex: string, amount: number): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;
  const target = amount > 0 ? 255 : 0;
  const r = Math.round(rgb.r + (target - rgb.r) * Math.abs(amount));
  const g = Math.round(rgb.g + (target - rgb.g) * Math.abs(amount));
  const b = Math.round(rgb.b + (target - rgb.b) * Math.abs(amount));
  return rgbToHex(r, g, b);
}

/** 获取用户自定义主题色（空字符串=使用默认） */
const customThemeBase = computed(() => {
  const c = appConfig.value.ui_theme_color;
  return c && /^#[0-9a-fA-F]{6}$/.test(c) ? c : "";
});

/** Naive UI 主题（null = 亮色） */
const naiveTheme = computed<GlobalTheme | null>(() => (effectiveDark.value ? darkTheme : null));

// vConsole 调试面板（由开发设置开关控制）
useVConsole(effectiveDark);
// Chii 远程调试注入（由开发设置开关控制，覆盖 Tauri 与服务模式）
useRemoteDebug();

const naiveThemeOverrides = computed<GlobalThemeOverrides>(() => {
  // 默认品牌色
  const lightBase = customThemeBase.value || "#6366f1";
  const darkBase = customThemeBase.value || "#818cf8";

  if (effectiveDark.value) {
    return {
      common: {
        primaryColor: darkBase,
        primaryColorHover: adjustLightness(darkBase, 0.12),
        primaryColorPressed: adjustLightness(darkBase, -0.12),
        primaryColorSuppl: darkBase,
        infoColor: darkBase,
        successColor: "#4ade80",
        warningColor: "#fbbf24",
        errorColor: "#f87171",
        bodyColor: "#18181b",
        cardColor: "#27272a",
        modalColor: "#27272a",
        popoverColor: "#27272a",
        tableColor: "#27272a",
        dividerColor: "#3f3f46",
        borderColor: "#3f3f46",
        inputColor: "#27272a",
        actionColor: adjustLightness(darkBase, 0.3),
        hoverColor: "rgba(255, 255, 255, 0.06)",
        textColorBase: "#fafafa",
        textColor1: "#fafafa",
        textColor2: "#a1a1aa",
        textColor3: "#71717a",
      },
    };
  }

  return {
    common: {
      primaryColor: lightBase,
      primaryColorHover: adjustLightness(lightBase, 0.08),
      primaryColorPressed: adjustLightness(lightBase, -0.08),
      primaryColorSuppl: lightBase,
      infoColor: lightBase,
      successColor: "#22c55e",
      warningColor: "#f59e0b",
      errorColor: "#ef4444",
      bodyColor: "#f4f4f5",
      cardColor: "#ffffff",
      modalColor: "#ffffff",
      popoverColor: "#ffffff",
      tableColor: "#ffffff",
      dividerColor: "#e4e4e7",
      borderColor: "#e4e4e7",
      inputColor: "#ffffff",
      actionColor: adjustLightness(lightBase, 0.45),
      hoverColor: "rgba(99, 102, 241, 0.08)",
      textColorBase: "#18181b",
      textColor1: "#18181b",
      textColor2: "#52525b",
      textColor3: "#71717a",
    },
  };
});

// 将 data-theme 属性同步到 <html>️元素，驱动 CSS 变量
// 同时同步用户自定义主题色 CSS 变量
watch(
  [effectiveDark, customThemeBase],
  ([isDark, themeColor]) => {
    document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");
    // 应用自定义主题色到 CSS 变量，覆盖 theme.css 中的 brand 默认值
    if (themeColor) {
      const lightAccent = themeColor;
      const hoverAccent = adjustLightness(themeColor, isDark ? 0.12 : 0.08);
      const softAccent = isDark ? themeColor + "28" : adjustLightness(themeColor, 0.45);
      document.documentElement.style.setProperty("--color-accent", lightAccent);
      document.documentElement.style.setProperty("--color-accent-hover", hoverAccent);
      document.documentElement.style.setProperty("--color-accent-soft", softAccent);
    } else {
      // 恢复默认（由 theme.css 的 :root / [data-theme] 规则接管）
      document.documentElement.style.removeProperty("--color-accent");
      document.documentElement.style.removeProperty("--color-accent-hover");
      document.documentElement.style.removeProperty("--color-accent-soft");
    }
  },
  { immediate: true },
);
void ensureAppConfig().then(() => setLayoutMode(appConfig.value.ui_layout_mode));
watch(
  () => appConfig.value.ui_layout_mode,
  (mode) => setLayoutMode(mode),
);

function onNavSelect(id: string) {
  if (!isMobile.value && navigationStore.activeView === id) {
    void eventEmit("app:view-reload", { view: id, reason: "nav-reselect" });
    return;
  }
  navigationStore.setActiveView(id);
}

const vueVersion = computed(() => packageJson.version || "0.0.0");
// Tauri 壳版本：仅在 Tauri 环境下传给 TaskBar；鸿蒙版本暂不对接
const tauriVersion = computed(() => (isTauri ? tauriConfig.version || "" : ""));
const { logZoneEnabled: showLogZone } = useLogZonePref();
const latestLogMessage = computed(() => shellStatusStore.latestLog?.message ?? "暂无日志");
</script>

<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="naiveThemeOverrides">
    <n-message-provider
      :container-style="{
        paddingTop: 'max(env(safe-area-inset-top, 0px), 44px)',
      }"
    >
      <n-notification-provider>
        <n-dialog-provider>
          <!-- 主窗口布局 -->
          <div
            class="app-layout"
            :class="{
              'app-layout--mobile': isMobile,
              'app-layout--mobile-desktop-bar': isMobile && forceDesktopBar,
              'app-layout--macos': platform === 'macOS',
            }"
          >
            <TitleBar :title="isMobile ? activeNavLabel : '开源阅读'" />
            <SideBar
              v-if="!isMobile"
              :items="navItems"
              :active-id="activeView"
              @select="onNavSelect"
            />
            <MainContent>
              <!-- 移动端视图首次加载遮罩：仅覆盖内容区，不遮盖底部导航 -->
              <Transition name="mask-fade">
                <div v-if="isMobile && showLoadingMask" class="view-loading-mask">
                  <n-spin size="large" />
                </div>
              </Transition>
              <template v-for="viewId in ALL_VIEW_IDS" :key="viewId">
                <div
                  v-if="mountedViews.has(viewId)"
                  v-show="activeView === viewId"
                  class="view-cache-wrapper"
                >
                  <Suspense @resolve="onSuspenseResolve(viewId)">
                    <component :is="viewMap[viewId]" />
                    <template #fallback>
                      <div class="view-loading">加载中…</div>
                    </template>
                  </Suspense>
                </div>
              </template>
            </MainContent>
            <TaskBar
              v-if="!isMobile"
              :latest-log-level="shellStatusStore.latestLogLevel"
              :latest-log-message="latestLogMessage"
              :vue-version="vueVersion"
              :tauri-version="tauriVersion"
              :platform-label="platform || '-'"
              :show-log-zone="showLogZone"
              @toggle-log-window="shellStatusStore.toggleLogWindow"
              @open-about="navigationStore.setActiveView('settings')"
            />
            <BottomNav
              v-if="isMobile"
              :items="navItems"
              :active-id="activeView"
              @select="onNavSelect"
            />
          </div>
          <GlobalFeedbackMirror />
          <!-- 全局预缓存进度条：主动缓存时显示，自动缓存时静默 -->
          <PrefetchProgressBar />
          <!-- 全局音乐播放器：mini 条 + 全屏播放页 -->
          <MiniPlayerBar />
          <MusicPlayerOverlay />
          <!-- 全局脚本交互弹窗：响应 Boa 引擎 legado.ui.emit / script_dialog_open -->
          <ScriptDialog />
          <!-- 前端插件声明式交互弹窗：书架动作/封面生成器等能力共用 -->
          <FrontendPluginDialog />
          <!-- 非 Tauri 模式后端连接失败时弹出地址输入框 -->
          <WsConnectDialog v-if="!hasNativeTransport" />
          <!-- 全局 legado:// 书源安装确认 -->
          <LegadoDeepLinkDialog />
          <!-- 书源超限启动警告 -->
          <BookSourceLimitWarningDialog
            :show="showBookSourceLimitWarning"
            :enabled-count="enabledBookSourceCount"
            @update:show="(v) => (showBookSourceLimitWarning = v)"
          />
          <TaskCenterDrawer
            :show="shellStatusStore.state.showTaskCenter"
            :running-tasks="shellStatusStore.state.runningTasks"
            :queued-tasks="shellStatusStore.state.queuedTasks"
            :completed-tasks="shellStatusStore.state.completedTasks"
            :failed-tasks="shellStatusStore.state.failedTasks"
            @update:show="(v) => (shellStatusStore.state.showTaskCenter = v)"
            @close="shellStatusStore.closeTaskCenter"
            @open-log="shellStatusStore.openLogWindow"
          />
          <LogWindowPanel
            :show="shellStatusStore.state.showLogWindow"
            @update:show="(v) => (shellStatusStore.state.showLogWindow = v)"
            @close="shellStatusStore.closeLogWindow"
          />
        </n-dialog-provider>
      </n-notification-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
/* 移动端视图缓存容器：flex:1 确保占满高度，隐藏时 display:none 不占空间 */
.view-cache-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

/* 加载遮罩：position:absolute 只覆盖 MainContent 内容区（MainContent 已设 position:relative）*/
.view-loading-mask {
  position: absolute;
  inset: 0;
  z-index: 9000;
  background: var(--color-sidebar-bg);
  display: flex;
  align-items: center;
  justify-content: center;
}

.mask-fade-enter-active,
.mask-fade-leave-active {
  transition: opacity 0.15s ease;
}

.mask-fade-enter-from,
.mask-fade-leave-to {
  opacity: 0;
}

.app-layout {
  display: grid;
  grid-template-areas:
    "sidebar title"
    "sidebar main"
    "sidebar taskbar";
  grid-template-rows: var(--topbar-height) 1fr var(--bottom-bar-height);
  grid-template-columns: var(--sidebar-w) 1fr;
  height: 100vh;
  height: 100dvh;
  /* 全局底层背景：纹理渐变 + 基色，侧边栏/顶栏/底栏透明后透出此层 */
  background-color: var(--color-sidebar-bg);
  background-image: var(--app-bg-texture);
}

/* 移动端：单列布局，顶栏（状态栏避让）+ 内容 + 底部导航 */
.app-layout--mobile {
  grid-template-areas:
    "title"
    "main"
    "bottomnav";
  grid-template-rows:
    var(--safe-top)
    1fr calc(var(--bottomnav-h) + var(--safe-bottom));
  grid-template-columns: 1fr;
}

/* Windows 强制手机布局时：顶栏行用固定高度（用于拖拽/窗口控制），底部导航保留正常高度 */
.app-layout--mobile-desktop-bar {
  grid-template-rows: var(--topbar-height) 1fr var(--nav-height);
}

.app-layout--macos {
  grid-template-rows: 0 1fr var(--bottom-bar-height);
}

.app-layout--macos :deep(.title-bar) {
  display: none;
}

.placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.view-loading {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-muted);
  font-size: 0.875rem;
}
</style>
