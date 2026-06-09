<script setup lang="ts">
import type { SelectOption } from "naive-ui";
import {
  ArrowDown,
  ArrowUp,
  Check,
  GripVertical,
  Plus,
  RefreshCw,
  Settings2,
  Sparkles,
  Trash2,
} from "lucide-vue-next";
import { useMessage } from "naive-ui";
import { computed, onMounted, reactive, ref, watch } from "vue";
import type { BookItem } from "@/stores";
import type { BookSourceMeta } from "@/types";
import AppDrawer from "@/components/base/AppDrawer.vue";
import SourceTypeBadge from "@/components/base/SourceTypeBadge.vue";
import BookCoverImg from "@/components/BookCoverImg.vue";
import { useDynamicConfig } from "@/composables/useDynamicConfig";
import { isHtmlExploreResult, isUrlExploreResult } from "@/composables/useExploreBridge";
import {
  type ExploreCategoryItem,
  getCachedExploreBooks,
  getCachedExploreCategories,
  normalizeExploreCategories,
  preloadExploreBooksCache,
  preloadExploreCategoryCache,
  setCachedExploreBooks,
  setCachedExploreCategories,
} from "@/composables/useExploreCategoryCache";
import { useBookSourceStore, useScriptBridgeStore } from "@/stores";

interface RecommendationConfig {
  enabled: boolean;
  fileName: string;
  sourceDir?: string;
  category: string;
  title: string;
  limit: number;
  layout: RecommendationLayout;
}

interface RecommendationModule extends RecommendationConfig {
  id: string;
}

interface ShelfComponentConfig {
  showTitle: boolean;
  title: string;
}

interface RecommendationState extends Partial<RecommendationConfig> {
  modules?: RecommendationModule[];
  order?: string[];
  shelf?: Partial<ShelfComponentConfig>;
}

interface BookshelfHomeConfig {
  modules: RecommendationModule[];
  order: string[];
  shelf: ShelfComponentConfig;
}

type RecommendationLayout =
  | "carousel"
  | "ranking"
  | "rankingGrid"
  | "grid"
  | "featured"
  | "infiniteGrid"
  | "buttonGroup"
  | "waterfall";

interface LayoutOption {
  key: RecommendationLayout;
  label: string;
  hint: string;
}

interface RecommendationRenderItem {
  id: string;
  order: number;
  module: RecommendationModule;
}

type SettingsItem =
  | { type: "shelf"; id: string; order: number }
  | {
      type: "recommendation";
      id: string;
      order: number;
      module: RecommendationModule;
    };

const SHELF_COMPONENT_ID = "bookshelf";
const LEGACY_MODULE_ID = "legacy-discovery-recommendation";

const DEFAULT_CONFIG: RecommendationConfig = {
  enabled: false,
  fileName: "",
  sourceDir: "",
  category: "",
  title: "发现推荐",
  limit: 8,
  layout: "carousel",
};

const DEFAULT_SHELF_CONFIG: ShelfComponentConfig = {
  showTitle: true,
  title: "我的书架",
};

const LAYOUT_OPTIONS: LayoutOption[] = [
  { key: "carousel", label: "横滑轮播", hint: "一行横滑封面" },
  { key: "ranking", label: "排行榜", hint: "纵向排行列表" },
  { key: "rankingGrid", label: "网格排行榜", hint: "双列排行" },
  { key: "grid", label: "网格", hint: "整齐封面墙" },
  { key: "featured", label: "推荐卡片", hint: "首本大卡突出" },
  { key: "infiniteGrid", label: "无限网格", hint: "更多内容展开" },
  { key: "buttonGroup", label: "按钮组", hint: "轻量文字入口" },
  { key: "waterfall", label: "错位瀑布流", hint: "错落封面排布" },
];

const layoutKeySet = new Set<RecommendationLayout>(LAYOUT_OPTIONS.map((option) => option.key));

const emit = defineEmits<{
  (e: "select", book: BookItem, fileName: string, sourceDir?: string): void;
  (e: "open-book", bookUrl: string, fileName: string, sourceDir?: string): void;
  (e: "shelf-order-change", order: number): void;
  (e: "shelf-config-change", config: ShelfComponentConfig): void;
}>();

const message = useMessage();
const bookSourceStore = useBookSourceStore();
const scriptBridgeStore = useScriptBridgeStore();
const settingsDrawerRef = ref<InstanceType<typeof AppDrawer> | null>(null);

const configStore = useDynamicConfig<RecommendationState>({
  namespace: "ui.bookshelf.discoveryRecommendation",
  version: 1,
  defaults: () => ({
    modules: [],
    order: [SHELF_COMPONENT_ID],
    shelf: { ...DEFAULT_SHELF_CONFIG },
  }),
  migrate: () => null,
  legacyKeys: [],
});

const configReady = ref(false);
const sourceLoading = ref(false);
let storageReadyPromise: Promise<void> | null = null;
let sourceSystemPromise: Promise<void> | null = null;
let moduleIdSeed = 0;

const showSettings = ref(false);
const editingModuleId = ref<string | null>(null);
const draft = reactive<RecommendationConfig>({ ...DEFAULT_CONFIG });
const draftCategories = ref<ExploreCategoryItem[]>([]);
const draftCategoriesLoading = ref(false);
const draftCategoriesError = ref("");
let categoryRequestToken = 0;

function flattenExploreCategories(
  categories: ExploreCategoryItem[],
  parentName = "",
): ExploreCategoryItem[] {
  const out: ExploreCategoryItem[] = [];
  for (const category of categories) {
    out.push({
      ...category,
      children: undefined,
      name: parentName ? `${parentName} / ${category.name}` : category.name,
    });
    const children = category.children ?? [];
    if (children.length) {
      out.push(
        ...flattenExploreCategories(
          children,
          parentName ? `${parentName} / ${category.name}` : category.name,
        ),
      );
    }
  }
  return out;
}

const draftSelectableCategories = computed(() => flattenExploreCategories(draftCategories.value));

const moduleBooks = reactive<Record<string, BookItem[]>>({});
const moduleLoading = reactive<Record<string, boolean>>({});
const moduleRefreshing = reactive<Record<string, boolean>>({});
const moduleErrors = reactive<Record<string, string>>({});
const moduleRequestTokens = new Map<string, number>();

function isStandardExploreSource(source: BookSourceMeta): boolean {
  return source.sourceType !== "webpage" && source.sourceType !== "web";
}

const selectableSources = computed(() =>
  bookSourceStore.explorableSources.filter(isStandardExploreSource),
);

function sourceKeyOf(source: BookSourceMeta): string {
  return source.sourceKey || `${source.sourceDir}::${source.fileName}`;
}

const sourceOptions = computed<SelectOption[]>(() =>
  selectableSources.value.map((source) => ({
    label: source.name,
    value: sourceKeyOf(source),
  })),
);

const homeConfig = computed(() => normalizeHomeConfigState(configStore.state));
const shelfConfig = computed(() => homeConfig.value.shelf);
const configuredModules = computed(() => homeConfig.value.modules);
const activeModules = computed(() =>
  configuredModules.value.filter((module) => module.enabled && !!module.fileName),
);
const activeModuleConfigKey = computed(() =>
  activeModules.value
    .map(
      (module) =>
        `${module.id}|${module.enabled}|${module.fileName}|${module.sourceDir ?? ""}|${module.category}|${module.limit}|${module.layout}`,
    )
    .join("\n"),
);
const visibleRecommendationItems = computed<RecommendationRenderItem[]>(() => {
  const moduleMap = new Map(configuredModules.value.map((module) => [module.id, module]));
  return homeConfig.value.order
    .map((id, order): RecommendationRenderItem | null => {
      if (id === SHELF_COMPONENT_ID) {
        return null;
      }
      const module = moduleMap.get(id);
      if (!module?.enabled || !module.fileName) {
        return null;
      }
      return { id, order, module };
    })
    .filter((item): item is RecommendationRenderItem => item !== null);
});
const settingsItems = computed<SettingsItem[]>(() => {
  const moduleMap = new Map(configuredModules.value.map((module) => [module.id, module]));
  return homeConfig.value.order
    .map((id, order): SettingsItem | null => {
      if (id === SHELF_COMPONENT_ID) {
        return { type: "shelf", id, order };
      }
      const module = moduleMap.get(id);
      return module ? { type: "recommendation", id, order, module } : null;
    })
    .filter((item): item is SettingsItem => item !== null);
});
const shelfOrder = computed(() => {
  const index = homeConfig.value.order.indexOf(SHELF_COMPONENT_ID);
  return index >= 0 ? index : 0;
});
const draftSource = computed(() => getSourceByFileName(draft.fileName, draft.sourceDir));
const draftSourceKey = computed(() => (draftSource.value ? sourceKeyOf(draftSource.value) : ""));
const editingModule = computed(() =>
  editingModuleId.value
    ? configuredModules.value.find((module) => module.id === editingModuleId.value)
    : null,
);

function normalizeLayout(raw: unknown): RecommendationLayout {
  return typeof raw === "string" && layoutKeySet.has(raw as RecommendationLayout)
    ? (raw as RecommendationLayout)
    : DEFAULT_CONFIG.layout;
}

function normalizeLimit(value: unknown): number {
  const numeric = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(numeric)) {
    return DEFAULT_CONFIG.limit;
  }
  return Math.min(30, Math.max(4, Math.floor(numeric)));
}

function normalizeText(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function normalizeShelfConfig(raw: unknown): ShelfComponentConfig {
  const record = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const title = normalizeText(record.title, DEFAULT_SHELF_CONFIG.title).trim();
  return {
    showTitle:
      typeof record.showTitle === "boolean" ? record.showTitle : DEFAULT_SHELF_CONFIG.showTitle,
    title: title || DEFAULT_SHELF_CONFIG.title,
  };
}

function normalizeModule(raw: unknown, fallbackId: string): RecommendationModule | null {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const record = raw as Record<string, unknown>;
  const fileName = normalizeText(record.fileName);
  return {
    id: normalizeText(record.id, fallbackId) || fallbackId,
    enabled: typeof record.enabled === "boolean" ? record.enabled : !!fileName,
    fileName,
    sourceDir: normalizeText(record.sourceDir),
    category: normalizeText(record.category),
    title: normalizeText(record.title, DEFAULT_CONFIG.title) || DEFAULT_CONFIG.title,
    limit: normalizeLimit(record.limit),
    layout: normalizeLayout(record.layout),
  };
}

function normalizeHomeConfigState(state: RecommendationState): BookshelfHomeConfig {
  const modules: RecommendationModule[] = [];
  const usedIds = new Set<string>();
  const rawModules = Array.isArray(state.modules) ? state.modules : null;

  if (rawModules) {
    rawModules.forEach((rawModule, index) => {
      const module = normalizeModule(rawModule, `recommendation-${index + 1}`);
      if (!module) {
        return;
      }
      let id = module.id;
      let suffix = 2;
      while (usedIds.has(id)) {
        id = `${module.id}-${suffix}`;
        suffix += 1;
      }
      usedIds.add(id);
      modules.push({ ...module, id });
    });
  } else if (state.fileName || typeof state.enabled === "boolean") {
    const legacyModule = normalizeModule(
      {
        id: LEGACY_MODULE_ID,
        enabled: state.enabled || !!state.fileName,
        fileName: state.fileName,
        sourceDir: state.sourceDir,
        category: state.category,
        title: state.title,
        limit: state.limit,
        layout: state.layout,
      },
      LEGACY_MODULE_ID,
    );
    if (legacyModule?.fileName || legacyModule?.enabled) {
      modules.push(legacyModule);
      usedIds.add(legacyModule.id);
    }
  }

  const moduleIds = new Set(modules.map((module) => module.id));
  const order = Array.isArray(state.order)
    ? state.order.filter((id) => id === SHELF_COMPONENT_ID || moduleIds.has(id))
    : [];

  if (!order.includes(SHELF_COMPONENT_ID)) {
    order.push(SHELF_COMPONENT_ID);
  }

  for (const module of modules) {
    if (order.includes(module.id)) {
      continue;
    }
    const shelfIndex = order.indexOf(SHELF_COMPONENT_ID);
    if (shelfIndex >= 0) {
      order.splice(shelfIndex, 0, module.id);
    } else {
      order.push(module.id);
    }
  }

  return { modules, order, shelf: normalizeShelfConfig(state.shelf) };
}

function toPersistableConfig(config: BookshelfHomeConfig): RecommendationState {
  return {
    modules: config.modules.map((module) => ({ ...module })),
    order: [...config.order],
    shelf: { ...config.shelf },
  };
}

function createModuleId(): string {
  moduleIdSeed += 1;
  return `recommendation-${Date.now().toString(36)}-${moduleIdSeed}`;
}

function getSourceByFileName(fileName: string, sourceDir?: string): BookSourceMeta | undefined {
  const matchesSource = (source: BookSourceMeta) =>
    source.fileName === fileName && (!sourceDir || source.sourceDir === sourceDir);
  return (
    selectableSources.value.find(matchesSource) ??
    bookSourceStore.sources.find(matchesSource) ??
    selectableSources.value.find((source) => source.fileName === fileName) ??
    bookSourceStore.sources.find((source) => source.fileName === fileName)
  );
}

function getModuleSource(module: RecommendationModule): BookSourceMeta | undefined {
  return getSourceByFileName(module.fileName, module.sourceDir);
}

function getModuleSourceDir(module: RecommendationModule): string | undefined {
  return module.sourceDir || getModuleSource(module)?.sourceDir;
}

function getModuleSourceKey(module: RecommendationModule): string {
  const source = getModuleSource(module);
  if (source) {
    return sourceKeyOf(source);
  }
  return module.sourceDir ? `${module.sourceDir}::${module.fileName}` : module.fileName;
}

async function persistHomeConfig(config: BookshelfHomeConfig): Promise<void> {
  await configStore.replace(toPersistableConfig(config));
}

async function persistNormalizedConfigIfNeeded(): Promise<void> {
  const normalized = toPersistableConfig(homeConfig.value);
  const current = {
    modules: Array.isArray(configStore.state.modules) ? configStore.state.modules : [],
    order: Array.isArray(configStore.state.order) ? configStore.state.order : [],
    shelf: normalizeShelfConfig(configStore.state.shelf),
  };
  if (JSON.stringify(normalized) !== JSON.stringify(current)) {
    await configStore.replace(normalized);
  }
}

async function ensureRecommendationStorage(): Promise<void> {
  if (storageReadyPromise) {
    return storageReadyPromise;
  }
  storageReadyPromise = Promise.all([
    configStore.ready,
    preloadExploreCategoryCache(),
    preloadExploreBooksCache(),
  ]).then(() => {
    configReady.value = true;
  });
  return storageReadyPromise;
}

async function ensureSourceSystem(): Promise<void> {
  if (sourceSystemPromise) {
    return sourceSystemPromise;
  }
  sourceSystemPromise = (async () => {
    sourceLoading.value = true;
    try {
      await ensureRecommendationStorage();
      await bookSourceStore.ensureCapsLoaded();
      await bookSourceStore.loadSources();
      await bookSourceStore.detectAllCapabilities();
    } finally {
      sourceLoading.value = false;
      sourceSystemPromise = null;
    }
  })();
  return sourceSystemPromise;
}

function normalizeBooksResult(raw: unknown): BookItem[] | null {
  if (isUrlExploreResult(raw) || isHtmlExploreResult(raw)) {
    return null;
  }
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.filter((item): item is BookItem => {
    if (!item || typeof item !== "object") {
      return false;
    }
    const bookUrl = Reflect.get(item, "bookUrl");
    return typeof bookUrl === "string" && bookUrl.length > 0;
  });
}

function nextRequestToken(moduleId: string): number {
  const token = (moduleRequestTokens.get(moduleId) ?? 0) + 1;
  moduleRequestTokens.set(moduleId, token);
  return token;
}

function isCurrentRequest(moduleId: string, requestToken: number): boolean {
  return moduleRequestTokens.get(moduleId) === requestToken;
}

async function fetchBooks(module: RecommendationModule, requestToken: number, noCache: boolean) {
  const raw = await scriptBridgeStore.runExplore(
    module.fileName,
    module.category,
    1,
    noCache,
    getModuleSourceDir(module),
  );
  if (!isCurrentRequest(module.id, requestToken)) {
    return;
  }
  const nextBooks = normalizeBooksResult(raw);
  if (nextBooks === null) {
    moduleBooks[module.id] = [];
    moduleErrors[module.id] = "该分类返回网页内容，暂不能作为首页推荐";
    return;
  }
  moduleBooks[module.id] = nextBooks;
  moduleErrors[module.id] = "";
  setCachedExploreBooks(getModuleSourceKey(module), module.category, nextBooks);
}

async function loadRecommendation(module: RecommendationModule, force = false) {
  if (!module.enabled || !module.fileName) {
    moduleBooks[module.id] = [];
    moduleErrors[module.id] = "";
    return;
  }

  const requestToken = nextRequestToken(module.id);
  moduleErrors[module.id] = "";

  if (!force) {
    const cached = getCachedExploreBooks(getModuleSourceKey(module), module.category);
    if (cached) {
      moduleBooks[module.id] = cached;
      moduleLoading[module.id] = false;
      void fetchBooks(module, requestToken, false).catch(() => {
        if (isCurrentRequest(module.id, requestToken) && !moduleBooks[module.id]?.length) {
          moduleErrors[module.id] = "推荐刷新失败";
        }
      });
      return;
    }
  }

  moduleLoading[module.id] = true;
  try {
    await fetchBooks(module, requestToken, force);
  } catch (error: unknown) {
    if (isCurrentRequest(module.id, requestToken)) {
      moduleBooks[module.id] = [];
      moduleErrors[module.id] = error instanceof Error ? error.message : String(error);
    }
  } finally {
    if (isCurrentRequest(module.id, requestToken)) {
      moduleLoading[module.id] = false;
    }
  }
}

async function loadAllRecommendations(force = false) {
  await Promise.all(activeModules.value.map((module) => loadRecommendation(module, force)));
}

async function refreshRecommendation(module: RecommendationModule) {
  if (moduleRefreshing[module.id]) {
    return;
  }
  moduleRefreshing[module.id] = true;
  try {
    await loadRecommendation(module, true);
    if (!moduleErrors[module.id]) {
      message.success("推荐已刷新");
    }
  } finally {
    moduleRefreshing[module.id] = false;
  }
}

function normalizeCategoriesResult(raw: unknown): ExploreCategoryItem[] | null {
  if (isUrlExploreResult(raw) || isHtmlExploreResult(raw)) {
    return null;
  }
  if (!Array.isArray(raw)) {
    return [];
  }
  const categories = normalizeExploreCategories(raw);
  if (categories.length === 0 || (categories.length === 1 && categories[0].url === "")) {
    return [];
  }
  return categories;
}

function applyDraftCategories(categories: ExploreCategoryItem[], preferredCategory?: string) {
  draftCategories.value = categories;
  const selectableCategories = flattenExploreCategories(categories);
  if (!selectableCategories.length) {
    draft.category = "";
    return;
  }
  if (draft.category && selectableCategories.some((category) => category.url === draft.category)) {
    return;
  }
  if (
    preferredCategory &&
    selectableCategories.some((category) => category.url === preferredCategory)
  ) {
    draft.category = preferredCategory;
    return;
  }
  draft.category = selectableCategories[0].url;
}

async function loadDraftCategories(
  fileName: string,
  preferredCategory?: string,
  sourceDir?: string,
) {
  const requestToken = ++categoryRequestToken;
  draftCategoriesError.value = "";
  draftCategories.value = [];
  if (!fileName) {
    return;
  }

  const cacheKey = sourceDir ? `${sourceDir}::${fileName}` : fileName;
  const cached = getCachedExploreCategories(cacheKey);
  if (cached !== null && cached !== undefined) {
    applyDraftCategories(cached, preferredCategory);
    void refreshDraftCategories(requestToken, fileName, preferredCategory, sourceDir);
    return;
  }

  draftCategoriesLoading.value = true;
  try {
    await refreshDraftCategories(requestToken, fileName, preferredCategory, sourceDir);
  } finally {
    if (requestToken === categoryRequestToken) {
      draftCategoriesLoading.value = false;
    }
  }
}

async function refreshDraftCategories(
  requestToken: number,
  fileName: string,
  preferredCategory?: string,
  sourceDir?: string,
) {
  const raw = await scriptBridgeStore.runExplore(fileName, "GETALL", 1, false, sourceDir);
  if (requestToken !== categoryRequestToken) {
    return;
  }
  const categories = normalizeCategoriesResult(raw);
  if (categories === null) {
    draftCategoriesError.value = "该书源是网页发现页，暂不能放到首页推荐";
    draftCategories.value = [];
    return;
  }
  draftCategoriesError.value = "";
  applyDraftCategories(categories, preferredCategory);
  setCachedExploreCategories(sourceDir ? `${sourceDir}::${fileName}` : fileName, categories);
}

function syncDraftFromModule(module: RecommendationModule) {
  editingModuleId.value = module.id;
  const source = getModuleSource(module);
  Object.assign(draft, {
    enabled: module.enabled,
    fileName: module.fileName,
    sourceDir: module.sourceDir || source?.sourceDir || "",
    category: module.category,
    title: module.title || DEFAULT_CONFIG.title,
    limit: normalizeLimit(module.limit),
    layout: normalizeLayout(module.layout),
  });
}

async function startAddModule() {
  await ensureSourceSystem();
  editingModuleId.value = null;
  Object.assign(draft, {
    ...DEFAULT_CONFIG,
    enabled: true,
    title: configuredModules.value.length
      ? `发现推荐 ${configuredModules.value.length + 1}`
      : "发现推荐",
  });
  draftCategories.value = [];
  draftCategoriesError.value = "";
  if (!draft.fileName && selectableSources.value[0]) {
    draft.fileName = selectableSources.value[0].fileName;
    draft.sourceDir = selectableSources.value[0].sourceDir;
  }
  if (draft.fileName) {
    await loadDraftCategories(draft.fileName, draft.category, draft.sourceDir);
  }
}

async function editModule(moduleId: string) {
  await ensureSourceSystem();
  const module = configuredModules.value.find((item) => item.id === moduleId);
  if (!module) {
    await startAddModule();
    return;
  }
  syncDraftFromModule(module);
  if (module.fileName) {
    await loadDraftCategories(module.fileName, module.category, module.sourceDir);
  }
}

async function openSettings(moduleId?: string) {
  showSettings.value = true;
  await ensureSourceSystem();
  const selectedModule = moduleId
    ? configuredModules.value.find((module) => module.id === moduleId)
    : (editingModule.value ?? configuredModules.value[0]);
  if (selectedModule) {
    await editModule(selectedModule.id);
    return;
  }
  await startAddModule();
}

function closeSettings() {
  settingsDrawerRef.value?.triggerClose();
}

async function onDraftSourceUpdate(value: string) {
  const source = selectableSources.value.find((item) => sourceKeyOf(item) === value);
  draft.fileName = source?.fileName ?? "";
  draft.sourceDir = source?.sourceDir ?? "";
  draft.category = "";
  if (source) {
    draft.enabled = true;
  }
  if (source) {
    await loadDraftCategories(source.fileName, undefined, source.sourceDir);
  } else {
    draftCategories.value = [];
  }
}

async function saveSettings() {
  if (!draft.fileName) {
    message.warning("请选择书源");
    return;
  }
  if (draftCategoriesError.value) {
    message.warning(draftCategoriesError.value);
    return;
  }

  const current = homeConfig.value;
  const moduleId = editingModule.value ? editingModule.value.id : createModuleId();
  const nextModule: RecommendationModule = {
    id: moduleId,
    enabled: draft.enabled,
    fileName: draft.fileName,
    sourceDir: draft.sourceDir || draftSource.value?.sourceDir,
    category: draft.category,
    title: draft.title.trim() || DEFAULT_CONFIG.title,
    limit: normalizeLimit(draft.limit),
    layout: normalizeLayout(draft.layout),
  };
  const existingIndex = current.modules.findIndex((module) => module.id === moduleId);
  const modules = current.modules.map((module) => ({ ...module }));
  if (existingIndex >= 0) {
    modules[existingIndex] = nextModule;
  } else {
    modules.push(nextModule);
  }

  const validIds = new Set(modules.map((module) => module.id));
  const order = current.order.filter((id) => id === SHELF_COMPONENT_ID || validIds.has(id));
  if (!order.includes(SHELF_COMPONENT_ID)) {
    order.push(SHELF_COMPONENT_ID);
  }
  if (!order.includes(moduleId)) {
    const shelfIndex = order.indexOf(SHELF_COMPONENT_ID);
    order.splice(shelfIndex >= 0 ? shelfIndex : order.length, 0, moduleId);
  }

  await persistHomeConfig({ modules, order, shelf: current.shelf });
  editingModuleId.value = moduleId;
  message.success(existingIndex >= 0 ? "推荐模块已保存" : "推荐模块已添加");
  await loadRecommendation(nextModule, true);
}

async function updateModuleEnabled(moduleId: string, enabled: boolean) {
  const current = homeConfig.value;
  const modules = current.modules.map((module) =>
    module.id === moduleId ? { ...module, enabled } : { ...module },
  );
  await persistHomeConfig({
    modules,
    order: [...current.order],
    shelf: current.shelf,
  });
  const module = modules.find((item) => item.id === moduleId);
  if (!module) {
    return;
  }
  if (enabled) {
    await loadRecommendation(module, false);
  } else {
    moduleBooks[moduleId] = [];
    moduleErrors[moduleId] = "";
  }
}

async function disableRecommendation() {
  if (!editingModuleId.value) {
    return;
  }
  await updateModuleEnabled(editingModuleId.value, false);
  draft.enabled = false;
  message.success("已停用此推荐模块");
}

async function deleteModule(moduleId: string) {
  const current = homeConfig.value;
  const modules = current.modules.filter((module) => module.id !== moduleId);
  const order = current.order.filter((id) => id !== moduleId);
  await persistHomeConfig({ modules, order, shelf: current.shelf });
  delete moduleBooks[moduleId];
  delete moduleLoading[moduleId];
  delete moduleRefreshing[moduleId];
  delete moduleErrors[moduleId];
  moduleRequestTokens.delete(moduleId);
  if (editingModuleId.value === moduleId) {
    if (modules[0]) {
      await editModule(modules[0].id);
    } else {
      await startAddModule();
    }
  }
  message.success("推荐模块已删除");
}

async function moveComponent(componentId: string, delta: -1 | 1) {
  const current = homeConfig.value;
  const order = [...current.order];
  const index = order.indexOf(componentId);
  const nextIndex = index + delta;
  if (index < 0 || nextIndex < 0 || nextIndex >= order.length) {
    return;
  }
  const currentId = order[index];
  order[index] = order[nextIndex];
  order[nextIndex] = currentId;
  await persistHomeConfig({
    modules: current.modules,
    order,
    shelf: current.shelf,
  });
}

async function updateShelfConfig(patch: Partial<ShelfComponentConfig>) {
  const current = homeConfig.value;
  await persistHomeConfig({
    modules: current.modules,
    order: [...current.order],
    shelf: normalizeShelfConfig({ ...current.shelf, ...patch }),
  });
}

async function updateShelfShowTitle(showTitle: boolean) {
  await updateShelfConfig({ showTitle });
}

async function updateShelfTitle(title: string) {
  await updateShelfConfig({ title });
}

function canMove(componentId: string, delta: -1 | 1): boolean {
  const index = homeConfig.value.order.indexOf(componentId);
  const nextIndex = index + delta;
  return index >= 0 && nextIndex >= 0 && nextIndex < homeConfig.value.order.length;
}

function selectBook(module: RecommendationModule, book: BookItem) {
  if (!module.fileName) {
    return;
  }
  emit("select", book, module.fileName, getModuleSourceDir(module));
}

function bookSubtitle(book: BookItem): string {
  return book.author?.trim() || book.kind?.trim() || "佚名";
}

function bookMeta(book: BookItem): string {
  return book.latestChapter || book.lastChapter || book.status || book.updateTime || "";
}

function getCategoryLabel(module: RecommendationModule): string {
  return module.category || "默认发现";
}

function getActiveLayout(module: RecommendationModule): RecommendationLayout {
  return normalizeLayout(module.layout);
}

function getActiveLayoutLabel(module: RecommendationModule): string {
  const layout = getActiveLayout(module);
  return LAYOUT_OPTIONS.find((option) => option.key === layout)?.label ?? "横滑轮播";
}

function getRecommendationTitle(module: RecommendationModule): string {
  return module.title.trim() || getCategoryLabel(module);
}

function getRecommendationLimit(module: RecommendationModule): number {
  return normalizeLimit(module.limit);
}

function getModuleDisplayLimit(module: RecommendationModule): number {
  return getActiveLayout(module) === "infiniteGrid"
    ? Math.max(18, getRecommendationLimit(module))
    : getRecommendationLimit(module);
}

function getModuleBooks(module: RecommendationModule): BookItem[] {
  return moduleBooks[module.id] ?? [];
}

function getModuleVisibleBooks(module: RecommendationModule): BookItem[] {
  return getModuleBooks(module).slice(0, getModuleDisplayLimit(module));
}

function getModuleError(module: RecommendationModule): string {
  return moduleErrors[module.id] ?? "";
}

function isModuleLoading(module: RecommendationModule): boolean {
  return moduleLoading[module.id] === true;
}

function isModuleRefreshing(module: RecommendationModule): boolean {
  return moduleRefreshing[module.id] === true;
}

function getModuleHint(module: RecommendationModule): string {
  const source = getModuleSource(module);
  return [
    source?.name ?? (module.fileName || "未选择书源"),
    getCategoryLabel(module),
    getActiveLayoutLabel(module),
  ]
    .filter(Boolean)
    .join(" · ");
}

function getModuleSourceType(module: RecommendationModule): string {
  return getModuleSource(module)?.sourceType ?? "";
}

function isEditingSettingsItem(item: SettingsItem): boolean {
  return item.type === "recommendation" && editingModuleId.value === item.module.id;
}

function getSettingsItemTitle(item: SettingsItem): string {
  if (item.type === "shelf") {
    return "默认书架";
  }
  return getRecommendationTitle(item.module);
}

function getSettingsItemHint(item: SettingsItem): string {
  if (item.type === "shelf") {
    return shelfConfig.value.showTitle
      ? `顶部标题：${shelfConfig.value.title}`
      : "书架书籍网格，顶部标题已隐藏";
  }
  return getModuleHint(item.module);
}

function layoutPreviewClass(layout: RecommendationLayout): string {
  return `bs-rec-layout-preview--${layout}`;
}

function waterfallCoverStyle(index: number) {
  const ratios = ["3 / 4", "4 / 5", "1 / 1", "5 / 7"];
  return { aspectRatio: ratios[index % ratios.length] };
}

function selectCategory(category: string) {
  draft.category = category;
}

onMounted(async () => {
  await ensureRecommendationStorage();
  await persistNormalizedConfigIfNeeded();
  if (activeModules.value.length) {
    await ensureSourceSystem();
    await loadAllRecommendations(false);
  }
});

watch(shelfOrder, (order) => emit("shelf-order-change", order), {
  immediate: true,
});

watch(shelfConfig, (config) => emit("shelf-config-change", { ...config }), {
  immediate: true,
});

watch(activeModuleConfigKey, () => {
  if (!configReady.value) {
    return;
  }
  void loadAllRecommendations(false);
});

defineExpose({ openSettings });
</script>

<template>
  <section
    v-for="{ id, module, order } in visibleRecommendationItems"
    :key="id"
    class="bs-rec"
    :style="{ order }"
    aria-label="首页推荐"
  >
    <div class="bs-rec__head">
      <div class="bs-rec__title-block">
        <div class="bs-rec__title-row">
          <Sparkles :size="16" />
          <h2 class="bs-rec__title">{{ getRecommendationTitle(module) }}</h2>
        </div>
        <div class="bs-rec__meta">
          <span>{{ getModuleSource(module)?.name ?? module.fileName }}</span>
          <span>{{ getCategoryLabel(module) }}</span>
          <span>{{ getActiveLayoutLabel(module) }}</span>
        </div>
      </div>
      <div class="bs-rec__actions">
        <button
          class="bs-rec__icon-btn"
          :class="{ 'bs-rec__icon-btn--spinning': isModuleRefreshing(module) }"
          type="button"
          title="刷新推荐"
          aria-label="刷新推荐"
          :disabled="isModuleLoading(module) || isModuleRefreshing(module)"
          @click="refreshRecommendation(module)"
        >
          <RefreshCw :size="15" />
        </button>
        <button
          class="bs-rec__icon-btn"
          type="button"
          title="设置推荐"
          aria-label="设置推荐"
          @click="openSettings(module.id)"
        >
          <Settings2 :size="15" />
        </button>
      </div>
    </div>

    <div
      v-if="isModuleLoading(module)"
      class="bs-rec__books bs-rec__books--carousel bs-rec__books--loading"
    >
      <div v-for="idx in getRecommendationLimit(module)" :key="idx" class="bs-rec-skeleton">
        <div class="bs-rec-skeleton__cover" />
        <div class="bs-rec-skeleton__line" />
        <div class="bs-rec-skeleton__line bs-rec-skeleton__line--short" />
      </div>
    </div>

    <div v-else-if="getModuleError(module)" class="bs-rec__state">
      <span>{{ getModuleError(module) }}</span>
      <button type="button" class="bs-rec__text-btn" @click="openSettings(module.id)">
        重新选择
      </button>
    </div>

    <div v-else-if="getModuleVisibleBooks(module).length" class="bs-rec__content">
      <div
        v-if="getActiveLayout(module) === 'ranking' || getActiveLayout(module) === 'rankingGrid'"
        class="bs-rec-ranking"
        :class="{
          'bs-rec-ranking--grid': getActiveLayout(module) === 'rankingGrid',
        }"
      >
        <button
          v-for="(book, index) in getModuleVisibleBooks(module)"
          :key="book.bookUrl"
          class="bs-rec-rank-card"
          type="button"
          @click="selectBook(module, book)"
        >
          <span class="bs-rec-rank-card__index">{{ index + 1 }}</span>
          <span class="bs-rec-rank-card__cover">
            <BookCoverImg :src="book.coverUrl" :alt="book.name" :base-url="book.bookUrl" />
          </span>
          <span class="bs-rec-rank-card__body">
            <span class="bs-rec-rank-card__name" :title="book.name || '未知书名'">
              {{ book.name || "未知书名" }}
            </span>
            <span class="bs-rec-rank-card__sub" :title="bookSubtitle(book)">
              {{ bookSubtitle(book) }}
            </span>
            <span v-if="bookMeta(book)" class="bs-rec-rank-card__meta" :title="bookMeta(book)">
              {{ bookMeta(book) }}
            </span>
          </span>
        </button>
      </div>

      <div v-else-if="getActiveLayout(module) === 'buttonGroup'" class="bs-rec-buttons">
        <button
          v-for="(book, index) in getModuleVisibleBooks(module)"
          :key="book.bookUrl"
          class="bs-rec-button-card"
          type="button"
          @click="selectBook(module, book)"
        >
          <span class="bs-rec-button-card__rank">{{ index + 1 }}</span>
          <span class="bs-rec-button-card__text">
            <span class="bs-rec-button-card__name">{{ book.name || "未知书名" }}</span>
            <span class="bs-rec-button-card__sub">{{ bookSubtitle(book) }}</span>
          </span>
        </button>
      </div>

      <div v-else-if="getActiveLayout(module) === 'waterfall'" class="bs-rec-waterfall">
        <button
          v-for="(book, index) in getModuleVisibleBooks(module)"
          :key="book.bookUrl"
          class="bs-rec-card bs-rec-card--waterfall"
          type="button"
          @click="selectBook(module, book)"
        >
          <span class="bs-rec-card__rank">{{ index + 1 }}</span>
          <span class="bs-rec-card__cover" :style="waterfallCoverStyle(index)">
            <BookCoverImg :src="book.coverUrl" :alt="book.name" :base-url="book.bookUrl" />
          </span>
          <span class="bs-rec-card__body">
            <span class="bs-rec-card__name" :title="book.name || '未知书名'">
              {{ book.name || "未知书名" }}
            </span>
            <span class="bs-rec-card__author" :title="bookSubtitle(book)">
              {{ bookSubtitle(book) }}
            </span>
          </span>
        </button>
      </div>

      <div
        v-else
        class="bs-rec__books app-scrollbar"
        :class="`bs-rec__books--${getActiveLayout(module)}`"
      >
        <button
          v-for="(book, index) in getModuleVisibleBooks(module)"
          :key="book.bookUrl"
          class="bs-rec-card"
          :class="{
            'bs-rec-card--featured': getActiveLayout(module) === 'featured' && index === 0,
          }"
          type="button"
          @click="selectBook(module, book)"
        >
          <span class="bs-rec-card__rank">{{ index + 1 }}</span>
          <span v-if="getModuleSourceType(module)" class="bs-rec-card__type">
            <SourceTypeBadge :source-type="getModuleSourceType(module)" :opaque="true" :size="11" />
          </span>
          <span class="bs-rec-card__cover">
            <BookCoverImg :src="book.coverUrl" :alt="book.name" :base-url="book.bookUrl" />
          </span>
          <span class="bs-rec-card__body">
            <span class="bs-rec-card__name" :title="book.name || '未知书名'">
              {{ book.name || "未知书名" }}
            </span>
            <span class="bs-rec-card__author" :title="bookSubtitle(book)">
              {{ bookSubtitle(book) }}
            </span>
            <span v-if="bookMeta(book)" class="bs-rec-card__meta" :title="bookMeta(book)">
              {{ bookMeta(book) }}
            </span>
          </span>
        </button>
      </div>
    </div>

    <div v-else class="bs-rec__state">
      <span>暂无推荐内容</span>
      <button type="button" class="bs-rec__text-btn" @click="refreshRecommendation(module)">
        刷新
      </button>
    </div>
  </section>

  <AppDrawer ref="settingsDrawerRef" v-model:show="showSettings" title="首页编排" :width="520">
    <div class="bs-rec-settings">
      <section class="bs-rec-settings__section">
        <div class="bs-rec-settings__section-head">
          <div>
            <div class="bs-rec-settings__label">首页组件</div>
            <div class="bs-rec-settings__hint">推荐模块和默认书架都可以上下调整顺序</div>
          </div>
          <n-button size="small" type="primary" ghost @click="startAddModule">
            <template #icon>
              <Plus :size="14" />
            </template>
            新推荐
          </n-button>
        </div>

        <div class="bs-rec-order-list">
          <div
            v-for="item in settingsItems"
            :key="item.id"
            class="bs-rec-order-item"
            :class="{
              'bs-rec-order-item--active': isEditingSettingsItem(item),
              'bs-rec-order-item--muted': item.type === 'recommendation' && !item.module.enabled,
            }"
          >
            <GripVertical class="bs-rec-order-item__grip" :size="16" />
            <div class="bs-rec-order-item__body">
              <div class="bs-rec-order-item__title">
                {{ getSettingsItemTitle(item) }}
              </div>
              <div class="bs-rec-order-item__hint">
                {{ getSettingsItemHint(item) }}
              </div>
            </div>
            <div class="bs-rec-order-item__actions">
              <n-button
                size="tiny"
                quaternary
                circle
                :disabled="!canMove(item.id, -1)"
                @click="moveComponent(item.id, -1)"
              >
                <template #icon>
                  <ArrowUp :size="13" />
                </template>
              </n-button>
              <n-button
                size="tiny"
                quaternary
                circle
                :disabled="!canMove(item.id, 1)"
                @click="moveComponent(item.id, 1)"
              >
                <template #icon>
                  <ArrowDown :size="13" />
                </template>
              </n-button>
              <template v-if="item.type === 'recommendation'">
                <n-switch
                  size="small"
                  :value="item.module.enabled"
                  @update:value="(value: boolean) => updateModuleEnabled(item.module.id, value)"
                />
                <n-button size="tiny" quaternary @click="editModule(item.module.id)">设置</n-button>
                <n-button
                  size="tiny"
                  quaternary
                  circle
                  type="error"
                  @click="deleteModule(item.module.id)"
                >
                  <template #icon>
                    <Trash2 :size="13" />
                  </template>
                </n-button>
              </template>
            </div>
          </div>
        </div>

        <div class="bs-rec-shelf-settings">
          <div class="bs-rec-shelf-settings__head">
            <div class="bs-rec-shelf-settings__text">
              <div class="bs-rec-settings__label">默认书架</div>
              <div class="bs-rec-settings__hint">给书架网格加入独立顶部标题</div>
            </div>
            <n-switch
              :value="shelfConfig.showTitle"
              @update:value="(value: boolean) => updateShelfShowTitle(value)"
            />
          </div>
          <div class="bs-rec-settings__row">
            <label class="bs-rec-settings__label" for="bookshelf-shelf-title">书架标题</label>
            <n-input
              id="bookshelf-shelf-title"
              :value="shelfConfig.title"
              :disabled="!shelfConfig.showTitle"
              maxlength="18"
              placeholder="我的书架"
              @update:value="(value: string) => updateShelfTitle(value)"
            />
          </div>
        </div>
      </section>

      <section class="bs-rec-settings__section">
        <div class="bs-rec-settings__section-head">
          <div>
            <div class="bs-rec-settings__label">
              {{ editingModule ? "编辑推荐模块" : "新增推荐模块" }}
            </div>
            <div class="bs-rec-settings__hint">
              {{ draftSource?.name ?? "选择一个发现书源和分类" }}
            </div>
          </div>
          <n-switch v-model:value="draft.enabled" />
        </div>

        <div class="bs-rec-settings__row">
          <label class="bs-rec-settings__label" for="bookshelf-recommend-source">书源</label>
          <n-select
            id="bookshelf-recommend-source"
            :value="draftSourceKey"
            :options="sourceOptions"
            :loading="sourceLoading"
            filterable
            clearable
            placeholder="选择发现书源"
            @update:value="(value: string | null) => onDraftSourceUpdate(value ?? '')"
          />
        </div>

        <div class="bs-rec-settings__row">
          <div class="bs-rec-settings__label">分类</div>
          <div v-if="draftCategoriesLoading" class="bs-rec-settings__loading">
            <n-spin size="small" />
          </div>
          <div v-else-if="draftCategoriesError" class="bs-rec-settings__error">
            {{ draftCategoriesError }}
          </div>
          <div v-else class="bs-rec-settings__cats">
            <button
              v-if="!draftCategories.length && draft.fileName"
              class="bs-rec-settings__cat"
              :class="{ 'bs-rec-settings__cat--active': draft.category === '' }"
              type="button"
              @click="selectCategory('')"
            >
              <Check v-if="draft.category === ''" :size="13" />
              默认发现
            </button>
            <button
              v-for="(category, index) in draftSelectableCategories"
              :key="`${category.url}:${index}`"
              class="bs-rec-settings__cat"
              :class="{
                'bs-rec-settings__cat--active': draft.category === category.url,
              }"
              type="button"
              @click="selectCategory(category.url)"
            >
              <Check v-if="draft.category === category.url" :size="13" />
              {{ category.name }}
            </button>
            <span v-if="!draft.fileName" class="bs-rec-settings__empty">先选择书源</span>
          </div>
        </div>

        <div class="bs-rec-settings__row">
          <div class="bs-rec-settings__label">模块类型</div>
          <div class="bs-rec-layouts">
            <button
              v-for="layout in LAYOUT_OPTIONS"
              :key="layout.key"
              class="bs-rec-layout"
              :class="{ 'bs-rec-layout--active': draft.layout === layout.key }"
              type="button"
              @click="draft.layout = layout.key"
            >
              <span class="bs-rec-layout__preview" :class="layoutPreviewClass(layout.key)">
                <span v-for="idx in 6" :key="idx" />
              </span>
              <span class="bs-rec-layout__text">
                <span class="bs-rec-layout__label">{{ layout.label }}</span>
                <span class="bs-rec-layout__hint">{{ layout.hint }}</span>
              </span>
              <Check v-if="draft.layout === layout.key" class="bs-rec-layout__check" :size="14" />
            </button>
          </div>
        </div>

        <div class="bs-rec-settings__grid">
          <div class="bs-rec-settings__row">
            <label class="bs-rec-settings__label" for="bookshelf-recommend-title">标题</label>
            <n-input
              id="bookshelf-recommend-title"
              v-model:value="draft.title"
              maxlength="18"
              placeholder="发现推荐"
            />
          </div>
          <div class="bs-rec-settings__row">
            <label class="bs-rec-settings__label" for="bookshelf-recommend-limit">数量</label>
            <n-input-number
              id="bookshelf-recommend-limit"
              v-model:value="draft.limit"
              :min="4"
              :max="30"
              :step="1"
            />
          </div>
        </div>
      </section>
    </div>
    <template #footer>
      <div class="bs-rec-settings__footer">
        <n-button quaternary :disabled="!editingModuleId" @click="disableRecommendation">
          停用此模块
        </n-button>
        <div class="bs-rec-settings__footer-actions">
          <n-button @click="startAddModule">
            <template #icon>
              <Plus :size="14" />
            </template>
            新建
          </n-button>
          <n-button @click="closeSettings">完成</n-button>
          <n-button type="primary" :disabled="!draft.fileName" @click="saveSettings">
            保存模块
          </n-button>
        </div>
      </div>
    </template>
  </AppDrawer>
</template>

<style scoped>
.bs-rec {
  padding: 2px 0 18px;
}

.bs-rec__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.bs-rec__title-block {
  min-width: 0;
}

.bs-rec__title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--color-accent);
}

.bs-rec__title {
  margin: 0;
  font-size: var(--fs-16);
  line-height: 1.35;
  font-weight: var(--fw-bold);
  color: var(--color-text);
}

.bs-rec__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 2px;
  font-size: var(--fs-12);
  line-height: 1.4;
  color: var(--color-text-muted);
}

.bs-rec__meta span + span::before {
  content: "·";
  margin-right: 6px;
  color: var(--color-text-muted);
}

.bs-rec__actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.bs-rec__icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: var(--color-text-muted);
  cursor: pointer;
  transition:
    color var(--dur-fast) var(--ease-standard),
    border-color var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard);
}

.bs-rec__icon-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

@media (hover: hover) and (pointer: fine) {
  .bs-rec__icon-btn:hover:not(:disabled) {
    color: var(--color-accent);
    border-color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 9%, transparent);
  }
}

.bs-rec__icon-btn--spinning svg {
  animation: bs-rec-spin 0.8s linear infinite;
}

@keyframes bs-rec-spin {
  to {
    transform: rotate(360deg);
  }
}

.bs-rec__content {
  min-width: 0;
}

.bs-rec__books {
  display: grid;
  gap: 10px;
  padding: 2px 2px 8px;
}

.bs-rec__books--carousel {
  grid-auto-flow: column;
  grid-auto-columns: minmax(106px, 128px);
  overflow-x: auto;
  overflow-y: hidden;
  scroll-snap-type: x proximity;
}

.bs-rec__books--grid,
.bs-rec__books--featured {
  grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
}

.bs-rec__books--infiniteGrid {
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
}

.bs-rec-card {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 0;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: inherit;
  text-align: left;
  cursor: pointer;
  overflow: hidden;
  scroll-snap-align: start;
  transition:
    border-color var(--dur-fast) var(--ease-standard),
    box-shadow var(--dur-fast) var(--ease-standard),
    transform var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .bs-rec-card:hover {
    border-color: var(--color-accent);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.13);
    transform: translateY(-2px);
  }
}

.bs-rec-card__rank {
  position: absolute;
  top: 6px;
  left: 6px;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 22px;
  padding: 0 6px;
  border-radius: var(--radius-1);
  background: rgba(0, 0, 0, 0.54);
  color: #fff;
  font-size: var(--fs-11);
  font-weight: var(--fw-bold);
  line-height: 1;
}

.bs-rec-card__type {
  position: absolute;
  top: 6px;
  right: 6px;
  z-index: 2;
}

.bs-rec-card__cover {
  display: block;
  width: 100%;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  background: var(--color-surface-hover);
}

.bs-rec-card__body {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
  padding: 7px 8px 8px;
}

.bs-rec-card__name {
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  min-height: 2.65em;
  color: var(--color-text);
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
  line-height: 1.32;
}

.bs-rec-card__author,
.bs-rec-card__meta {
  overflow: hidden;
  color: var(--color-text-muted);
  font-size: var(--fs-11);
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bs-rec-card__meta {
  opacity: 0.82;
}

.bs-rec-card--featured {
  grid-column: span 2;
  flex-direction: row;
  min-height: 170px;
}

.bs-rec-card--featured .bs-rec-card__cover {
  width: min(42%, 150px);
  height: auto;
  flex-shrink: 0;
}

.bs-rec-card--featured .bs-rec-card__body {
  justify-content: center;
  padding: 12px;
}

.bs-rec-card--featured .bs-rec-card__name {
  min-height: 0;
  font-size: var(--fs-16);
  -webkit-line-clamp: 3;
}

.bs-rec-ranking {
  display: grid;
  gap: 8px;
}

.bs-rec-ranking--grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.bs-rec-rank-card {
  display: grid;
  grid-template-columns: 28px 48px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 72px;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    border-color var(--dur-fast) var(--ease-standard),
    box-shadow var(--dur-fast) var(--ease-standard),
    transform var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .bs-rec-rank-card:hover {
    border-color: var(--color-accent);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
    transform: translateY(-1px);
  }
}

.bs-rec-rank-card__index {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--color-accent) 13%, transparent);
  color: var(--color-accent);
  font-size: var(--fs-12);
  font-weight: var(--fw-bold);
}

.bs-rec-rank-card__cover {
  display: block;
  width: 48px;
  height: 64px;
  overflow: hidden;
  border-radius: var(--radius-1);
  background: var(--color-surface-hover);
}

.bs-rec-rank-card__body {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.bs-rec-rank-card__name,
.bs-rec-rank-card__sub,
.bs-rec-rank-card__meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bs-rec-rank-card__name {
  color: var(--color-text);
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
}

.bs-rec-rank-card__sub,
.bs-rec-rank-card__meta {
  color: var(--color-text-muted);
  font-size: var(--fs-11);
}

.bs-rec-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.bs-rec-button-card {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 100%;
  min-height: 38px;
  padding: 6px 10px 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: inherit;
  cursor: pointer;
  transition:
    border-color var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .bs-rec-button-card:hover {
    color: var(--color-accent);
    border-color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 8%, transparent);
  }
}

.bs-rec-button-card__rank {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--color-accent) 13%, transparent);
  color: var(--color-accent);
  font-size: var(--fs-11);
  font-weight: var(--fw-bold);
  flex-shrink: 0;
}

.bs-rec-button-card__text {
  display: flex;
  min-width: 0;
  flex-direction: column;
  text-align: left;
}

.bs-rec-button-card__name,
.bs-rec-button-card__sub {
  overflow: hidden;
  max-width: min(220px, 62vw);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bs-rec-button-card__name {
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
  color: var(--color-text);
}

.bs-rec-button-card__sub {
  font-size: var(--fs-11);
  color: var(--color-text-muted);
}

.bs-rec-waterfall {
  column-count: 4;
  column-gap: 10px;
}

.bs-rec-card--waterfall {
  display: inline-flex;
  width: 100%;
  margin: 0 0 10px;
  break-inside: avoid;
}

.bs-rec__state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  min-height: 104px;
  border: 1px dashed var(--color-border);
  border-radius: var(--radius-1);
  color: var(--color-text-muted);
  font-size: var(--fs-13);
}

.bs-rec__text-btn {
  padding: 4px 9px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: var(--color-text-soft);
  cursor: pointer;
}

.bs-rec-skeleton {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  overflow: hidden;
}

.bs-rec-skeleton__cover,
.bs-rec-skeleton__line {
  position: relative;
  overflow: hidden;
  background: var(--color-surface-hover);
}

.bs-rec-skeleton__cover::after,
.bs-rec-skeleton__line::after {
  content: "";
  position: absolute;
  inset: 0;
  transform: translateX(-100%);
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.16), transparent);
  animation: bs-rec-shimmer 1.5s infinite;
}

.bs-rec-skeleton__cover {
  aspect-ratio: 3 / 4;
}

.bs-rec-skeleton__line {
  height: 10px;
  margin: 8px 8px 0;
  border-radius: 999px;
}

.bs-rec-skeleton__line--short {
  width: 62%;
  margin-bottom: 10px;
}

@keyframes bs-rec-shimmer {
  to {
    transform: translateX(100%);
  }
}

.bs-rec-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.bs-rec-settings__section {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}

.bs-rec-settings__section + .bs-rec-settings__section {
  padding-top: 16px;
  border-top: 1px solid var(--color-border);
}

.bs-rec-settings__section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.bs-rec-order-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bs-rec-order-item {
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  transition:
    border-color var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard);
}

.bs-rec-order-item--active {
  border-color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 8%, transparent);
}

.bs-rec-order-item--muted {
  opacity: 0.68;
}

.bs-rec-order-item__grip {
  color: var(--color-text-muted);
}

.bs-rec-order-item__body {
  min-width: 0;
}

.bs-rec-order-item__title,
.bs-rec-order-item__hint {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bs-rec-order-item__title {
  color: var(--color-text);
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
}

.bs-rec-order-item__hint {
  margin-top: 1px;
  color: var(--color-text-muted);
  font-size: var(--fs-11);
}

.bs-rec-order-item__actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
  flex-wrap: wrap;
}

.bs-rec-shelf-settings {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: color-mix(in srgb, var(--color-surface) 88%, transparent);
}

.bs-rec-shelf-settings__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.bs-rec-shelf-settings__text {
  min-width: 0;
}

.bs-rec-settings__row {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 8px;
}

.bs-rec-settings__row--inline {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--color-border);
}

.bs-rec-settings__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 112px;
  gap: 12px;
}

.bs-rec-settings__label {
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
  color: var(--color-text);
}

.bs-rec-settings__hint,
.bs-rec-settings__empty,
.bs-rec-settings__error {
  font-size: var(--fs-12);
  color: var(--color-text-muted);
}

.bs-rec-settings__error {
  color: var(--color-danger);
}

.bs-rec-settings__loading {
  display: flex;
  justify-content: center;
  padding: 18px 0;
}

.bs-rec-settings__cats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  max-height: 220px;
  overflow-y: auto;
}

.bs-rec-settings__cat {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-height: 30px;
  padding: 5px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: var(--color-text-soft);
  font-size: var(--fs-13);
  cursor: pointer;
  transition:
    color var(--dur-fast) var(--ease-standard),
    border-color var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .bs-rec-settings__cat:hover {
    color: var(--color-accent);
    border-color: var(--color-accent);
  }
}

.bs-rec-settings__cat--active {
  color: var(--color-accent);
  border-color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 12%, transparent);
}

.bs-rec-layouts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.bs-rec-layout {
  position: relative;
  display: flex;
  align-items: center;
  gap: 9px;
  min-width: 0;
  min-height: 62px;
  padding: 9px 28px 9px 9px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1);
  background: var(--color-surface);
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    border-color var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard),
    box-shadow var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .bs-rec-layout:hover {
    border-color: var(--color-accent);
  }
}

.bs-rec-layout--active {
  border-color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-accent) 18%, transparent);
}

.bs-rec-layout__preview {
  display: grid;
  width: 50px;
  height: 38px;
  gap: 3px;
  flex-shrink: 0;
  overflow: hidden;
}

.bs-rec-layout__preview span {
  display: block;
  min-width: 0;
  min-height: 0;
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-accent) 28%, var(--color-surface-hover));
}

.bs-rec-layout-preview--carousel {
  grid-auto-flow: column;
  grid-auto-columns: 13px;
}

.bs-rec-layout-preview--ranking {
  grid-template-columns: 1fr;
}

.bs-rec-layout-preview--rankingGrid,
.bs-rec-layout-preview--grid,
.bs-rec-layout-preview--infiniteGrid {
  grid-template-columns: repeat(3, 1fr);
}

.bs-rec-layout-preview--featured {
  grid-template-columns: 1.45fr 1fr 1fr;
}

.bs-rec-layout-preview--featured span:first-child {
  grid-row: span 2;
}

.bs-rec-layout-preview--buttonGroup {
  display: flex;
  flex-wrap: wrap;
  align-content: center;
}

.bs-rec-layout-preview--buttonGroup span {
  width: 21px;
  height: 9px;
  border-radius: 999px;
}

.bs-rec-layout-preview--waterfall {
  grid-template-columns: repeat(3, 1fr);
  align-items: start;
}

.bs-rec-layout-preview--waterfall span:nth-child(2),
.bs-rec-layout-preview--waterfall span:nth-child(5) {
  height: 24px;
}

.bs-rec-layout__text {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1px;
}

.bs-rec-layout__label {
  color: var(--color-text);
  font-size: var(--fs-13);
  font-weight: var(--fw-semibold);
  line-height: 1.25;
}

.bs-rec-layout__hint {
  overflow: hidden;
  color: var(--color-text-muted);
  font-size: var(--fs-11);
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bs-rec-layout__check {
  position: absolute;
  top: 8px;
  right: 8px;
  color: var(--color-accent);
}

.bs-rec-settings__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.bs-rec-settings__footer-actions {
  display: flex;
  gap: 8px;
}

@media (pointer: coarse), (max-width: 640px) {
  .bs-rec {
    padding-bottom: 16px;
  }

  .bs-rec__books--carousel {
    grid-auto-columns: minmax(96px, 112px);
    gap: 8px;
  }

  .bs-rec__books--grid,
  .bs-rec__books--featured,
  .bs-rec__books--infiniteGrid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .bs-rec-card--featured {
    min-height: 150px;
  }

  .bs-rec-ranking--grid {
    grid-template-columns: 1fr;
  }

  .bs-rec-waterfall {
    column-count: 2;
    column-gap: 8px;
  }

  .bs-rec-layouts {
    grid-template-columns: 1fr;
  }

  .bs-rec-order-item {
    grid-template-columns: 18px minmax(0, 1fr);
  }

  .bs-rec-order-item__actions {
    grid-column: 2;
    justify-content: flex-start;
  }

  .bs-rec__state {
    min-height: 92px;
  }

  .bs-rec-settings__grid {
    grid-template-columns: 1fr;
  }

  .bs-rec-settings__footer {
    align-items: stretch;
    flex-direction: column-reverse;
  }

  .bs-rec-settings__footer-actions {
    justify-content: flex-end;
  }
}
</style>
