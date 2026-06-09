import { computed, ref, type ComputedRef, type Ref } from "vue";
import type { ChapterItem } from "@/stores";
import {
  clearChapterRuntimeTextCache,
  clearProcessedRuntimeTextCache,
  createReaderRuntimeTextCache,
} from "@/features/reader/services/readerContentPipeline";
import {
  buildParagraphCommentContext,
  getParagraphCommentCapabilities,
  hashReaderContent,
  normalizeParagraphCommentSummaries,
  type ParagraphCommentContext,
  type ParagraphCommentSummary,
} from "@/features/reader/services/readerParagraphComments";
import { LOCAL_TXT_FILE_NAME } from "@/stores/bookshelf";
import type {
  PaginationEngine,
  ReaderPagePadding,
  ReaderTypography,
  TemporaryChapterSourceOverride,
} from "../types";
import { splitReaderParagraphs } from "../utils/paragraphs";
import { usePagedChapterCache } from "./usePagedChapterCache";

type ReaderPipelineStage =
  | "reader.content.raw"
  | "reader.content.cleaned"
  | "reader.content.beforePaginate"
  | "reader.content.beforeRender";
type ValueSource<T> = Ref<T> | ComputedRef<T>;

interface ReaderSettingsLike {
  typography: ReaderTypography;
  pagePadding: number | ReaderPagePadding;
  paginationEngine: PaginationEngine;
}

interface ReaderContentPayload {
  stage: ReaderPipelineStage;
  content: string;
  sourceType: string;
  fileName: string;
  chapterIndex: number;
  chapterName: string;
  chapterUrl: string;
}

interface UseReaderContentStateOptions {
  fileName: ValueSource<string>;
  sourceDir?: ValueSource<string | undefined>;
  sourceType: ValueSource<string | undefined>;
  currentShelfId: ComputedRef<string | undefined>;
  activeChapterIndex: Ref<number>;
  temporaryChapterOverrides: Ref<Record<number, TemporaryChapterSourceOverride>>;
  readIndices: Ref<Set<number>>;
  cachedIndices: Ref<Set<number>>;
  hasPrev: ComputedRef<boolean>;
  hasNext: ComputedRef<boolean>;
  measureHostRef: Ref<HTMLElement | null>;
  backgroundMeasureHostRef: Ref<HTMLElement | null>;
  settings: ReaderSettingsLike;
  runChapterContent: (fileName: string, chapterUrl: string, sourceDir?: string) => Promise<unknown>;
  getSourceCapabilities: (fileName: string, sourceDir?: string) => Promise<Set<string>>;
  runChapterParagraphCommentCounts: (
    fileName: string,
    chapterUrl: string,
    context: ParagraphCommentContext,
  ) => Promise<unknown>;
  getContent: (shelfId: string, index: number) => Promise<string | null | undefined>;
  saveContent: (shelfId: string, index: number, content: string) => Promise<unknown>;
  getCachedIndices: (shelfId: string) => Promise<Set<number>>;
  getChapter: (index: number) => ChapterItem | undefined;
  buildReaderContentPayload: (
    stage: ReaderPipelineStage,
    contentText: string,
    index: number,
  ) => ReaderContentPayload;
  runReaderContentPipeline: (
    stage: ReaderPipelineStage,
    payload: ReaderContentPayload,
  ) => Promise<string>;
}

function readSource<T>(source: ValueSource<T>): T {
  return source.value;
}

const PAGED_END_SCREEN =
  '<div class="paged-mode-end-screen"><div class="paged-mode-end-screen__icon">📖</div><p class="paged-mode-end-screen__title">已读完最后一章</p><p class="paged-mode-end-screen__sub">全书完，感谢阅读</p></div>';

export function useReaderContentState(options: UseReaderContentStateOptions) {
  const runtimeTextCache = createReaderRuntimeTextCache();
  const {
    rawChapterTextCache,
    rawChapterTextRequests,
    processedChapterTextCache,
    processedChapterTextRequests,
  } = runtimeTextCache;
  const paragraphCommentSummariesByIndex = ref<Record<number, ParagraphCommentSummary[]>>({});
  const paragraphCommentCacheKeyByIndex = new Map<number, string>();
  const paragraphCommentRequests = new Map<string, Promise<ParagraphCommentSummary[]>>();

  async function loadShelfStatus() {
    if (!options.currentShelfId.value) {
      return;
    }

    try {
      options.cachedIndices.value = await options.getCachedIndices(options.currentShelfId.value);
    } catch {
      options.cachedIndices.value = new Set();
    }

    const nextRead = new Set<number>();
    const readUpTo = options.activeChapterIndex.value >= 0 ? options.activeChapterIndex.value : -1;
    for (let index = 0; index <= readUpTo; index++) {
      nextRead.add(index);
    }
    options.readIndices.value = nextRead;
  }

  function markChapterRead(index: number) {
    options.readIndices.value.add(index);
  }

  async function fetchRawChapterText(index: number, forceNetwork = false): Promise<string> {
    const chapter = options.getChapter(index);
    if (!chapter) {
      return "";
    }
    const chapterOverride = options.temporaryChapterOverrides.value[index];

    const isVideo = readSource(options.sourceType) === "video";

    if (forceNetwork || isVideo) {
      rawChapterTextCache.delete(index);
      rawChapterTextRequests.delete(index);
    }

    const cached = rawChapterTextCache.get(index);
    if (!forceNetwork && !isVideo && cached !== undefined) {
      return cached;
    }

    const inflight = rawChapterTextRequests.get(index);
    if (!forceNetwork && !isVideo && inflight) {
      return inflight;
    }

    const request = (async () => {
      let text: string | null = null;

      if (chapterOverride) {
        const raw = await options.runChapterContent(
          chapterOverride.fileName,
          chapterOverride.chapterUrl,
          chapterOverride.sourceDir,
        );
        text = typeof raw === "string" ? raw : String(raw ?? "");
      }

      // 视频类型跳过磁盘缓存读取：m3u8 URL 有时效性，不能复用
      if (!text && !forceNetwork && !isVideo && options.currentShelfId.value) {
        try {
          const shelfText = await options.getContent(options.currentShelfId.value, index);
          text = typeof shelfText === "string" ? shelfText : null;
          if (readSource(options.sourceType) === "comic" && text === "comic") {
            text = null;
          }
        } catch {
          // 回退到网络请求
        }
      }

      if (!text) {
        const currentFileName = readSource(options.fileName);
        if (currentFileName === LOCAL_TXT_FILE_NAME) {
          throw new Error("本地 TXT 章节内容未缓存，请重新导入该书籍");
        }
        const raw = await options.runChapterContent(
          currentFileName,
          chapter.url,
          options.sourceDir ? readSource(options.sourceDir) : undefined,
        );
        text = typeof raw === "string" ? raw : String(raw ?? "");
        // 视频类型不写入磁盘缓存：m3u8 URL 有时效性
        if (!isVideo && options.currentShelfId.value && text) {
          void options.saveContent(options.currentShelfId.value, index, text).catch(() => {});
        }
      }

      const nextText = text ?? "";
      // 视频类型不写入内存缓存
      if (!isVideo) {
        rawChapterTextCache.set(index, nextText);
      }

      if (!isVideo && options.currentShelfId.value && nextText) {
        options.cachedIndices.value.add(index);
      }

      return nextText;
    })();

    rawChapterTextRequests.set(index, request);

    try {
      return await request;
    } finally {
      if (rawChapterTextRequests.get(index) === request) {
        rawChapterTextRequests.delete(index);
      }
    }
  }

  async function fetchProcessedChapterText(
    index: number,
    finalStage: "reader.content.beforePaginate" | "reader.content.beforeRender",
    forceNetwork = false,
  ): Promise<string> {
    const sourceType = readSource(options.sourceType);
    if (sourceType === "comic" || sourceType === "video") {
      return fetchRawChapterText(index, forceNetwork);
    }

    const cacheKey = `${index}:${finalStage}`;
    if (forceNetwork) {
      processedChapterTextCache.delete(cacheKey);
      processedChapterTextRequests.delete(cacheKey);
    }

    const cached = processedChapterTextCache.get(cacheKey);
    if (!forceNetwork && cached !== undefined) {
      return cached;
    }

    const inflight = processedChapterTextRequests.get(cacheKey);
    if (!forceNetwork && inflight) {
      return inflight;
    }

    const request = (async () => {
      let nextText = await fetchRawChapterText(index, forceNetwork);
      nextText = await options.runReaderContentPipeline(
        "reader.content.raw",
        options.buildReaderContentPayload("reader.content.raw", nextText, index),
      );
      nextText = await options.runReaderContentPipeline(
        "reader.content.cleaned",
        options.buildReaderContentPayload("reader.content.cleaned", nextText, index),
      );
      nextText = await options.runReaderContentPipeline(
        "reader.content.beforePaginate",
        options.buildReaderContentPayload("reader.content.beforePaginate", nextText, index),
      );
      if (finalStage === "reader.content.beforeRender") {
        nextText = await options.runReaderContentPipeline(
          "reader.content.beforeRender",
          options.buildReaderContentPayload("reader.content.beforeRender", nextText, index),
        );
      }
      processedChapterTextCache.set(cacheKey, nextText);
      return nextText;
    })();

    processedChapterTextRequests.set(cacheKey, request);

    try {
      return await request;
    } finally {
      if (processedChapterTextRequests.get(cacheKey) === request) {
        processedChapterTextRequests.delete(cacheKey);
      }
    }
  }

  function resolveChapterSource(
    index: number,
  ): { fileName: string; sourceDir?: string; chapterUrl: string } | null {
    const chapter = options.getChapter(index);
    if (!chapter) {
      return null;
    }
    const override = options.temporaryChapterOverrides.value[index];
    return {
      fileName: override?.fileName ?? readSource(options.fileName),
      sourceDir:
        override?.sourceDir ?? (options.sourceDir ? readSource(options.sourceDir) : undefined),
      chapterUrl: override?.chapterUrl ?? chapter.url,
    };
  }

  function setParagraphCommentSummaries(index: number, summaries: ParagraphCommentSummary[]) {
    paragraphCommentSummariesByIndex.value = {
      ...paragraphCommentSummariesByIndex.value,
      [index]: summaries,
    };
  }

  function clearParagraphCommentSummaries(index?: number) {
    if (index === undefined) {
      paragraphCommentSummariesByIndex.value = {};
      paragraphCommentCacheKeyByIndex.clear();
      paragraphCommentRequests.clear();
      return;
    }
    const next = { ...paragraphCommentSummariesByIndex.value };
    delete next[index];
    paragraphCommentSummariesByIndex.value = next;
    paragraphCommentCacheKeyByIndex.delete(index);
  }

  async function ensureParagraphCommentSummaries(
    index: number,
    content: string,
    forceNetwork = false,
  ): Promise<ParagraphCommentSummary[]> {
    const sourceType = readSource(options.sourceType);
    if (
      sourceType === "comic" ||
      sourceType === "video" ||
      sourceType === "music" ||
      sourceType === "audio" ||
      sourceType === "image"
    ) {
      setParagraphCommentSummaries(index, []);
      return [];
    }

    const source = resolveChapterSource(index);
    const chapter = options.getChapter(index);
    if (!source || !chapter) {
      setParagraphCommentSummaries(index, []);
      return [];
    }

    const paragraphs = splitReaderParagraphs(content);
    const contentHash = hashReaderContent(content);
    const cacheKey = `${source.fileName}\x1f${source.chapterUrl}\x1f${contentHash}`;
    if (!forceNetwork && paragraphCommentCacheKeyByIndex.get(index) === cacheKey) {
      return paragraphCommentSummariesByIndex.value[index] ?? [];
    }

    const requestKey = `${index}\x1f${cacheKey}`;
    if (!forceNetwork && paragraphCommentRequests.has(requestKey)) {
      return paragraphCommentRequests.get(requestKey)!;
    }

    const request = (async () => {
      const capabilities = await options
        .getSourceCapabilities(source.fileName, source.sourceDir)
        .catch(() => new Set<string>());
      if (!getParagraphCommentCapabilities(capabilities).counts) {
        paragraphCommentCacheKeyByIndex.set(index, cacheKey);
        setParagraphCommentSummaries(index, []);
        return [];
      }

      const context = buildParagraphCommentContext(
        index,
        chapter.name,
        source.chapterUrl,
        sourceType ?? "novel",
        paragraphs,
        contentHash,
      );
      const raw = await options.runChapterParagraphCommentCounts(
        source.fileName,
        source.chapterUrl,
        context,
      );
      const summaries = normalizeParagraphCommentSummaries(raw, paragraphs.length);
      paragraphCommentCacheKeyByIndex.set(index, cacheKey);
      setParagraphCommentSummaries(index, summaries);
      return summaries;
    })();

    paragraphCommentRequests.set(requestKey, request);
    try {
      return await request;
    } finally {
      if (paragraphCommentRequests.get(requestKey) === request) {
        paragraphCommentRequests.delete(requestKey);
      }
    }
  }

  const pagedCache = usePagedChapterCache({
    activeHostRef: options.measureHostRef,
    backgroundHostRef: options.backgroundMeasureHostRef,
    loadChapterText: (index, forceNetwork) =>
      fetchProcessedChapterText(index, "reader.content.beforePaginate", forceNetwork),
    loadParagraphCommentSummaries: ensureParagraphCommentSummaries,
    getChapterTitle: (index) => options.getChapter(index)?.name ?? "",
    getTypography: () => options.settings.typography,
    getPadding: () => options.settings.pagePadding,
    getPaginationEngine: () => options.settings.paginationEngine,
  });

  const activePagedPages = computed(() => pagedCache.getPages(options.activeChapterIndex.value));
  const activeParagraphCommentSummaries = computed(
    () => paragraphCommentSummariesByIndex.value[options.activeChapterIndex.value] ?? [],
  );
  const prevBoundaryPage = computed(() =>
    options.hasPrev.value
      ? pagedCache.getBoundaryPage(options.activeChapterIndex.value - 1, "last")
      : "",
  );
  const nextBoundaryPage = computed(() => {
    if (!options.hasNext.value) {
      return PAGED_END_SCREEN;
    }
    return pagedCache.getBoundaryPage(options.activeChapterIndex.value + 1, "first");
  });

  function clearChapterRuntimeCache(index: number) {
    clearChapterRuntimeTextCache(runtimeTextCache, index);
    pagedCache.dropChapter(index);
    clearParagraphCommentSummaries(index);
  }

  function clearProcessedRuntimeCache() {
    clearProcessedRuntimeTextCache(runtimeTextCache);
    clearParagraphCommentSummaries();
  }

  function clearAllRuntimeCache() {
    rawChapterTextCache.clear();
    rawChapterTextRequests.clear();
    processedChapterTextCache.clear();
    processedChapterTextRequests.clear();
    clearParagraphCommentSummaries();
  }

  function invalidatePages() {
    pagedCache.invalidatePages();
  }

  return {
    runtimeTextCache,
    loadShelfStatus,
    markChapterRead,
    fetchRawChapterText,
    fetchProcessedChapterText,
    ensureParagraphCommentSummaries,
    activeParagraphCommentSummaries,
    pagedCache,
    activePagedPages,
    prevBoundaryPage,
    nextBoundaryPage,
    clearChapterRuntimeCache,
    clearProcessedRuntimeCache,
    clearAllRuntimeCache,
    invalidatePages,
  };
}
