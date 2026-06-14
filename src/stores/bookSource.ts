import { defineStore } from "pinia";
import { computed, ref, shallowReactive } from "vue";
import {
  applyBookSourceUpdate,
  checkBookSourceUpdate,
  evalBookSource,
  getBookSourceDir,
  getBookSourceDirs,
  listBookSources,
  listBookSourcesStreaming,
  toggleBookSource,
  type BookSourceMeta,
  type UpdateCheckResult,
} from "@/composables/useBookSource";
import { useCapabilities } from "@/composables/useCapabilities";
import { eventListenSync } from "@/composables/useEventBus";
import {
  ensureFrontendNamespaceLoaded,
  getFrontendStorageItem,
  legacyLocalStorageGet,
  legacyLocalStorageRemove,
  listFrontendStorageNamespaceSync,
  onFrontendStorageChange,
  removeFrontendStorageItem,
  setFrontendStorageItem,
  setFrontendStorageJson,
} from "@/composables/useFrontendStorage";
import { safeRandomUUID } from "@/utils/uuid";

// ── 书源能力存储键（继承自 useSourceCapabilities）────────────────────────
const STORAGE_NAMESPACE = "source.capabilities";
/** 能力缓存条目前缀：cap_{sourceKey} → 逗号分隔函数名列表 */
const CAP_KEY_PREFIX = "cap_";

// ── 书源更新检查 ─────────────────────────────────────────────────────────
const UPDATE_NS = "source.updates";
const LAST_CHECK_KEY = "lastCheckedAt";
/** 最短检查间隔：1 小时 */
const MIN_CHECK_INTERVAL_MS = 60 * 60 * 1000;
const SOURCE_LIST_CACHE_TTL_MS = 30 * 60 * 1000;
const LS_EXPLORE_KEY = "source-explore-disabled";
const LS_SEARCH_KEY = "source-search-disabled";
const EXPLORE_KEY = "exploreDisabled";
const SEARCH_KEY = "searchDisabled";

function parseDisabledSet(raw: string | null): Set<string> {
  if (!raw) {
    return new Set();
  }
  try {
    return new Set(JSON.parse(raw) as string[]);
  } catch {
    return new Set();
  }
}

function loadDisabledSet(key: string): Set<string> {
  const storageKey = key === LS_EXPLORE_KEY ? EXPLORE_KEY : SEARCH_KEY;
  const cached = listFrontendStorageNamespaceSync(STORAGE_NAMESPACE);
  return parseDisabledSet(cached[storageKey] ?? null);
}

function saveDisabledSet(key: string, set: Set<string>) {
  const storageKey = key === LS_EXPLORE_KEY ? EXPLORE_KEY : SEARCH_KEY;
  setFrontendStorageJson(STORAGE_NAMESPACE, storageKey, [...set]);
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  if (typeof error === "object" && error !== null) {
    const message = Reflect.get(error, "message");
    if (typeof message === "string") {
      return message;
    }
    const content = Reflect.get(error, "content");
    if (typeof content === "string") {
      return content;
    }
  }
  return String(error);
}

function isStreamingListUnsupported(error: unknown): boolean {
  const message = getErrorMessage(error);
  const normalized = message.toLowerCase();
  return (
    message.includes("booksource_list_streaming") &&
    (message.includes("未知命令") ||
      message.includes("未实现命令") ||
      normalized.includes("unknown command") ||
      normalized.includes("not implemented"))
  );
}

function getSourceCacheKey(source: BookSourceMeta): string {
  return source.sourceKey || `${source.sourceDir}::${source.fileName}`;
}

function isNovelSource(source: BookSourceMeta): boolean {
  const type = (source.sourceType || "novel").trim().toLowerCase();
  return type === "" || type === "novel" || type === "小说";
}

export const useBookSourceStore = defineStore("bookSource", () => {
  // ── 书源列表状态 ─────────────────────────────────────────────────────
  const sources = ref<BookSourceMeta[]>([]);
  const sourceDirs = ref<string[]>([]);
  const loading = ref(false);
  const capabilityDetecting = ref(false);
  /** 流式加载：已接收到的书源数 */
  const streamingLoaded = ref(0);
  let _loadInFlight: Promise<void> | null = null;
  let _detectAllInFlight: Promise<void> | null = null;
  let _sourcesLoadedOnce = false;
  let _sourcesLoadedAt = 0;
  let _sourcesDirty = true;
  /** 当前活跃的流式加载请求 ID，用于过滤过期事件 */
  let _streamRequestId = "";
  // ── 更新检查结果 ─────────────────────────────────────────────────────────
  /** 上次检查发现有可用更新的书源列表（含版本对比信息） */
  const pendingUpdates = ref<UpdateCheckResult[]>([]);
  let updatesCheckedAt = 0;
  // ── 能力检测缓存 ─────────────────────────────────────────────────────
  // shallowReactive：fnsCache[key]=val 只通知访问了该 key 的 computed，
  // 实现 buffer-append 语义——新书源逐条追加进发现列表，而非全量重渲染。
  const fnsCache = shallowReactive<Record<string, Set<string>>>({});

  // ── 用户开关（探索/搜索禁用集合）────────────────────────────────────
  const exploreDisabled = ref(loadDisabledSet(LS_EXPLORE_KEY));
  const searchDisabled = ref(loadDisabledSet(LS_SEARCH_KEY));

  // ── Getters ──────────────────────────────────────────────────────────
  const enabledSources = computed(() => sources.value.filter((s) => s.enabled));

  const explorableSources = computed(() =>
    enabledSources.value.filter(
      (s) =>
        isNovelSource(s) &&
        (s.hasExplore === true || fnsCache[getSourceCacheKey(s)]?.has("explore")) &&
        isExploreUserEnabled(getSourceCacheKey(s)),
    ),
  );

  const searchableSources = computed(() =>
    enabledSources.value.filter(
      (s) =>
        isNovelSource(s) &&
        fnsCache[getSourceCacheKey(s)]?.has("search") &&
        isSearchUserEnabled(getSourceCacheKey(s)),
    ),
  );

  const getSourceByFileName = computed(
    () => (fileName: string) => sources.value.find((s) => s.fileName === fileName),
  );

  // ── Actions ──────────────────────────────────────────────────────────

  function seedCapabilitiesFromMeta(source: BookSourceMeta): void {
    const capabilities = source.capabilities?.filter(Boolean) ?? [];
    if (!capabilities.length) {
      return;
    }
    fnsCache[getSourceCacheKey(source)] = new Set(capabilities);
  }

  /** 将新扫描到的批次立即合并进 sources.value，不清空数组以保持滚动位置。 */
  function mergeSourcesBatch(next: BookSourceMeta[]): void {
    if (!next.length) {
      return;
    }
    const currentByKey = new Map(
      sources.value.map((source) => [getSourceCacheKey(source), source]),
    );
    for (const item of next) {
      seedCapabilitiesFromMeta(item);
      const key = getSourceCacheKey(item);
      const current = currentByKey.get(key);
      if (current) {
        Object.assign(current, item);
      } else {
        sources.value.push(item);
      }
    }
    sources.value.sort((a, b) => a.name.localeCompare(b.name));
  }

  function pruneSourcesNotSeen(seenKeys: Set<string>): void {
    for (let i = sources.value.length - 1; i >= 0; i--) {
      if (!seenKeys.has(getSourceCacheKey(sources.value[i]))) {
        sources.value.splice(i, 1);
      }
    }
  }

  /** 统一书源加载（去重保护，三个视图共用此接口），流式分批推送到 sources */
  async function loadSources(options: { force?: boolean } = {}): Promise<void> {
    const force = options.force === true;
    const cacheFresh =
      _sourcesLoadedOnce &&
      !_sourcesDirty &&
      Date.now() - _sourcesLoadedAt < SOURCE_LIST_CACHE_TTL_MS;
    if (!force && cacheFresh) {
      return;
    }
    if (_loadInFlight) {
      return _loadInFlight;
    }
    _loadInFlight = (async () => {
      loading.value = true;
      streamingLoaded.value = 0;
      const freshKeys = new Set<string>();

      try {
        const requestId = safeRandomUUID();
        _streamRequestId = requestId;

        // 先并发获取目录信息
        const [dir, dirs] = await Promise.all([getBookSourceDir(), getBookSourceDirs()]);
        sourceDirs.value = [dir, ...dirs.filter((d) => d !== dir)];

        // 设置流式事件监听（必须在调用命令前注册，避免错过首批）
        await new Promise<void>((resolve, reject) => {
          const unlisten = eventListenSync<{
            requestId: string;
            items: BookSourceMeta[];
            done: boolean;
            total?: number;
            error?: string;
          }>("booksource:batch", (event) => {
            const { requestId: id, items, done, error } = event.payload;

            if (id !== requestId) {
              return;
            }

            // 当前监听器所属请求已过期：只清理自己，不影响新请求。
            if (id !== _streamRequestId) {
              if (done) {
                unlisten();
              }
              return;
            }

            if (items.length > 0) {
              for (const item of items) {
                freshKeys.add(getSourceCacheKey(item));
              }
              mergeSourcesBatch(items);
              streamingLoaded.value += items.length;
            }

            if (done) {
              pruneSourcesNotSeen(freshKeys);
              unlisten();
              if (typeof error === "string" && error.length > 0) {
                reject(new Error(error));
              } else {
                resolve();
              }
            }
          });

          // 启动后端流式扫描（立即返回）
          listBookSourcesStreaming(requestId, force).catch((err: unknown) => {
            if (isStreamingListUnsupported(err)) {
              unlisten();
              listBookSources()
                .then((items) => {
                  const seenKeys = new Set(items.map(getSourceCacheKey));
                  mergeSourcesBatch(items);
                  pruneSourcesNotSeen(seenKeys);
                  streamingLoaded.value = sources.value.length;
                  resolve();
                })
                .catch((fallbackErr: unknown) => {
                  reject(
                    fallbackErr instanceof Error ? fallbackErr : new Error(String(fallbackErr)),
                  );
                });
              return;
            }
            unlisten();
            reject(err instanceof Error ? err : new Error(String(err)));
          });
        });

        // 加载完书源列表后自动触发能力检测与更新检查（非阻塞，后台跑）
        _sourcesLoadedOnce = true;
        _sourcesLoadedAt = Date.now();
        _sourcesDirty = false;

        void detectAllCapabilities();
        void ensureFrontendNamespaceLoaded(UPDATE_NS).then(() => checkUpdatesIfStale());
      } finally {
        loading.value = false;
        _loadInFlight = null;
      }
    })();
    return _loadInFlight;
  }

  /** 强制刷新书源列表 */
  async function reloadSources(): Promise<void> {
    _sourcesDirty = true;
    _sourcesLoadedAt = 0;
    _loadInFlight = null;
    await loadSources({ force: true });
  }

  function markSourcesStale(): void {
    _sourcesDirty = true;
    _sourcesLoadedAt = 0;
  }

  /**
   * 从持久化存储中预加载所有已缓存的能力记录到内存 fnsCache。
   * 在首次 detectAllCapabilities 之前调用可大幅缩短能力扫描时间。
   */
  async function ensureCapsLoaded(): Promise<void> {
    await ensureFrontendNamespaceLoaded(STORAGE_NAMESPACE);
    const stored = listFrontendStorageNamespaceSync(STORAGE_NAMESPACE);
    for (const [key, val] of Object.entries(stored)) {
      if (!key.startsWith(CAP_KEY_PREFIX)) {
        continue;
      }
      const sourceKey = key.slice(CAP_KEY_PREFIX.length);
      if (!fnsCache[sourceKey]) {
        // 逐条写入 shallowReactive，每条只触发读了该 key 的 computed
        fnsCache[sourceKey] = new Set(val ? val.split(",").filter(Boolean) : []);
      }
    }
  }

  function resolveSourceRef(
    sourceOrKey: BookSourceMeta | string,
    sourceDir?: string,
  ): { fileName: string; sourceDir?: string; cacheKey: string } {
    if (typeof sourceOrKey !== "string") {
      return {
        fileName: sourceOrKey.fileName,
        sourceDir: sourceOrKey.sourceDir,
        cacheKey: getSourceCacheKey(sourceOrKey),
      };
    }
    const source = sources.value.find(
      (item) =>
        getSourceCacheKey(item) === sourceOrKey ||
        (item.fileName === sourceOrKey &&
          (sourceDir === undefined || item.sourceDir === sourceDir)),
    );
    if (source) {
      return {
        fileName: source.fileName,
        sourceDir: source.sourceDir,
        cacheKey: getSourceCacheKey(source),
      };
    }
    return { fileName: sourceOrKey, cacheKey: sourceOrKey };
  }

  /** 检测单个书源的函数能力（带缓存，结果持久化到存储） */
  async function detectCapabilities(
    sourceOrKey: BookSourceMeta | string,
    sourceDir?: string,
  ): Promise<Set<string>> {
    const sourceRef = resolveSourceRef(sourceOrKey, sourceDir);
    if (fnsCache[sourceRef.cacheKey]) {
      return fnsCache[sourceRef.cacheKey];
    }
    try {
      const raw = await evalBookSource(sourceRef.fileName, undefined, sourceRef.sourceDir);
      const fns = new Set(
        (raw ?? "")
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
      );
      // shallowReactive 直接赋值，只触发读了该 sourceKey 的 computed
      fnsCache[sourceRef.cacheKey] = fns;
      const newVal = [...fns].join(",");
      const storedVal = getFrontendStorageItem(
        STORAGE_NAMESPACE,
        CAP_KEY_PREFIX + sourceRef.cacheKey,
      );
      if (storedVal !== newVal) {
        setFrontendStorageItem(STORAGE_NAMESPACE, CAP_KEY_PREFIX + sourceRef.cacheKey, newVal);
      }
      return fns;
    } catch {
      const empty = new Set<string>();
      fnsCache[sourceRef.cacheKey] = empty;
      return empty;
    }
  }

  /** 批量检测所有启用书源的能力（5 并发，重复调用复用同一 Promise）
   * fnsCache 是 shallowReactive：每次 fnsCache[sourceKey]=fns 只通知
   * 读了该 key 的 computed，实现逐条追加（buffer-append）而非整页刷新。
   */
  function detectAllCapabilities(): Promise<void> {
    if (_detectAllInFlight) {
      return _detectAllInFlight;
    }
    const enabled = sources.value.filter((s) => s.enabled && isNovelSource(s));
    const pending = enabled.filter((src) => !fnsCache[getSourceCacheKey(src)]);
    if (pending.length === 0) {
      return Promise.resolve();
    }
    _detectAllInFlight = (async () => {
      capabilityDetecting.value = true;
      const CONCURRENCY = 5;
      try {
        for (let i = 0; i < pending.length; i += CONCURRENCY) {
          await Promise.all(
            pending.slice(i, i + CONCURRENCY).map(async (src) => {
              // 并发的单片检测已完成则复用（不重复 eval）
              const cacheKey = getSourceCacheKey(src);
              if (fnsCache[cacheKey]) return;
              try {
                const raw = await evalBookSource(src.fileName, undefined, src.sourceDir);
                const fns = new Set(
                  (raw ?? "")
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean),
                );
                const newVal = [...fns].join(",");
                const storedVal = getFrontendStorageItem(
                  STORAGE_NAMESPACE,
                  CAP_KEY_PREFIX + cacheKey,
                );
                if (storedVal !== newVal) {
                  setFrontendStorageItem(STORAGE_NAMESPACE, CAP_KEY_PREFIX + cacheKey, newVal);
                }
                // 直接写入 shallowReactive：只通知依赖了 sourceKey 这条 key
                // 的 computed（如 explorableSources / searchableSources），
                // 该书源对应的 tab 单独追加，其他 tab 不受影响。
                fnsCache[cacheKey] = fns;
              } catch {
                fnsCache[cacheKey] = new Set<string>();
              }
            }),
          );
        }
      } finally {
        capabilityDetecting.value = false;
      }
    })();
    void _detectAllInFlight.finally(() => {
      _detectAllInFlight = null;
    });
    return _detectAllInFlight;
  }

  /** 使单个书源的能力缓存失效（同时删除持久化条目） */
  function invalidateCapability(sourceKeyOrFileName: string) {
    const matchingKeys = sources.value
      .filter(
        (source) =>
          getSourceCacheKey(source) === sourceKeyOrFileName ||
          source.fileName === sourceKeyOrFileName,
      )
      .map(getSourceCacheKey);
    const keys = matchingKeys.length ? matchingKeys : [sourceKeyOrFileName];
    for (const key of keys) {
      // eslint-disable-next-line @typescript-eslint/no-dynamic-delete
      delete fnsCache[key];
      removeFrontendStorageItem(STORAGE_NAMESPACE, CAP_KEY_PREFIX + key);
    }
  }

  /** 清空全部能力缓存（同时删除所有持久化条目） */
  function invalidateAllCapabilities() {
    const stored = listFrontendStorageNamespaceSync(STORAGE_NAMESPACE);
    for (const key of Object.keys(stored)) {
      if (key.startsWith(CAP_KEY_PREFIX)) {
        removeFrontendStorageItem(STORAGE_NAMESPACE, key);
      }
    }
    for (const key of Object.keys(fnsCache)) {
      // eslint-disable-next-line @typescript-eslint/no-dynamic-delete
      delete fnsCache[key];
    }
  }

  /** 获取书源已缓存的能力集合（未检测则返回 undefined） */
  function getCachedCapabilities(
    sourceOrKey: BookSourceMeta | string,
    sourceDir?: string,
  ): Set<string> | undefined {
    const sourceRef = resolveSourceRef(sourceOrKey, sourceDir);
    return fnsCache[sourceRef.cacheKey] ?? fnsCache[sourceRef.fileName];
  }

  /** 切换书源启用/禁用状态（包含 API 调用 + 本地状态同步）*/
  async function toggleSource(
    fileName: string,
    enabled: boolean,
    sourceDir?: string,
  ): Promise<void> {
    await toggleBookSource(fileName, enabled, sourceDir);
    const src = sources.value.find(
      (s) => s.fileName === fileName && (sourceDir === undefined || s.sourceDir === sourceDir),
    );
    if (src) {
      src.enabled = enabled;
    }
  }

  // ── 用户开关（探索/搜索）────────────────────────────────────────────

  function isExploreUserEnabled(sourceKeyOrFileName: string): boolean {
    const sourceRef = resolveSourceRef(sourceKeyOrFileName);
    return (
      !exploreDisabled.value.has(sourceRef.cacheKey) &&
      !exploreDisabled.value.has(sourceRef.fileName)
    );
  }

  function setExploreUserEnabled(sourceKeyOrFileName: string, enabled: boolean) {
    const sourceRef = resolveSourceRef(sourceKeyOrFileName);
    const newSet = new Set(exploreDisabled.value);
    if (enabled) {
      newSet.delete(sourceRef.cacheKey);
      newSet.delete(sourceRef.fileName);
    } else {
      newSet.add(sourceRef.cacheKey);
    }
    exploreDisabled.value = newSet;
    saveDisabledSet(LS_EXPLORE_KEY, newSet);
  }

  function isSearchUserEnabled(sourceKeyOrFileName: string): boolean {
    const sourceRef = resolveSourceRef(sourceKeyOrFileName);
    return (
      !searchDisabled.value.has(sourceRef.cacheKey) && !searchDisabled.value.has(sourceRef.fileName)
    );
  }

  function setSearchUserEnabled(sourceKeyOrFileName: string, enabled: boolean) {
    const sourceRef = resolveSourceRef(sourceKeyOrFileName);
    const newSet = new Set(searchDisabled.value);
    if (enabled) {
      newSet.delete(sourceRef.cacheKey);
      newSet.delete(sourceRef.fileName);
    } else {
      newSet.add(sourceRef.cacheKey);
    }
    searchDisabled.value = newSet;
    saveDisabledSet(LS_SEARCH_KEY, newSet);
  }

  // ── 书源更新周期检查 ─────────────────────────────────────────────────

  /**
   * 检查所有拥有 updateUrl 的启用书源是否有新版本。
   * 若距上次检查不足 1 小时，跳过以避免高频请求。
   * 结果写入 `pendingUpdates`（仅含有更新的条目）。
   */
  async function checkUpdatesIfStale(): Promise<void> {
    const capabilities = await useCapabilities().loadCapabilities();
    if (!capabilities.repository.supported) {
      pendingUpdates.value = [];
      return;
    }

    const now = Date.now();
    const lastCheckedRaw = getFrontendStorageItem(UPDATE_NS, LAST_CHECK_KEY);
    const lastChecked = lastCheckedRaw ? parseInt(lastCheckedRaw, 10) : 0;
    const effectiveLastChecked = Math.max(
      updatesCheckedAt,
      Number.isNaN(lastChecked) ? 0 : lastChecked,
    );
    if (pendingUpdates.value.length > 0 && now - effectiveLastChecked < MIN_CHECK_INTERVAL_MS) {
      return;
    }

    const targets = sources.value.filter((s) => s.enabled && s.updateUrl);
    if (targets.length === 0) {
      return;
    }

    // 记录本次检查时间戳（先写，避免并发重入）
    updatesCheckedAt = now;
    setFrontendStorageItem(UPDATE_NS, LAST_CHECK_KEY, String(now));

    const results: UpdateCheckResult[] = [];
    const CONCURRENCY = 3;
    for (let i = 0; i < targets.length; i += CONCURRENCY) {
      const batch = targets.slice(i, i + CONCURRENCY);
      const batchResults = await Promise.allSettled(
        batch.map((src) => checkBookSourceUpdate(src.fileName)),
      );
      for (const r of batchResults) {
        if (r.status === "fulfilled" && r.value.hasUpdate) {
          results.push(r.value);
        }
      }
    }
    pendingUpdates.value = results;
  }

  function getPendingUpdate(uuid: string): UpdateCheckResult | undefined {
    return pendingUpdates.value.find((item) => item.uuid === uuid);
  }

  async function applyUpdate(fileName: string): Promise<void> {
    await applyBookSourceUpdate(fileName);
    const source = sources.value.find((item) => item.fileName === fileName);
    if (source) {
      pendingUpdates.value = pendingUpdates.value.filter((item) => item.uuid !== source.uuid);
    } else {
      pendingUpdates.value = pendingUpdates.value.filter((item) => item.fileName !== fileName);
    }
    invalidateCapability(fileName);
  }

  // ── 初始化（迁移旧数据 + 监听存储变更）──────────────────────────────
  function initialize() {
    void ensureFrontendNamespaceLoaded(STORAGE_NAMESPACE, () => {
      const migrated: Record<string, string> = {};
      const exploreLegacy = legacyLocalStorageGet(LS_EXPLORE_KEY);
      const searchLegacy = legacyLocalStorageGet(LS_SEARCH_KEY);
      if (exploreLegacy) {
        migrated[EXPLORE_KEY] = exploreLegacy;
        legacyLocalStorageRemove(LS_EXPLORE_KEY);
      }
      if (searchLegacy) {
        migrated[SEARCH_KEY] = searchLegacy;
        legacyLocalStorageRemove(LS_SEARCH_KEY);
      }
      return Object.keys(migrated).length ? migrated : null;
    }).then(() => {
      exploreDisabled.value = loadDisabledSet(LS_EXPLORE_KEY);
      searchDisabled.value = loadDisabledSet(LS_SEARCH_KEY);
    });

    onFrontendStorageChange(({ namespace }) => {
      if (namespace !== STORAGE_NAMESPACE) {
        return;
      }
      exploreDisabled.value = loadDisabledSet(LS_EXPLORE_KEY);
      searchDisabled.value = loadDisabledSet(LS_SEARCH_KEY);
    });
  }

  return {
    // state
    sources,
    sourceDirs,
    loading,
    capabilityDetecting,
    streamingLoaded,
    fnsCache,
    exploreDisabled,
    searchDisabled,
    pendingUpdates,
    // getters
    enabledSources,
    explorableSources,
    searchableSources,
    getSourceByFileName,
    // actions
    loadSources,
    reloadSources,
    markSourcesStale,
    ensureCapsLoaded,
    detectCapabilities,
    detectAllCapabilities,
    invalidateCapability,
    invalidateAllCapabilities,
    getCachedCapabilities,
    toggleSource,
    isExploreUserEnabled,
    setExploreUserEnabled,
    isSearchUserEnabled,
    setSearchUserEnabled,
    checkUpdatesIfStale,
    getPendingUpdate,
    applyUpdate,
    initialize,
  };
});
