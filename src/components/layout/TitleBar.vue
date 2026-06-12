<script setup lang="ts">
import { Minus, Square, Copy, X } from "lucide-vue-next";
import { ref, computed, onMounted, onUnmounted } from "vue";
import { isTauri, isMobile, platform } from "@/composables/useEnv";

/** Windows 桌面端即使切换手机布局，也保留完整标题栏（拖拽 + 窗口控制） */
const forceDesktopBar = computed(() => isTauri && platform.value === "Windows");
const isMacOS = computed(() => platform.value === "macOS");
const showDesktopControls = computed(() => isTauri && !isMacOS.value);

withDefaults(
  defineProps<{
    title?: string;
  }>(),
  {
    title: "开源阅读",
  },
);

const isMaximized = ref(false);
type AppWindowApi = {
  isMaximized: () => Promise<boolean>;
  minimize: () => Promise<void>;
  toggleMaximize: () => Promise<void>;
  close: () => Promise<void>;
  onResized: (handler: () => void) => Promise<() => void>;
};

let appWindow: AppWindowApi | null = null;
let unlisten: (() => void) | undefined;
let iconSize = 15;

async function ensureAppWindow(): Promise<AppWindowApi | null> {
  if (appWindow) {
    return appWindow;
  }
  if (!isTauri) {
    return null;
  }
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    appWindow = getCurrentWindow() as AppWindowApi;
    return appWindow;
  } catch (error) {
    console.warn("[TitleBar] 获取窗口对象失败:", error);
    return null;
  }
}

async function refreshMaximized() {
  const currentWindow = await ensureAppWindow();
  if (!currentWindow) {
    return;
  }
  try {
    isMaximized.value = await currentWindow.isMaximized();
  } catch (error) {
    console.warn("[TitleBar] 刷新窗口状态失败:", error);
  }
}

async function minimize() {
  const currentWindow = await ensureAppWindow();
  if (!currentWindow) {
    return;
  }
  try {
    await currentWindow.minimize();
  } catch (error) {
    console.warn("[TitleBar] 最小化失败:", error);
  }
}

async function toggleMaximize() {
  const currentWindow = await ensureAppWindow();
  if (!currentWindow) {
    return;
  }
  try {
    await currentWindow.toggleMaximize();
    await refreshMaximized();
  } catch (error) {
    console.warn("[TitleBar] 最大化/还原失败:", error);
  }
}

async function closeWindow() {
  const currentWindow = await ensureAppWindow();
  if (!currentWindow) {
    return;
  }
  try {
    await currentWindow.close();
  } catch (error) {
    console.warn("[TitleBar] 关闭窗口失败:", error);
  }
}

onMounted(async () => {
  if (!isTauri) {
    return;
  }
  try {
    const currentWindow = await ensureAppWindow();
    if (!currentWindow) {
      return;
    }
    await refreshMaximized();
    unlisten = await currentWindow.onResized(() => {
      void refreshMaximized();
    });
  } catch (error) {
    console.warn("[TitleBar] 初始化窗口控制失败:", error);
  }
});

onUnmounted(() => {
  unlisten?.();
});
</script>

<template>
  <!-- 移动端：纯状态栏颜色遮罩，高度由 grid row（env safe-area-inset-top）决定，无文字 -->
  <header
    v-if="isMobile && !forceDesktopBar"
    class="title-bar title-bar--mobile"
    aria-hidden="true"
  />
  <!-- 桌面端：完整标题栏 + 窗口控制 -->
  <header v-else class="title-bar">
    <span v-if="isMobile" class="title-bar__title" data-tauri-drag-region>{{ title }}</span>
    <div class="title-bar__spacer" data-tauri-drag-region />
    <!-- 仅 Tauri 桌面环境显示窗口控制按钮 -->
    <div v-if="showDesktopControls" class="title-bar__controls">
      <button
        class="ctrl-btn ctrl-btn--minimize"
        aria-label="最小化"
        tabindex="0"
        @pointerdown.stop
        @click="minimize"
      >
        <Minus :size="iconSize" />
      </button>
      <button
        class="ctrl-btn ctrl-btn--maximize"
        :aria-label="isMaximized ? '还原' : '最大化'"
        tabindex="0"
        @pointerdown.stop
        @click="toggleMaximize"
      >
        <Copy v-if="isMaximized" :size="iconSize" />
        <Square v-else :size="iconSize" />
      </button>
      <button
        class="ctrl-btn ctrl-btn--close"
        aria-label="关闭"
        tabindex="0"
        @pointerdown.stop
        @click="closeWindow"
      >
        <X :size="iconSize" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.title-bar {
  grid-area: title;
  display: flex;
  align-items: center;
  height: var(--topbar-height);
  padding-left: var(--space-4);
  background: transparent;
  user-select: none;
  -webkit-app-region: no-drag;
}

.title-bar__title {
  font-size: var(--fs-14);
  font-weight: var(--fw-semibold);
  color: var(--color-text);
  letter-spacing: 0.02em;
  flex-shrink: 0;
  -webkit-app-region: drag;
}

.title-bar__spacer {
  flex: 1;
  height: 100%;
  -webkit-app-region: drag;
}

/* 窗口控制按钮区 */
.title-bar__controls {
  display: flex;
  align-items: stretch;
  height: 100%;
  -webkit-app-region: no-drag;
}

.ctrl-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--color-text-soft);
  cursor: pointer;
  transition:
    background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
  -webkit-app-region: no-drag;
}

.ctrl-btn:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: -2px;
}

@media (hover: hover) and (pointer: fine) {
  .ctrl-btn:hover {
    background: var(--color-hover);
    color: var(--color-text);
  }

  .ctrl-btn--close:hover {
    background: var(--color-danger);
    color: var(--color-text-inverse);
  }
}

.ctrl-btn svg {
  pointer-events: none;
}

/* ── 移动端顶栏：仅作状态栏背景遮盖，高度 = grid row (env safe-area-inset-top) ── */
.title-bar--mobile {
  background: transparent;
  border-bottom: none;
  -webkit-app-region: none;
}
</style>
