import type { ComputedRef, Ref } from "vue";
import { nextTick, ref, watch } from "vue";
import { useTts, splitIntoSegments } from "@/composables/useTts";
import type { PagedModeApi, ScrollModeApi } from "./useReaderModeBridge";

interface UseReaderTtsManagerOptions {
  activeChapterIndex: Ref<number>;
  content: Ref<string>;
  isPagedMode: ComputedRef<boolean>;
  isScrollMode: ComputedRef<boolean>;
  isComicMode: ComputedRef<boolean>;
  isVideoMode: ComputedRef<boolean>;
  pagedPageIndex: Ref<number>;
  activePagedPages: ComputedRef<string[]>;
  hasPrev: ComputedRef<boolean>;
  hasNext: ComputedRef<boolean>;
  pagedModeRef: Ref<PagedModeApi | null>;
  scrollModeRef: Ref<ScrollModeApi | null>;
  blockingLoading: ComputedRef<boolean>;
  showTtsBar: Ref<boolean>;
  setPagedPage: (page: number) => void;
  fetchRawChapterText: (index: number) => Promise<string>;
  gotoNextChapter: () => Promise<void>;
}

interface PagedTtsSegmentMeta {
  page: number;
  chapterIdx: number;
  paragraphIndex: number;
}

/** 从页面 HTML 字符串中提取段落块文本，避免按视觉行朗读 */
function extractPageParagraphs(html: string): string[] {
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, "text/html");
  return Array.from(doc.querySelectorAll(".reader-block--paragraph, .reader-block--title"))
    .map((block) => {
      const lines = Array.from(block.querySelectorAll(".reader-line"));
      if (lines.length === 0) {
        return block.textContent?.trim() ?? "";
      }
      return lines
        .map((line) => line.textContent ?? "")
        .join("")
        .trim();
    })
    .filter(Boolean);
}

export function useReaderTtsManager(options: UseReaderTtsManagerOptions) {
  const {
    activeChapterIndex,
    content,
    isPagedMode: _isPagedMode,
    isScrollMode,
    isComicMode,
    isVideoMode,
    pagedPageIndex,
    activePagedPages,
    hasPrev: _hasPrev,
    hasNext,
    pagedModeRef,
    scrollModeRef,
    blockingLoading,
    showTtsBar,
    setPagedPage,
    fetchRawChapterText,
    gotoNextChapter,
  } = options;

  void _hasPrev; // referenced in future: not currently used
  void _isPagedMode;

  const tts = useTts();
  const ttsProgressText = ref("—");
  const ttsScrollHighlightIdx = ref(-1);

  // TTS 分页模式状态（跨章节持续累计）
  let ttsPagedSegmentMetas: PagedTtsSegmentMeta[] = [];
  let ttsFeedPage = 0;
  let ttsFeedChapter = -1;

  /**
   * 等待下一章加载完成后返回。
   */
  async function gotoNextChapterAndWait(): Promise<void> {
    if (!hasNext.value) {
      return;
    }
    const targetIdx = activeChapterIndex.value + 1;
    await gotoNextChapter();
    if (blockingLoading.value) {
      await new Promise<void>((resolve) => {
        const stop = watch(
          [blockingLoading, activeChapterIndex] as const,
          ([isLoading, idx]) => {
            if (idx === targetIdx && !isLoading) {
              stop();
              resolve();
            }
          },
          { immediate: true },
        );
      });
    }
  }

  /** 构建分页模式的 TTS 启动参数 */
  function buildPagedTtsOptions() {
    const startPage = pagedPageIndex.value;
    const pages = activePagedPages.value;
    if (pages.length === 0) {
      return null;
    }

    ttsPagedSegmentMetas = [];
    ttsFeedPage = startPage;
    ttsFeedChapter = activeChapterIndex.value;
    let globalIdx = 0;

    const collectPageSegments = (page: number, chapterIdx: number, pageHtml: string): string[] => {
      const paragraphs = extractPageParagraphs(pageHtml);
      const segments: string[] = [];
      paragraphs.forEach((paragraph, paragraphIndex) => {
        const paragraphSegments = splitIntoSegments(paragraph);
        for (const segment of paragraphSegments) {
          ttsPagedSegmentMetas[globalIdx] = {
            page,
            chapterIdx,
            paragraphIndex,
          };
          globalIdx += 1;
          segments.push(segment);
        }
      });
      return segments;
    };

    const initialSegments: string[] = [];
    for (let page = startPage; page < pages.length; page++) {
      const pageSegments = collectPageSegments(page, ttsFeedChapter, pages[page] ?? "");
      if (pageSegments.length === 0) {
        continue;
      }
      initialSegments.push(...pageSegments);
      ttsFeedPage = page;
    }

    if (initialSegments.length === 0) {
      return null;
    }

    const onNeedMore = async (): Promise<string[] | null> => {
      const nextPage = ttsFeedPage + 1;
      const currentPages = activePagedPages.value;

      if (nextPage < currentPages.length) {
        ttsFeedPage = nextPage;
        const pageSegments = collectPageSegments(
          nextPage,
          ttsFeedChapter,
          currentPages[nextPage] ?? "",
        );
        return pageSegments.length > 0 ? pageSegments : onNeedMore();
      }

      if (!hasNext.value) {
        return null;
      }
      void fetchRawChapterText(activeChapterIndex.value + 1);
      await gotoNextChapterAndWait();

      ttsFeedPage = 0;
      ttsFeedChapter = activeChapterIndex.value;
      const newPages = activePagedPages.value;
      if (newPages.length === 0) {
        return null;
      }

      const pageSegments = collectPageSegments(0, ttsFeedChapter, newPages[0] ?? "");
      return pageSegments.length > 0 ? pageSegments : onNeedMore();
    };

    const onSegmentStart = (gIdx: number) => {
      const meta = ttsPagedSegmentMetas[gIdx];
      if (!meta) {
        return;
      }

      const total =
        meta.chapterIdx === activeChapterIndex.value ? activePagedPages.value.length : 0;
      ttsProgressText.value =
        total > 0 ? `第 ${meta.page + 1}/${total} 页` : `第 ${meta.page + 1} 页`;

      if (meta.chapterIdx === activeChapterIndex.value && meta.page !== pagedPageIndex.value) {
        setPagedPage(meta.page);
      }

      void nextTick(() => {
        pagedModeRef.value?.highlightParagraph?.(meta.paragraphIndex);
      });
    };

    return {
      initialSegments,
      onNeedMore,
      onSegmentStart,
      onAllDone: () => {
        showTtsBar.value = false;
      },
    };
  }

  /** 构建滚动模式的 TTS 启动参数 */
  function buildScrollTtsOptions() {
    const startPara = scrollModeRef.value?.getFirstVisibleParaIndex?.() ?? 0;
    ttsFeedChapter = activeChapterIndex.value;

    const paragraphs = content.value.split(/\n+/).filter((p) => p.trim());
    const initialSegs = paragraphs.slice(startPara).flatMap((p) => splitIntoSegments(p));

    if (initialSegs.length === 0) {
      return null;
    }

    const segToParaMap: number[] = [];
    for (let pi = startPara; pi < paragraphs.length; pi++) {
      const segs = splitIntoSegments(paragraphs[pi] ?? "");
      for (let si = 0; si < segs.length; si++) {
        segToParaMap.push(pi);
      }
    }
    const onNeedMore = async (): Promise<string[] | null> => {
      if (!hasNext.value) {
        return null;
      }
      void fetchRawChapterText(activeChapterIndex.value + 1);
      await gotoNextChapterAndWait();
      ttsFeedChapter = activeChapterIndex.value;
      const newParas = content.value.split(/\n+/).filter((p) => p.trim());
      const newSegs = newParas.flatMap((p) => splitIntoSegments(p));
      for (let pi = 0; pi < newParas.length; pi++) {
        const segs = splitIntoSegments(newParas[pi] ?? "");
        for (let si = 0; si < segs.length; si++) {
          segToParaMap.push(pi);
        }
      }
      return newSegs.length > 0 ? newSegs : null;
    };

    const onSegmentStart = (gIdx: number) => {
      const paraIdx = segToParaMap[gIdx] ?? -1;
      if (paraIdx >= 0) {
        ttsScrollHighlightIdx.value = paraIdx;
        ttsProgressText.value = `第 ${paraIdx + 1} 段`;
        void nextTick(() => {
          const el = scrollModeRef.value;
          if (!el) {
            return;
          }
          const container = el.$el ?? null;
          if (!container) {
            return;
          }
          const paras = container.querySelectorAll<HTMLElement>(".scroll-mode__para");
          paras[paraIdx]?.scrollIntoView({
            behavior: "smooth",
            block: "center",
          });
        });
      }
    };

    return {
      initialSegments: initialSegs,
      onNeedMore,
      onSegmentStart,
      onAllDone: () => {
        showTtsBar.value = false;
      },
    };
  }

  function onTtsToggle() {
    if (showTtsBar.value) {
      showTtsBar.value = false;
      tts.stop();
      return;
    }

    if (isComicMode.value || isVideoMode.value) {
      return;
    }

    let opts: ReturnType<typeof buildPagedTtsOptions>;
    if (isScrollMode.value) {
      opts = buildScrollTtsOptions();
    } else {
      opts = buildPagedTtsOptions();
    }
    if (!opts) {
      return;
    }

    showTtsBar.value = true;
    ttsProgressText.value = "—";
    tts.startReading(opts);
  }

  // TTS 控制条关闭时停止播放 + 清除高亮
  watch(
    () => showTtsBar.value,
    (v) => {
      if (!v) {
        tts.stop();
        ttsScrollHighlightIdx.value = -1;
        pagedModeRef.value?.clearTtsHighlight?.();
        ttsProgressText.value = "—";
      }
    },
  );

  // 章节切换时如果 TTS 未在播放，清理遗留状态
  watch(activeChapterIndex, () => {
    if (!showTtsBar.value) {
      tts.stop();
      ttsScrollHighlightIdx.value = -1;
    }
  });

  return {
    tts,
    ttsProgressText,
    ttsScrollHighlightIdx,
    showTtsBar,
    onTtsToggle,
  };
}
