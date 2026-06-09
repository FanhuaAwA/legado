import { computed, ref, type ComputedRef, type Ref } from "vue";
import type { ChapterItem } from "@/stores";
import type { TemporaryChapterSourceOverride } from "../types";

type PagedModeKind = "slide" | "cover" | "simulation" | "none";
type ValueSource<T> = Ref<T> | ComputedRef<T>;

interface ReaderSettingsLike {
  skinPresetId?: string;
  flipMode: string;
}

interface ReaderSkinLike {
  localId: string;
  lockedFlipMode?: string | null;
}

interface UseReaderChapterContextOptions {
  shelfBookId: ValueSource<string | undefined>;
  sourceType: ValueSource<string | undefined>;
  chapterName: ValueSource<string>;
  chapterUrl: ValueSource<string>;
  chapters: ValueSource<ChapterItem[]>;
  activeChapterIndex: Ref<number>;
  temporaryChapterOverrides: Ref<Record<number, TemporaryChapterSourceOverride>>;
  settings: ReaderSettingsLike;
  getContentStyle: () => Record<string, string>;
  readerAppearanceVars: ComputedRef<Record<string, string>>;
  readerSkins: ComputedRef<ReaderSkinLike[]>;
}

function readSource<T>(source: ValueSource<T>): T {
  return source.value;
}

export function useReaderChapterContext(options: UseReaderChapterContextOptions) {
  const localAddedShelfId = ref("");
  const currentShelfId = computed(() => readSource(options.shelfBookId) ?? localAddedShelfId.value);
  const isOnShelf = computed(() => !!currentShelfId.value);
  const addingToShelf = ref(false);
  const chapters = computed(() => readSource(options.chapters));
  const sourceType = computed(() => readSource(options.sourceType));
  const fallbackChapterName = computed(() => readSource(options.chapterName));
  const fallbackChapterUrl = computed(() => readSource(options.chapterUrl));

  function getChapter(index: number): ChapterItem | undefined {
    return index >= 0 && index < chapters.value.length ? chapters.value[index] : undefined;
  }

  const activeChapter = computed(() => getChapter(options.activeChapterIndex.value));
  const hasPrev = computed(() => options.activeChapterIndex.value > 0);
  const hasNext = computed(() => options.activeChapterIndex.value < chapters.value.length - 1);
  const currentChapterName = computed(() => activeChapter.value?.name ?? fallbackChapterName.value);
  const currentChapterUrl = computed(() => activeChapter.value?.url ?? fallbackChapterUrl.value);
  const currentChapterOverride = computed(
    () => options.temporaryChapterOverrides.value[options.activeChapterIndex.value] ?? null,
  );

  const isComicMode = computed(() => sourceType.value === "comic");
  const isVideoMode = computed(() => sourceType.value === "video");
  const activeSkinLockedFlipMode = computed<string | null>(() => {
    const skinId = options.settings.skinPresetId;
    if (!skinId) {
      return null;
    }
    return (
      options.readerSkins.value.find((skin) => skin.localId === skinId)?.lockedFlipMode ?? null
    );
  });
  const effectiveFlipMode = computed(
    () => activeSkinLockedFlipMode.value ?? options.settings.flipMode,
  );
  const isScrollMode = computed(
    () => !isComicMode.value && !isVideoMode.value && effectiveFlipMode.value === "scroll",
  );
  const pagedMode = computed<PagedModeKind | null>(() => {
    if (isComicMode.value || isVideoMode.value || effectiveFlipMode.value === "scroll") {
      return null;
    }
    return effectiveFlipMode.value as PagedModeKind;
  });
  const legacyPagedMode = computed<PagedModeKind | null>(() => pagedMode.value ?? null);
  const isPagedMode = computed(() => pagedMode.value !== null);

  const effectiveStyle = computed(() => {
    const base = options.getContentStyle();
    if (isComicMode.value || isVideoMode.value) {
      base["--reader-bg-color"] = "#000000";
      base["--reader-bg-image"] = "none";
      base["--reader-bg-size"] = "auto";
      base["--reader-bg-position"] = "center";
      base["--reader-bg-repeat"] = "no-repeat";
      base["--reader-bg-attachment"] = "scroll";
      base["--reader-bg-blend-mode"] = "normal";
      base["--reader-text-color"] = "#ffffff";
    }
    Object.assign(base, options.readerAppearanceVars.value);
    base["--reader-tts-hl-bg"] =
      "color-mix(in srgb, var(--reader-selection-color) 65%, transparent)";
    // 小说模式：正文区从顶部安全区域下方开始，避免文字遮挡状态栏；两者相互独立
    // 漫画/视频模式保持沉浸式全屏，不做偏移
    // 注意：这里显式为两种模式都赋值，effectiveStyle 是 --reader-body-top 的唯一控制来源，
    // 避免与 ChapterReaderModal.vue scoped CSS 中已删除的默认值产生级联竞争
    if (!isComicMode.value && !isVideoMode.value) {
      base["--reader-body-top"] = "var(--safe-area-inset-top, env(safe-area-inset-top, 0px))";
    } else {
      base["--reader-body-top"] = "0px";
    }
    return base;
  });

  return {
    localAddedShelfId,
    currentShelfId,
    isOnShelf,
    addingToShelf,
    getChapter,
    activeChapter,
    hasPrev,
    hasNext,
    currentChapterName,
    currentChapterUrl,
    currentChapterOverride,
    isComicMode,
    isVideoMode,
    activeSkinLockedFlipMode,
    effectiveFlipMode,
    isScrollMode,
    pagedMode,
    legacyPagedMode,
    isPagedMode,
    effectiveStyle,
  };
}
