<script setup lang="ts">
import { Folder } from "lucide-vue-next";
import { useMessage } from "naive-ui";
import { storeToRefs } from "pinia";
import { ref, computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, watch } from "vue";
import type {
  DebugSourceTabInstance,
  InstalledSourcesTabInstance,
  OnlineSourcesTabInstance,
} from "@/types";
import { useCapabilities } from "@/composables/useCapabilities";
import { eventEmit, eventListen } from "@/composables/useEventBus";
import { invokeWithTimeout } from "@/composables/useInvoke";
import { useMobileHorizontalSwipe } from "@/composables/useMobileHorizontalSwipe";
import { useBookSourceStore, useNavigationStore } from "@/stores";
import AppPageHeader from "../components/layout/AppPageHeader.vue";
import MobileToolbarMenu from "../components/layout/MobileToolbarMenu.vue";
import { type BookSourceMeta, getBookSourceDir } from "../composables/useBookSource";
import { isMobile } from "../composables/useEnv";

const message = useMessage();
const bookSourceStore = useBookSourceStore();
const navigationStore = useNavigationStore();
const capabilities = useCapabilities();
const repositoryCapability = capabilities.getCapability("repository");
const repositoryDisabled = computed(() => !repositoryCapability.value.supported);
void capabilities.loadCapabilities();

// sources / loading / streamingLoaded 直接响应式引用 store，流式批次到达时自动更新
const { sources, loading, sourceDirs: storeDirs, streamingLoaded } = storeToRefs(bookSourceStore);
const { onlineRepoDeepLinkRequest, bookSourceImportDeepLinkRequest } = storeToRefs(navigationStore);

type BookSourceTab = "installed" | "online" | "debug" | "test" | "ai";
const BOOK_SOURCE_TABS: BookSourceTab[] = ["installed", "online", "debug", "test", "ai"];

const InstalledSourcesTab = defineAsyncComponent(
  () => import("../components/booksource/InstalledSourcesTab.vue"),
);
const OnlineSourcesTab = defineAsyncComponent(
  () => import("../components/booksource/OnlineSourcesTab.vue"),
);
const DebugSourceTab = defineAsyncComponent(
  () => import("../components/booksource/DebugSourceTab.vue"),
);
const TestSourcesTab = defineAsyncComponent(
  () => import("../components/booksource/TestSourcesTab.vue"),
);
const AiSourceTab = defineAsyncComponent(() => import("../components/booksource/AiSourceTab.vue"));

const activeTab = ref<BookSourceTab>("installed");

// ---- 共享状态 ----
const sourceDir = ref("");
const sourceDirs = computed(() => storeDirs.value);

const shortSourceDir = computed(() => {
  if (!sourceDir.value) {
    return "";
  }
  const sep = sourceDir.value.includes("\\") ? "\\" : "/";
  const parts = sourceDir.value.split(sep).filter(Boolean);
  if (parts.length <= 3) {
    return sourceDir.value;
  }
  return `…${sep}${parts.slice(-2).join(sep)}`;
});

async function openSourceDirInExplorer() {
  if (!sourceDir.value) {
    return;
  }
  try {
    await invokeWithTimeout("open_dir_in_explorer", { path: sourceDir.value });
  } catch (e: unknown) {
    message.error(`无法打开目录: ${e instanceof Error ? e.message : String(e)}`);
  }
}

// 防止 loadSources 并发执行：正在加载时跳过后续调用
let _loadSourcesInFlight = false;

async function loadSources(options: { force?: boolean } = {}) {
  if (_loadSourcesInFlight) {
    return;
  }
  _loadSourcesInFlight = true;
  try {
    await bookSourceStore.loadSources(options);
    const cachedDir = storeDirs.value[0];
    sourceDir.value = cachedDir || sourceDir.value || (await getBookSourceDir());
    // sourceDirs 通过 storeToRefs 已响应式绑定，无需手动同步
  } catch (e: unknown) {
    message.error(`加载失败: ${e instanceof Error ? e.message : String(e)}`);
  } finally {
    _loadSourcesInFlight = false;
  }
}

// ---- 子组件 refs ----
const installedRef = ref<InstalledSourcesTabInstance | null>(null);
const onlineRef = ref<OnlineSourcesTabInstance | null>(null);
const debugRef = ref<DebugSourceTabInstance | null>(null);
const pendingDebugSource = ref<BookSourceMeta | null>(null);

function onNavigateTab(tab: string) {
  if (BOOK_SOURCE_TABS.includes(tab as BookSourceTab)) {
    activeTab.value = tab as BookSourceTab;
  }
}

function onSelectDebugSource(source: BookSourceMeta) {
  pendingDebugSource.value = source;
  activeTab.value = "debug";
}

async function handleOnlineRepoDeepLinkRequest() {
  const request = onlineRepoDeepLinkRequest.value;
  if (!request) {
    return;
  }

  activeTab.value = "online";
  await nextTick();
  if (!onlineRef.value) {
    await nextTick();
  }
  if (!onlineRef.value) {
    return;
  }

  onlineRef.value.openAddRepoFromDeepLink(request.url, request.name);
  navigationStore.consumeOnlineRepoDeepLinkRequest(request.id);
}

async function handleBookSourceImportDeepLinkRequest() {
  const request = bookSourceImportDeepLinkRequest.value;
  if (!request) {
    return;
  }

  activeTab.value = "installed";
  await nextTick();
  if (!installedRef.value) {
    await nextTick();
  }
  if (!installedRef.value) {
    return;
  }

  await installedRef.value.importLegacyJsonFromDeepLink(request.url);
  navigationStore.consumeBookSourceImportDeepLinkRequest(request.id);
}

watch(
  () => [activeTab.value, debugRef.value, pendingDebugSource.value] as const,
  async ([tab, debugInstance, source]) => {
    if (tab !== "debug" || !debugInstance || !source) {
      return;
    }
    await nextTick();
    debugInstance.setDebugSource(source);
    pendingDebugSource.value = null;
  },
  { flush: "post" },
);

watch(
  () => [onlineRepoDeepLinkRequest.value?.id, onlineRef.value] as const,
  () => {
    void handleOnlineRepoDeepLinkRequest();
  },
  { flush: "post", immediate: true },
);

watch(
  () => [bookSourceImportDeepLinkRequest.value?.id, installedRef.value] as const,
  () => {
    void handleBookSourceImportDeepLinkRequest();
  },
  { flush: "post", immediate: true },
);

function switchActiveTab(direction: "prev" | "next") {
  const idx = BOOK_SOURCE_TABS.indexOf(activeTab.value);
  if (idx < 0) {
    return;
  }
  const nextIdx = direction === "next" ? idx + 1 : idx - 1;
  if (nextIdx < 0 || nextIdx >= BOOK_SOURCE_TABS.length) {
    return;
  }
  activeTab.value = BOOK_SOURCE_TABS[nextIdx];
}

const {
  onSwipePointerDown,
  onSwipePointerMove,
  onSwipePointerUp,
  onSwipePointerCancel,
  onSwipeClickCapture,
} = useMobileHorizontalSwipe({
  onSwipeLeft: () => switchActiveTab("next"),
  onSwipeRight: () => switchActiveTab("prev"),
});

// ── 移动端工具栏菜单 ──────────────────────────────────────────────────
const newSourceOptions = [
  { label: "小说书源", key: "new-novel" },
  { label: "视频书源", key: "new-video" },
];

const legacyJsonImportOptions = [
  { label: "选择文件导入", key: "import-legacy-file" },
  { label: "输入 URL 导入", key: "import-legacy-url" },
  { label: "喵公子订阅", key: "import-miaogongzi" },
];

const mobileMenuOptions = computed(() => [
  { label: "目录管理", key: "dir" },
  { label: "导入本地", key: "import-file" },
  { label: "导入在线", key: "import-online" },
  { label: "阅读源文件导入", key: "import-legacy-file" },
  { label: "阅读源 URL 导入", key: "import-legacy-url" },
  { label: "喵公子订阅", key: "import-miaogongzi" },
  { label: "导出书源", key: "export-file" },
  { label: "新建书源", key: "new", children: newSourceOptions },
  { label: "全部重载", key: "reload", disabled: loading.value },
]);

const onlineMenuOptions = computed(() => [
  { label: "获取列表", key: "fetch-online", disabled: repositoryDisabled.value },
  { label: "添加仓库", key: "add-online-repo" },
  { label: "移除仓库", key: "remove-online-repo" },
  { label: "重新检查", key: "recheck-online", disabled: repositoryDisabled.value },
  { label: "批量安装", key: "install-all-online", disabled: repositoryDisabled.value },
  { label: "批量更新", key: "update-all-online", disabled: repositoryDisabled.value },
  { label: "批量强制更新", key: "force-update-all-online", disabled: repositoryDisabled.value },
]);

const onlineBatchOptions = computed(() => [
  { label: "重新检查", key: "recheck-online", disabled: repositoryDisabled.value },
  { label: "批量安装", key: "install-all-online", disabled: repositoryDisabled.value },
  { label: "批量更新", key: "update-all-online", disabled: repositoryDisabled.value },
  { label: "批量强制更新", key: "force-update-all-online", disabled: repositoryDisabled.value },
]);

function handleMobileMenuSelect(key: string) {
  switch (key) {
    case "dir":
      installedRef.value?.openDirManager();
      break;
    case "import-file":
      installedRef.value?.importFromFile();
      break;
    case "import-online":
      installedRef.value?.importFromUrl();
      break;
    case "import-legacy-file":
      installedRef.value?.importLegacyJsonFromFile();
      break;
    case "import-legacy-url":
      installedRef.value?.importLegacyJsonFromUrl();
      break;
    case "import-miaogongzi":
      installedRef.value?.importMiaoGongziSubscription();
      break;
    case "export-file":
      void installedRef.value?.exportSources();
      break;
    case "new":
    case "new-novel":
      installedRef.value?.openEditor(undefined, "novel");
      break;
    case "new-video":
      installedRef.value?.openEditor(undefined, "video");
      break;
    case "reload":
      installedRef.value?.reloadAllSources();
      break;
  }
}

function handleOnlineMenuSelect(key: string) {
  switch (key) {
    case "fetch-online":
      void onlineRef.value?.fetchOnlineSources();
      break;
    case "add-online-repo":
      onlineRef.value?.openAddRepo();
      break;
    case "remove-online-repo":
      onlineRef.value?.removeActiveRepo();
      break;
    case "recheck-online":
      void onlineRef.value?.recheckInstalledSources();
      break;
    case "install-all-online":
      void onlineRef.value?.installAll();
      break;
    case "update-all-online":
      void onlineRef.value?.updateAll();
      break;
    case "force-update-all-online":
      onlineRef.value?.confirmForceUpdateAll();
      break;
  }
}

async function handleForceReload() {
  if (installedRef.value) {
    await installedRef.value.reloadAllSources();
    return;
  }
  await loadSources({ force: true });
  await eventEmit("app:booksource-reload", { scope: "all" });
}

/**
 * 子标签页（导入/删除/启停/保存/安装）后的统一刷新：除刷新本页书源列表外，
 * 还广播 app:booksource-reload，让搜索/发现/书架页即时同步，无需重启应用。
 */
async function handleInstalledReload() {
  bookSourceStore.markSourcesStale();
  await loadSources({ force: true });
  await eventEmit("app:booksource-reload", { scope: "all" });
}

// ── 初始化 ──
let unlistenFileChange: (() => void) | null = null;
let unlistenViewReload: (() => void) | null = null;
let _bsvMounted = false;

onMounted(async () => {
  _bsvMounted = true;
  await loadSources();

  // 若加载期间组件已卸载，不再注册监听
  if (!_bsvMounted) {
    return;
  }

  const unlisten = await eventListen<{ fileName?: string; reason?: string }>(
    "booksource:changed",
    async (event) => {
      const { fileName, reason } = event.payload ?? {};
      if (fileName) {
        // toggle 操作仅修改 enabled 字段，前端已就地更新，无需全量重载（避免列表滚动到顶部）
        if (reason === "toggle") {
          return;
        }
        bookSourceStore.invalidateCapability(fileName);
        installedRef.value?.handleFileChange(fileName);
      } else {
        // 批量变更（如同步后），使所有能力缓存失效
        bookSourceStore.invalidateAllCapabilities();
      }
      bookSourceStore.markSourcesStale();
      await loadSources({ force: true });
    },
  );

  // listen 返回前组件可能已卸载（快速切换视图），直接清理
  if (!_bsvMounted) {
    unlisten();
    return;
  }
  unlistenFileChange = unlisten;

  const unlistenReload = await eventListen<{ view?: string }>("app:view-reload", async (event) => {
    if (event.payload?.view === "booksource") {
      await handleForceReload();
    }
  });

  if (!_bsvMounted) {
    unlistenReload();
    return;
  }
  unlistenViewReload = unlistenReload;
});

onUnmounted(() => {
  _bsvMounted = false;
  unlistenFileChange?.();
  unlistenFileChange = null;
  unlistenViewReload?.();
  unlistenViewReload = null;
});
</script>

<template>
  <div class="booksource-view">
    <AppPageHeader title="书源管理" :divider="true" :hide-subtitle-on-mobile="true">
      <template #title-extra>
        <div
          class="bv-header__dir bv-header__dir--clickable"
          v-if="sourceDir && !isMobile"
          role="button"
          tabindex="0"
          :aria-label="`打开书源目录: ${sourceDir}`"
          :title="sourceDir"
          @click="openSourceDirInExplorer"
          @keydown.enter.prevent="openSourceDirInExplorer"
          @keydown.space.prevent="openSourceDirInExplorer"
        >
          <Folder aria-hidden="true" :size="12" />
          <span class="bv-header__dir-path">{{ shortSourceDir }}</span>
        </div>
      </template>
      <template #subtitle> 管理已安装书源、浏览在线仓库 </template>
      <template #actions>
        <template v-if="activeTab === 'installed'">
          <MobileToolbarMenu :options="mobileMenuOptions" @select="handleMobileMenuSelect">
            <n-button
              size="small"
              quaternary
              title="管理外部书源目录"
              @click="installedRef?.openDirManager()"
              >目录</n-button
            >
            <n-button size="small" quaternary @click="installedRef?.importFromFile()"
              >导入本地</n-button
            >
            <n-button size="small" quaternary @click="installedRef?.importFromUrl()"
              >导入在线</n-button
            >
            <n-dropdown
              trigger="click"
              :options="legacyJsonImportOptions"
              @select="handleMobileMenuSelect"
            >
              <n-button size="small" quaternary>导入开源阅读书源</n-button>
            </n-dropdown>
            <n-dropdown
              trigger="click"
              :options="newSourceOptions"
              @select="handleMobileMenuSelect"
            >
              <n-button size="small" type="primary">新建书源</n-button>
            </n-dropdown>
            <n-button size="small" :loading="loading" @click="installedRef?.reloadAllSources()"
              >全部重载</n-button
            >
          </MobileToolbarMenu>
        </template>
        <template v-else-if="activeTab === 'online'">
          <MobileToolbarMenu :options="onlineMenuOptions" @select="handleOnlineMenuSelect">
            <n-button
              size="small"
              type="primary"
              :disabled="repositoryDisabled"
              @click="onlineRef?.fetchOnlineSources()"
              >获取列表</n-button
            >
            <n-button size="small" quaternary @click="onlineRef?.openAddRepo()">添加仓库</n-button>
            <n-button size="small" quaternary @click="onlineRef?.removeActiveRepo()">移除</n-button>
            <n-dropdown
              trigger="click"
              :options="onlineBatchOptions"
              @select="handleOnlineMenuSelect"
            >
              <n-button size="small" quaternary :disabled="repositoryDisabled">批量操作</n-button>
            </n-dropdown>
          </MobileToolbarMenu>
        </template>
      </template>
    </AppPageHeader>

    <!-- 流式加载进度提示（仅在后台持续加载时显示） -->
    <div v-if="loading && streamingLoaded > 0" class="bv-streaming-bar">
      <n-progress
        type="line"
        :percentage="100"
        :height="2"
        :border-radius="0"
        :fill-border-radius="0"
        status="info"
        :indicator-placement="'inside'"
        :show-indicator="false"
        processing
      />
      <span class="bv-streaming-bar__text">正在加载书源 · 已加载 {{ streamingLoaded }} 条</span>
    </div>

    <!-- 主 Tabs -->
    <n-tabs
      v-model:value="activeTab"
      type="line"
      animated
      class="bv-tabs"
      @pointerdown="onSwipePointerDown"
      @pointermove="onSwipePointerMove"
      @pointerup="onSwipePointerUp"
      @pointercancel="onSwipePointerCancel"
      @click.capture="onSwipeClickCapture"
    >
      <n-tab-pane name="installed" tab="已安装书源">
        <InstalledSourcesTab
          ref="installedRef"
          :sources="sources"
          :source-dir="sourceDir"
          :source-dirs="sourceDirs"
          :loading="loading"
          @reload="handleInstalledReload"
          @navigate-tab="onNavigateTab"
          @select-debug-source="onSelectDebugSource"
        />
      </n-tab-pane>

      <n-tab-pane name="online" tab="在线书源">
        <OnlineSourcesTab
          ref="onlineRef"
          :sources="sources"
          :active="activeTab === 'online'"
          @reload="handleInstalledReload"
        />
      </n-tab-pane>

      <n-tab-pane name="debug" tab="调试书源">
        <DebugSourceTab ref="debugRef" :sources="sources" />
      </n-tab-pane>

      <n-tab-pane name="test" tab="书源测试">
        <TestSourcesTab :sources="sources" />
      </n-tab-pane>

      <n-tab-pane name="ai" tab="AI 写书源" display-directive="show:lazy">
        <div class="bv-pane bv-pane--fill">
          <AiSourceTab :sources="sources" @reload="handleInstalledReload" />
        </div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<style scoped>
/* ---- 外层 ---- */
.booksource-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* background: var(--color-bg-page); */
}

.booksource-view :deep(.app-page-header__title-group) {
  flex: 0 0 auto;
  min-width: max-content;
  flex-wrap: wrap;
}

.booksource-view :deep(.app-page-header__actions) {
  flex: 1 1 520px;
  min-width: min(100%, 360px);
  max-width: none;
  margin-left: 0;
}

.bv-header__dir {
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--color-text-muted);
  opacity: 0.6;
}

.bv-header__dir--clickable {
  cursor: pointer;
  border-radius: var(--radius-1);
  padding: 2px 8px;
  transition:
    background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard),
    opacity var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .bv-header__dir--clickable:hover {
    background: var(--color-hover);
    color: var(--color-text-soft);
    opacity: 1;
  }
}

.bv-header__dir-path {
  font-size: var(--fs-11);
  font-family: "Cascadia Code", "Consolas", monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 420px;
}

/* ---- 流式加载进度条 ---- */
.bv-streaming-bar {
  flex-shrink: 0;
  position: relative;
  height: 20px;
}

.bv-streaming-bar__text {
  position: absolute;
  right: 12px;
  top: 2px;
  font-size: var(--fs-11);
  color: var(--color-text-muted);
  pointer-events: none;
}

/* ---- Tabs ---- */
.bv-tabs {
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0 24px;
}

:deep(.n-tabs-content) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

:deep(.n-tabs-nav) {
  padding-top: 4px;
}

:deep(.n-tabs-tab) {
  font-size: var(--fs-14);
  font-weight: var(--fw-medium);
  color: var(--color-text-muted) !important;
  padding: 8px 2px;
  transition: color var(--dur-fast) var(--ease-standard);
}

:deep(.n-tabs-tab--active) {
  font-weight: var(--fw-semibold);
  color: var(--color-text) !important;
}

:deep(.n-tabs-tab:hover:not(.n-tabs-tab--active)) {
  color: var(--color-text-soft) !important;
}

:deep(.n-tabs-bar) {
  background: var(--color-accent) !important;
  height: 2px;
}

:deep(.n-tabs-pane-wrapper) {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
}

:deep(.n-tab-pane) {
  flex: 1;
  min-height: 0;
  height: 100%;
  padding: 0;
  display: flex;
  flex-direction: column;
}

/* ---- 通用 Pane ---- */
.bv-pane {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 0;
  padding-top: 12px;
}

.bv-pane--fill {
  padding-top: 0;
  overflow: hidden;
}

.bv-pane :deep(.n-spin-container) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.bv-pane :deep(.n-spin-content) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* ── 移动端适配 ── */
@media (pointer: coarse), (max-width: 640px) {
  .booksource-view :deep(.app-page-header__title-group),
  .booksource-view :deep(.app-page-header__actions) {
    flex-basis: 100%;
    min-width: 0;
  }

  .booksource-view :deep(.app-page-header__actions) {
    justify-content: flex-start;
  }

  .bv-tabs {
    min-height: 0;
    padding: 0 12px;
    touch-action: pan-y;
  }

  :deep(.n-tabs-tab) {
    padding: 6px 2px !important;
    font-size: var(--fs-13) !important;
  }
}
</style>
