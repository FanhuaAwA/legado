<!-- SearchView — 聚合/分组搜索书源内容，并承载搜索结果详情与内联阅读入口。 -->
<script setup lang="ts">
import { Image, ImageOff, LayoutGrid, List, Search } from "lucide-vue-next";
import { useMessage, type SelectOption } from "naive-ui";
import { storeToRefs } from "pinia";
import { ref, reactive, computed, onMounted, onUnmounted, watch, shallowRef } from "vue";
import type { CardSizeKey } from "@/composables/useViewCardDensity";
import type { TaggedBookItem, BookSourceMeta, BookItem, AggregatedBook } from "@/types";
import { useBookDetailDrawerState } from "@/composables/useBookDetailDrawerState";
import { eventListen } from "@/composables/useEventBus";
import { useInlineBookReader } from "@/composables/useInlineBookReader";
import { useViewCardDensity } from "@/composables/useViewCardDensity";
import {
  useBookSourceStore,
  useNavigationStore,
  useBookshelfStore,
  usePreferencesStore,
  usePrivacyModeStore,
  useScriptBridgeStore,
} from "@/stores";
import { appendTaggedResultsToGroups, sortAggregatedGroups } from "@/utils/searchAggregation";
import { safeRandomUUID } from "@/utils/uuid";
import AppEmpty from "../components/base/AppEmpty.vue";
import AggregatedSearchResults from "../components/explore/AggregatedSearchResults.vue";
import BookDetailDrawer from "../components/explore/BookDetailDrawer.vue";
import ChapterReaderModal from "../components/explore/ChapterReaderModal.vue";
import SourceSearchGroup from "../components/explore/SourceSearchGroup.vue";
import AppPageHeader from "../components/layout/AppPageHeader.vue";
import MobileToolbarMenu from "../components/layout/MobileToolbarMenu.vue";

const message = useMessage();
const bookSourceStore = useBookSourceStore();
const navigationStore = useNavigationStore();
const bookshelfStore = useBookshelfStore();
const privacyModeStore = usePrivacyModeStore();
const scriptBridgeStore = useScriptBridgeStore();
const { sources: sourcesRef } = storeToRefs(bookSourceStore);
const { runSearch, runChapterList, cancelTask } = scriptBridgeStore;
const prefsStore = usePreferencesStore();
const {
  cardSizes: CARD_SIZES,
  activeSize,
  activeSizeKey,
  style: searchDensityStyle,
  setSize,
} = useViewCardDensity("search");

// ── 书源列表 & 能力检测 ──────────────────────────────────────────────────
/** 参与搜索的书源：有 search 能力 + 用户未禁用（来自 bookSourceStore，自动响应式） */
const allSearchableSources = computed(() => bookSourceStore.searchableSources);

const ALL_SOURCES_VALUE = "__all__";
type SearchSourceType = "all" | "novel";

const selectedSearchType = ref<SearchSourceType>("all");
const sourceTypeLabels: Record<SearchSourceType, string> = {
  all: "全部小说",
  novel: "小说",
};
const sourceTypeOptions = computed(() => {
  const counts = allSearchableSources.value.reduce(
    (acc, source) => {
      const type = normalizeSourceType(source.sourceType);
      acc[type] += 1;
      acc.all += 1;
      return acc;
    },
    { all: 0, novel: 0 } satisfies Record<SearchSourceType, number>,
  );

  return (["all", "novel"] as const).map((type) => ({
    label: `${sourceTypeLabels[type]}（${counts[type]}）`,
    value: type,
  }));
});

function normalizeSourceType(_sourceType?: string | null): Exclude<SearchSourceType, "all"> {
  return "novel";
}

function renderSourceTypeLabel(option: SelectOption) {
  return String(option.label ?? "");
}

const searchableSources = computed(() => {
  if (selectedSearchType.value === "all") {
    return allSearchableSources.value;
  }
  return allSearchableSources.value.filter(
    (source) => normalizeSourceType(source.sourceType) === selectedSearchType.value,
  );
});

/** 当前限制到单一书源（来自发现页跳转或搜索页手动选择） */
const limitedSource = ref<BookSourceMeta | null>(null);
const limitedSourceName = computed(() => limitedSource.value?.name ?? "");
const selectedTypeLabel = computed(() => sourceTypeLabels[selectedSearchType.value]);

const searchScopeOptions = computed(() => [
  {
    label: `全部书源（${searchableSources.value.length}）`,
    value: ALL_SOURCES_VALUE,
  },
  ...searchableSources.value.map((source) => ({
    label: source.name,
    value: source.sourceKey,
  })),
]);

/** 实际参与搜索的书源列表 */
const activeSources = computed(() =>
  limitedSource.value ? [limitedSource.value] : searchableSources.value,
);

const searchStartDescription = computed(() =>
  limitedSource.value
    ? `输入关键词后点击搜索，仅搜索当前书源（${selectedTypeLabel.value}）`
    : `输入关键词后点击搜索，将搜索${selectedSearchType.value === "all" ? "全部可用小说书源" : `可用${selectedTypeLabel.value}书源`}`,
);

const aggregatedEmptyDescription = computed(() =>
  limitedSource.value ? `${limitedSourceName.value} 暂无搜索结果` : "当前搜索范围暂无结果",
);
const hasSearchKeyword = computed(() => searchKeyword.value.trim().length > 0);

function sourceKeyOf(source: BookSourceMeta): string {
  return source.sourceKey || `${source.sourceDir}::${source.fileName}`;
}

function sourceEventKey(fileName: string, sourceDir?: string): string {
  return sourceDir ? `${sourceDir}::${fileName}` : fileName;
}

function setSourceLimit(sourceId: string | null): boolean {
  if (!sourceId) {
    navigationStore.searchInitSource = null;
    limitedSource.value = null;
    updateHasSearchedForScope();
    return true;
  }

  const found = allSearchableSources.value.find(
    (source) => sourceKeyOf(source) === sourceId || source.fileName === sourceId,
  );
  if (!found) {
    return false;
  }

  navigationStore.searchInitSource = null;
  limitedSource.value = found;
  selectedSearchType.value = normalizeSourceType(found.sourceType);
  updateHasSearchedForScope();
  return true;
}

function clearSourceLimit() {
  setSourceLimit(null);
}

const selectedSourceValue = computed<string | null>({
  get: () => (limitedSource.value ? sourceKeyOf(limitedSource.value) : ALL_SOURCES_VALUE),
  set: (value) => {
    if (!value || value === ALL_SOURCES_VALUE) {
      clearSourceLimit();
      return;
    }
    setSourceLimit(value);
  },
});

async function loadSources() {
  try {
    await bookSourceStore.loadSources();
    // 如果有限定书源，从已知列表中查找
    if (navigationStore.searchInitSource) {
      if (!setSourceLimit(navigationStore.searchInitSource)) {
        clearSourceLimit();
      }
    }
  } catch (e: unknown) {
    message.error(`加载书源列表失败: ${e instanceof Error ? e.message : String(e)}`);
  }
}

// 监听 searchInitSource 变化（用户从发现页切换过来时可能还未 mount）
watch(
  () => navigationStore.searchInitSource,
  (val) => {
    if (val) {
      setSourceLimit(val);
    }
  },
);

watch(searchableSources, (nextSources) => {
  if (!limitedSource.value) {
    return;
  }
  const currentKey = sourceKeyOf(limitedSource.value);
  const found = nextSources.find((source) => sourceKeyOf(source) === currentKey);
  if (found) {
    limitedSource.value = found;
    return;
  }
  clearSourceLimit();
});

watch(selectedSearchType, (type) => {
  if (!limitedSource.value || type === "all") {
    return;
  }
  if (normalizeSourceType(limitedSource.value.sourceType) !== type) {
    clearSourceLimit();
  }
});

// ── 搜索 ─────────────────────────────────────────────────────────────────
const searchKeyword = ref("");
const searchRunning = ref(false);
const showCovers = ref(true);
const searchPage = ref(1);

/** 搜索模式：'grouped' 按书源分组 | 'aggregated' 聚合排序 */
const searchMode = ref<"grouped" | "aggregated">("aggregated");
const reloadingSources = ref(false);

// ── 移动端三点菜单 ──────────────────────────────────────────────────────
const mobileMenuOptions = computed(() => [
  {
    label: showCovers.value ? "隐藏封面" : "显示封面",
    key: "toggle-covers",
  },
  ...CARD_SIZES.map((size) => ({
    label: `卡片大小：${size.label}`,
    key: `size-${size.key}`,
    disabled: activeSizeKey.value === size.key,
  })),
  {
    label: "聚合模式",
    key: "mode-aggregated",
    disabled: searchMode.value === "aggregated",
  },
  {
    label: "分组模式",
    key: "mode-grouped",
    disabled: searchMode.value === "grouped",
  },
  {
    label: "强制刷新",
    key: "reload",
    disabled: reloadingSources.value || searchRunning.value,
  },
]);

function handleMobileMenuSelect(key: string) {
  if (key.startsWith("size-")) {
    setSize(key.slice(5) as CardSizeKey);
    return;
  }
  switch (key) {
    case "toggle-covers":
      showCovers.value = !showCovers.value;
      break;
    case "mode-aggregated":
      searchMode.value = "aggregated";
      break;
    case "mode-grouped":
      searchMode.value = "grouped";
      break;
    case "reload":
      void handleForceReload();
      break;
  }
}

interface SourceSearchState {
  loading: boolean;
  results: BookItem[];
  error: string;
}
interface SearchRun {
  token: number;
  keyword: string;
  page: number;
  pending: BookSourceMeta[];
  queuedKeys: Set<string>;
  searchedKeys: Set<string>;
  taskIds: Map<string, string>;
  activeWorkers: number;
}
const searchStates = reactive<Record<string, SourceSearchState>>({});
const activeSearchToken = ref(0);
let currentSearchRun: SearchRun | null = null;

const hasSearched = ref(false);
const aggregatedGroups = shallowRef<AggregatedBook[]>([]);
const completedSourceCount = ref(0);
const totalRawResultCount = ref(0);
const sourcesWithResultCount = ref(0);
const completedSourceKeys = new Set<string>();
const sourceResultCounts = new Map<string, number>();
let aggregatedGroupBuffer: AggregatedBook[] = [];
let aggregationPublishHandle: number | null = null;
let aggregationPublishViaFrame = false;

/** 聚合模式下是否仍有书源在搜索中 */
const aggregatedLoading = computed(() => searchRunning.value);

function updateHasSearchedForScope() {
  if (searchRunning.value) {
    hasSearched.value = true;
    return;
  }
  const limited = limitedSource.value;
  if (!limited) {
    hasSearched.value = sourceResultCounts.size > 0 || completedSourceKeys.size > 0;
    return;
  }
  const key = sourceKeyOf(limited);
  hasSearched.value = sourceResultCounts.has(key) || completedSourceKeys.has(key);
}

function cancelAggregatedGroupPublish() {
  if (aggregationPublishHandle === null) {
    return;
  }
  if (typeof window !== "undefined" && aggregationPublishViaFrame) {
    window.cancelAnimationFrame(aggregationPublishHandle);
  } else if (typeof window !== "undefined") {
    window.clearTimeout(aggregationPublishHandle);
  } else {
    globalThis.clearTimeout(aggregationPublishHandle);
  }
  aggregationPublishHandle = null;
  aggregationPublishViaFrame = false;
}

function scheduleAggregatedGroupPublish() {
  if (aggregationPublishHandle !== null) {
    return;
  }
  const publish = () => {
    aggregationPublishHandle = null;
    aggregationPublishViaFrame = false;
    sortAggregatedGroups(aggregatedGroupBuffer);
    aggregatedGroups.value = aggregatedGroupBuffer.slice();
  };
  if (typeof window !== "undefined" && typeof window.requestAnimationFrame === "function") {
    aggregationPublishViaFrame = true;
    aggregationPublishHandle = window.requestAnimationFrame(publish);
  } else if (typeof window !== "undefined") {
    aggregationPublishViaFrame = false;
    aggregationPublishHandle = window.setTimeout(publish, 16);
  } else {
    aggregationPublishViaFrame = false;
    aggregationPublishHandle = globalThis.setTimeout(publish, 16) as unknown as number;
  }
}

function resetSearchDerivedState() {
  cancelAggregatedGroupPublish();
  aggregatedGroupBuffer = [];
  aggregatedGroups.value = [];
  completedSourceKeys.clear();
  sourceResultCounts.clear();
  completedSourceCount.value = 0;
  totalRawResultCount.value = 0;
  sourcesWithResultCount.value = 0;
}

function markSourceCompleted(key: string) {
  if (completedSourceKeys.has(key)) {
    return;
  }
  completedSourceKeys.add(key);
  completedSourceCount.value += 1;
}

function applySourceResults(run: SearchRun, src: BookSourceMeta, results: BookItem[]) {
  const key = sourceKeyOf(src);
  searchStates[key].results = results;

  const oldCount = sourceResultCounts.get(key) ?? 0;
  if (oldCount > 0) {
    sourcesWithResultCount.value -= 1;
    totalRawResultCount.value -= oldCount;
  }
  sourceResultCounts.set(key, results.length);
  if (results.length > 0) {
    sourcesWithResultCount.value += 1;
    totalRawResultCount.value += results.length;
  }

  const taggedResults: TaggedBookItem[] = results.map((book) => ({
    book,
    fileName: src.fileName,
    sourceDir: src.sourceDir,
    sourceName: src.name,
    sourceLogo: src.logo,
  }));
  appendTaggedResultsToGroups(aggregatedGroupBuffer, taggedResults, run.keyword);
  scheduleAggregatedGroupPublish();
}

/** 立即终止当前搜索，清除所有进行中状态 */
function stopSearch() {
  const run = currentSearchRun;
  activeSearchToken.value += 1;
  currentSearchRun = null;
  searchRunning.value = false;
  if (run) {
    for (const taskId of run.taskIds.values()) {
      void cancelTask(taskId);
    }
    run.taskIds.clear();
  }
  for (const src of activeSources.value) {
    const key = sourceKeyOf(src);
    if (searchStates[key]?.loading) {
      searchStates[key].loading = false;
      markSourceCompleted(key);
    }
  }
}

function searchConcurrencyLimit(): number {
  return Math.max(1, prefsStore.search.searchConcurrency || 5);
}

function finishSearchIfIdle(run: SearchRun) {
  if (currentSearchRun !== run || run.token !== activeSearchToken.value) {
    return;
  }
  if (run.pending.length > 0 || run.activeWorkers > 0) {
    return;
  }
  if (bookSourceStore.loading && !limitedSource.value) {
    return;
  }
  searchRunning.value = false;
  currentSearchRun = null;
}

async function searchOneSource(run: SearchRun, src: BookSourceMeta) {
  const key = sourceKeyOf(src);
  const taskId = `search-${run.token}-${safeRandomUUID()}`;
  run.taskIds.set(key, taskId);
  try {
    const raw = await runSearch(src.fileName, run.keyword, run.page, src.sourceDir, taskId);
    if (currentSearchRun !== run || run.token !== activeSearchToken.value) {
      return;
    }
    applySourceResults(run, src, Array.isArray(raw) ? (raw as BookItem[]) : []);
  } catch (e: unknown) {
    if (currentSearchRun !== run || run.token !== activeSearchToken.value) {
      return;
    }
    searchStates[key].error = e instanceof Error ? e.message : String(e);
  } finally {
    run.taskIds.delete(key);
    run.activeWorkers = Math.max(0, run.activeWorkers - 1);
    if (currentSearchRun === run && run.token === activeSearchToken.value) {
      searchStates[key].loading = false;
      markSourceCompleted(key);
      pumpSearchQueue(run);
      finishSearchIfIdle(run);
    }
  }
}

function pumpSearchQueue(run: SearchRun) {
  if (currentSearchRun !== run || run.token !== activeSearchToken.value) {
    return;
  }
  const limit = searchConcurrencyLimit();
  while (run.activeWorkers < limit && run.pending.length > 0) {
    const src = run.pending.shift();
    if (!src) {
      continue;
    }
    const key = sourceKeyOf(src);
    run.queuedKeys.delete(key);
    run.searchedKeys.add(key);
    run.activeWorkers += 1;
    void searchOneSource(run, src);
  }
}

function enqueueSearchSources(run: SearchRun, nextSources: readonly BookSourceMeta[]) {
  if (currentSearchRun !== run || run.token !== activeSearchToken.value) {
    return;
  }
  for (const src of nextSources) {
    const key = sourceKeyOf(src);
    if (run.searchedKeys.has(key) || run.queuedKeys.has(key)) {
      continue;
    }
    run.queuedKeys.add(key);
    run.pending.push(src);
    searchStates[key] = { loading: true, results: [], error: "" };
    completedSourceKeys.delete(key);
    sourceResultCounts.set(key, 0);
  }
  pumpSearchQueue(run);
  finishSearchIfIdle(run);
}

async function doSearch(page = 1) {
  const kw = searchKeyword.value.trim();
  if (!kw) {
    message.warning("请输入搜索关键词");
    return;
  }
  if (searchRunning.value) {
    message.warning('搜索进行中，请先点击"停止搜索"再发起新搜索');
    return;
  }
  if (!activeSources.value.length && !bookSourceStore.loading) {
    message.warning("没有可用的搜索书源");
    return;
  }

  const requestToken = activeSearchToken.value + 1;
  activeSearchToken.value = requestToken;
  searchPage.value = page;
  searchRunning.value = true;
  hasSearched.value = true;
  resetSearchDerivedState();

  const run: SearchRun = {
    token: requestToken,
    keyword: kw,
    page,
    pending: [],
    queuedKeys: new Set(),
    searchedKeys: new Set(),
    taskIds: new Map(),
    activeWorkers: 0,
  };
  currentSearchRun = run;
  enqueueSearchSources(run, activeSources.value);
}

watch(activeSources, (nextSources) => {
  if (!currentSearchRun || !searchRunning.value) {
    return;
  }
  enqueueSearchSources(currentSearchRun, nextSources);
});

watch(
  () => bookSourceStore.loading,
  (loading) => {
    if (loading || !currentSearchRun || !searchRunning.value) {
      return;
    }
    enqueueSearchSources(currentSearchRun, activeSources.value);
    finishSearchIfIdle(currentSearchRun);
  },
);

async function handleForceReload() {
  if (reloadingSources.value || searchRunning.value) {
    return;
  }
  reloadingSources.value = true;
  try {
    await bookSourceStore.reloadSources();
    // 刷新当前限定书源引用
    if (navigationStore.searchInitSource) {
      if (!setSourceLimit(navigationStore.searchInitSource)) {
        clearSourceLimit();
      }
    } else if (limitedSource.value) {
      const currentKey = sourceKeyOf(limitedSource.value);
      const refreshed = bookSourceStore.searchableSources.find(
        (s) => sourceKeyOf(s) === currentKey,
      );
      limitedSource.value = refreshed ?? null;
    }
    if (hasSearched.value && searchKeyword.value.trim()) {
      await doSearch(searchPage.value);
    }
  } finally {
    reloadingSources.value = false;
  }
}

function goToSearchPage(page: number) {
  if (page < 1 || searchRunning.value) {
    return;
  }
  void doSearch(page);
}

const searchPaginationPages = computed(() => {
  const start = Math.max(1, searchPage.value - 3);
  const end = searchPage.value + 3;
  const pages: number[] = [];
  for (let i = start; i <= end; i++) {
    pages.push(i);
  }
  return pages;
});

// ── 书籍详情 ─────────────────────────────────────────────────────────────
const {
  showDrawer,
  drawerBookUrl,
  drawerFileName,
  drawerSourceDir,
  drawerFallbackBook,
  drawerSourceName,
  drawerSourceType,
  openDetail,
} = useBookDetailDrawerState({
  sources: sourcesRef,
});

// ── 章节阅读 ─────────────────────────────────────────────────────────────
const { getShelfId, ensureLoaded: ensureShelfLoaded, isPrivateShelfBook } = bookshelfStore;
const { privacyExitTick } = storeToRefs(privacyModeStore);
const {
  showReader,
  readerChapterUrl,
  readerChapterName,
  readerFileName,
  readerSourceDir,
  readerChapters,
  readerCurrentIndex,
  readerBookInfo,
  readerSourceType,
  readerShelfId,
  readerChapterGroups,
  readerActiveGroupIndex,
  applySourceSwitchToReader,
  onReadChapter,
} = useInlineBookReader({
  showDrawer,
  drawerBookUrl,
  drawerFileName,
  drawerSourceDir,
  privacyExitTick,
  runChapterList,
  cancelTask,
  ensureShelfLoaded,
  getShelfId,
  isPrivateShelfBook,
});

// ── 生命周期 ─────────────────────────────────────────────────────────────
const unlisteners: (() => void)[] = [];

onMounted(async () => {
  await loadSources();
  unlisteners.push(
    await eventListen<{ fileName: string; reason?: string }>("booksource:changed", (event) => {
      const { fileName: changed, reason } = event.payload ?? {};
      if (reason === "toggle") {
        return;
      }
      if (changed) {
        bookSourceStore.invalidateCapability(changed);
      } else {
        bookSourceStore.invalidateAllCapabilities();
      }
      bookSourceStore.markSourcesStale();
      void bookSourceStore.loadSources({ force: true });
    }),
  );
  unlisteners.push(
    await eventListen<{
      scope: string;
      fileName?: string;
      sourceDir?: string;
      refreshStarted?: boolean;
    }>("app:booksource-reload", (event) => {
      if (event.payload.scope === "all") {
        bookSourceStore.invalidateAllCapabilities();
      } else if (event.payload.scope === "single" && event.payload.fileName) {
        bookSourceStore.invalidateCapability(
          sourceEventKey(event.payload.fileName, event.payload.sourceDir),
        );
      }
      if (event.payload.refreshStarted) {
        void bookSourceStore.loadSources();
      } else {
        bookSourceStore.markSourcesStale();
        void bookSourceStore.loadSources({ force: true });
      }
    }),
  );
  unlisteners.push(
    await eventListen<{ view?: string }>("app:view-reload", async (event) => {
      if (event.payload?.view === "search") {
        await handleForceReload();
      }
    }),
  );
});

onUnmounted(() => {
  activeSearchToken.value += 1;
  cancelAggregatedGroupPublish();
  unlisteners.forEach((fn) => fn());
});
</script>

<template>
  <div class="search-view" :style="searchDensityStyle">
    <AppPageHeader title="搜索">
      <template #title-extra>
        <span class="sv-header__meta">
          {{ activeSources.length }} 个搜索源{{
            limitedSource && (!hasSearched || !hasSearchKeyword)
              ? `（仅限：${limitedSourceName}）`
              : ""
          }}
        </span>
      </template>
      <template #subtitle>
        <template v-if="!hasSearchKeyword && limitedSource">
          当前仅搜索：{{ limitedSourceName }}
        </template>
        <template v-else-if="!hasSearchKeyword">
          {{ searchStartDescription }}
        </template>
        <template v-else-if="aggregatedLoading">
          搜索中…&nbsp;已获得
          <span class="sv-header__count">{{ totalRawResultCount }}</span>
          条结果（{{ completedSourceCount }}/{{ activeSources.length }}
          书源完成）
        </template>
        <template v-else-if="totalRawResultCount === 0">
          未找到结果 · {{ activeSources.length }} 个书源已搜索
        </template>
        <template v-else>
          找到
          <span class="sv-header__count">{{ totalRawResultCount }}</span>
          条结果 · {{ sourcesWithResultCount }} / {{ activeSources.length }} 个书源有结果
        </template>
      </template>
      <template #actions>
        <MobileToolbarMenu :options="mobileMenuOptions" @select="handleMobileMenuSelect">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" quaternary @click="showCovers = !showCovers">
                <template #icon>
                  <Image v-if="showCovers" :size="14" />
                  <ImageOff v-else :size="14" />
                </template>
              </n-button>
            </template>
            {{ showCovers ? "隐藏封面图片" : "显示封面图片" }}
          </n-tooltip>

          <n-dropdown
            trigger="click"
            :options="CARD_SIZES.map((size) => ({ label: size.label, key: size.key }))"
            :value="activeSizeKey"
            @select="(key: string) => setSize(key as CardSizeKey)"
          >
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button size="small" quaternary>
                  <template #icon>
                    <LayoutGrid :size="14" />
                  </template>
                </n-button>
              </template>
              卡片大小（{{ activeSize.label }}）
            </n-tooltip>
          </n-dropdown>

          <!-- 视图模式切换 -->
          <n-button-group size="small" class="sv-header__mode">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button
                  :type="searchMode === 'aggregated' ? 'primary' : 'default'"
                  @click="searchMode = 'aggregated'"
                >
                  <template #icon>
                    <LayoutGrid :size="14" />
                  </template>
                </n-button>
              </template>
              聚合模式：按相似度排序，同名书堆叠显示
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button
                  :type="searchMode === 'grouped' ? 'primary' : 'default'"
                  @click="searchMode = 'grouped'"
                >
                  <template #icon>
                    <List :size="14" />
                  </template>
                </n-button>
              </template>
              分组模式：按书源分组展示
            </n-tooltip>
          </n-button-group>
          <n-button size="small" quaternary :loading="reloadingSources" @click="handleForceReload">
            强制刷新
          </n-button>
        </MobileToolbarMenu>
      </template>
    </AppPageHeader>

    <div class="sv-toolbar">
      <n-input
        v-model:value="searchKeyword"
        placeholder="输入书名或作者..."
        size="small"
        clearable
        class="sv-toolbar__input"
        @keydown.enter="doSearch()"
      >
        <template #prefix>
          <Search :size="13" />
        </template>
      </n-input>
      <n-select
        v-model:value="selectedSearchType"
        size="small"
        class="sv-toolbar__type"
        :options="sourceTypeOptions"
        :render-label="renderSourceTypeLabel"
      />
      <n-select
        v-model:value="selectedSourceValue"
        size="small"
        filterable
        class="sv-toolbar__scope"
        :options="searchScopeOptions"
      />
      <n-button v-if="searchRunning" type="warning" size="small" @click="stopSearch">
        停止搜索
      </n-button>
      <n-button v-else type="primary" size="small" @click="doSearch()"> 搜索 </n-button>

      <!-- 单书源限制标签 -->
      <n-tag
        v-if="limitedSource"
        closable
        size="small"
        type="info"
        class="sv-toolbar__limit-tag"
        @close="clearSourceLimit"
      >
        {{ limitedSourceName }}
      </n-tag>
    </div>

    <!-- 搜索结果 -->
    <div class="sv-content">
      <div class="sv-scroll app-scrollbar">
        <!-- 聚合模式 -->
        <template v-if="searchMode === 'aggregated'">
          <AggregatedSearchResults
            v-if="hasSearched"
            :keyword="searchKeyword"
            :groups="aggregatedGroups"
            :show-covers="showCovers"
            :loading="aggregatedLoading"
            :empty-description="aggregatedEmptyDescription"
            @select="openDetail"
          />
          <AppEmpty v-else :title="searchStartDescription" />
        </template>
        <!-- 分组模式 -->
        <template v-else>
          <template v-if="hasSearched">
            <SourceSearchGroup
              v-for="src in activeSources"
              :key="sourceKeyOf(src)"
              :source="src"
              :results="searchStates[sourceKeyOf(src)]?.results ?? []"
              :loading="searchStates[sourceKeyOf(src)]?.loading ?? false"
              :error="searchStates[sourceKeyOf(src)]?.error ?? ''"
              :show-covers="showCovers"
              @select="openDetail"
            />
          </template>
          <AppEmpty v-else :title="searchStartDescription" />
        </template>
      </div>

      <!-- 翻页栏 -->
      <div v-if="hasSearched && !searchRunning && totalRawResultCount > 0" class="sv-pagination">
        <button
          class="sv-page-btn"
          :disabled="searchPage === 1"
          @click="goToSearchPage(searchPage - 1)"
        >
          上一页
        </button>
        <button
          v-for="p in searchPaginationPages"
          :key="p"
          class="sv-page-btn"
          :class="{ 'sv-page-btn--active': p === searchPage }"
          @click="goToSearchPage(p)"
        >
          {{ p }}
        </button>
        <button class="sv-page-btn" @click="goToSearchPage(searchPage + 1)">下一页</button>
      </div>
    </div>

    <!-- 书籍详情抽屉 -->
    <BookDetailDrawer
      v-model:show="showDrawer"
      :book-url="drawerBookUrl"
      :file-name="drawerFileName"
      :source-dir="drawerSourceDir"
      :source-name="drawerSourceName"
      :source-type="drawerSourceType"
      :fallback-book="drawerFallbackBook"
      @read-chapter="onReadChapter"
    />

    <!-- 章节阅读器 -->
    <ChapterReaderModal
      v-model:show="showReader"
      v-model:current-index="readerCurrentIndex"
      :chapter-url="readerChapterUrl"
      :chapter-name="readerChapterName"
      :file-name="readerFileName"
      :source-dir="readerSourceDir"
      :chapters="readerChapters"
      :shelf-book-id="readerShelfId"
      :book-info="readerBookInfo"
      :source-type="readerSourceType"
      :chapter-groups="readerChapterGroups"
      :initial-group-index="readerActiveGroupIndex"
      @added-to-shelf="readerShelfId = $event"
      @source-switched="applySourceSwitchToReader"
    />
  </div>
</template>

<style scoped>
.search-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* background: var(--color-bg-page); */
}

.sv-header__sub {
  font-size: var(--fs-13);
  color: var(--color-text-soft);
}

.sv-header__meta {
  font-size: var(--fs-13);
  color: var(--color-text-soft);
  white-space: nowrap;
}

.sv-header__count {
  font-weight: var(--fw-bold);
  color: var(--color-accent);
}

.sv-header__actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.sv-header__mode {
  flex-shrink: 0;
}

.sv-toolbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-6) var(--space-2);
  flex-wrap: wrap;
}

.sv-toolbar__input {
  flex: 1;
  max-width: 400px;
}

.sv-toolbar__type {
  width: 140px;
  flex-shrink: 0;
}

.sv-toolbar__scope {
  width: 220px;
  flex-shrink: 0;
}

.sv-toolbar__limit-tag {
  flex-shrink: 0;
}

.sv-content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0 var(--space-6);
}

.sv-scroll {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-right: 4px;
  padding-bottom: calc(16px + var(--keyboard-inset-bottom, 0px));
}

/* ── 移动端适配 ─────────────────────────── */
@media (pointer: coarse), (max-width: 640px) {
  .sv-toolbar {
    padding: 0 var(--space-4) var(--space-2);
  }

  .sv-toolbar__input {
    max-width: 100%;
  }

  .sv-toolbar__scope {
    width: 100%;
  }

  .sv-toolbar__type {
    width: calc(40% - var(--space-1));
  }

  .sv-toolbar__scope {
    width: calc(60% - var(--space-1));
  }

  .sv-content {
    padding: 0 var(--space-4);
  }
}

.sv-pagination {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 10px 0 12px;
  flex-wrap: wrap;
}

.sv-page-btn {
  padding: 4px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: var(--color-text-soft);
  font-size: var(--fs-13);
  cursor: pointer;
  transition:
    border-color var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard);
  white-space: nowrap;
  min-width: 36px;
}
@media (hover: hover) and (pointer: fine) {
  .sv-page-btn:hover:not(:disabled) {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
}
.sv-page-btn--active {
  background: var(--color-accent);
  border-color: var(--color-accent);
  color: #fff;
  font-weight: var(--fw-semibold);
}
.sv-page-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
