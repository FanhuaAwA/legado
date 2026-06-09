<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { ParagraphCommentClickPayload } from "@/features/reader/services/readerParagraphComments";
import type { PaginationMeasurementData } from "../composables/usePagination";
import type { ReaderTapAction } from "../types";
import LayoutDebugIndicator from "../LayoutDebugIndicator.vue";

type PagedModeVariant = "slide" | "cover" | "simulation" | "none";
type FlipAction = "next" | "prev";
type DragDir = "left" | "right" | null;

const props = defineProps<{
  mode: PagedModeVariant;
  chapterKey?: string | number;
  pages: string[];
  currentPage: number;
  prevBoundaryPage?: string;
  nextBoundaryPage?: string;
  hasPrevChapter?: boolean;
  hasNextChapter?: boolean;
  tapZoneLeft?: number;
  tapZoneRight?: number;
  tapLeftAction?: ReaderTapAction;
  tapRightAction?: ReaderTapAction;
  selectionMode?: boolean;
  busy?: boolean;
  layoutDebug?: boolean;
  tapZoneDebug?: boolean;
  paginationMeasurement?: PaginationMeasurementData | null;
}>();

const emit = defineEmits<{
  (e: "tap", zone: "center"): void;
  (e: "update:currentPage", page: number): void;
  (e: "request-prev-chapter"): void;
  (e: "request-next-chapter"): void;
  (e: "progress", ratio: number): void;
  (e: "paragraph-comment-click", payload: ParagraphCommentClickPayload): void;
}>();

const containerRef = ref<HTMLElement | null>(null);
const isAnimating = ref(false);
const simulationTransitionArmed = ref(false);
const boundaryMsg = ref("");
const pendingStaticPageHtml = ref<string | null>(null);

let boundaryTimer = 0;
let animationTimer: number | null = null;
let deferredActionToken = 0;
let animationRunId = 0;

// ── Simulation rAF 优化：拖拽阶段绕过 Vue 响应式，直接操作 DOM ──
// 这些 plain 变量在 touchmove 热路径中使用，不会触发 Vue 重渲染
let pendingRafId = 0;
let cachedContainerWidth = 0;
let rawPointerX = 0;
let rawDragDir: Exclude<DragDir, null> = "left";

// simulation 阶段各层 DOM 引用
const simulationRevealEl = ref<HTMLElement | null>(null);
const simulationCurrentEl = ref<HTMLElement | null>(null);
const simulationCurlEl = ref<HTMLElement | null>(null);
const simulationCurlContentEl = ref<HTMLElement | null>(null);
const simulationShadowEl = ref<HTMLElement | null>(null);
/** 翻章锁：从发出 request-next/prev-chapter 到新章内容载入期间，鸟所有微手势都不再触发翻章事件 */
let chapterChanging = false;
/**
 * 记录最近一次 touchstart 的时间戳。
 * 移动端在 touchend 后约 300~600ms 会生成合成 mousedown/mouseup/click，
 * 导致手势处理器被重复执行引发连续翻页。
 * onPointerDown 通过比较时间戳差过滤掉这类合成事件。
 */
let lastTouchTime = 0;
let activeAnimation: {
  action: FlipAction | null;
  commitOnFinish: boolean;
} = {
  action: null,
  commitOnFinish: false,
};

const totalPages = computed(() => props.pages.length);
const hasPrevPage = computed(() => props.currentPage > 0);
const hasNextPage = computed(() => props.currentPage < totalPages.value - 1);
const leftRatio = computed(() => props.tapZoneLeft ?? 0.3);
const rightRatio = computed(() => props.tapZoneRight ?? 0.7);
const leftTapAction = computed<FlipAction>(() => props.tapLeftAction ?? "prev");
const rightTapAction = computed<FlipAction>(() => props.tapRightAction ?? "next");
const isLayeredMode = computed(() => props.mode === "cover" || props.mode === "simulation");
const transitionDurationMs = computed(() => (props.mode === "simulation" ? 360 : 260));
const tapMenuStart = computed(() => leftRatio.value);
const tapMenuEnd = computed(() => rightRatio.value);

const currentPageHtml = computed(
  () => pendingStaticPageHtml.value ?? props.pages[props.currentPage] ?? "",
);
const prevPageHtml = computed(() => {
  const prev = props.pages[props.currentPage - 1];
  if (prev) {
    return prev;
  }
  return !hasPrevPage.value ? (props.prevBoundaryPage ?? "") : "";
});
const nextPageHtml = computed(() => {
  const next = props.pages[props.currentPage + 1];
  if (next) {
    return next;
  }
  return !hasNextPage.value ? (props.nextBoundaryPage ?? "") : "";
});

const pageInfo = computed(() => {
  if (totalPages.value <= 0) {
    return "";
  }
  return `${Math.min(props.currentPage + 1, totalPages.value)}/${totalPages.value}`;
});

const showAnyDebug = computed(() => !!props.layoutDebug || !!props.tapZoneDebug);
const leftTapLabel = computed(() => (leftTapAction.value === "prev" ? "上一页" : "下一页"));
const rightTapLabel = computed(() => (rightTapAction.value === "prev" ? "上一页" : "下一页"));
const debugPrevWidth = computed(() => tapMenuStart.value);
const debugCenterWidth = computed(() => Math.max(0, tapMenuEnd.value - tapMenuStart.value));
const debugNextWidth = computed(() => Math.max(0, 1 - tapMenuEnd.value));

// 从容器读取实时测量数据用于诊断
const runtimeMeasurement = computed<PaginationMeasurementData | null>(() => {
  if (!props.layoutDebug || !containerRef.value) {
    return null;
  }

  // 如果有上层传递的数据，优先使用
  if (props.paginationMeasurement) {
    return props.paginationMeasurement;
  }

  // 否则从容器推导（限制精度）
  const container = containerRef.value;
  const computedStyle = getComputedStyle(container);
  const paddingTop = parseFloat(computedStyle.paddingTop) || 0;
  const paddingRight = parseFloat(computedStyle.paddingRight) || 0;
  const paddingBottom = parseFloat(computedStyle.paddingBottom) || 0;
  const paddingLeft = parseFloat(computedStyle.paddingLeft) || 0;

  return {
    containerWidth: container.clientWidth,
    containerHeight: container.clientHeight,
    availableWidth: Math.max(0, container.clientWidth - paddingLeft - paddingRight),
    availableHeight: Math.max(0, container.clientHeight - paddingTop - paddingBottom),
    padding: {
      top: paddingTop,
      right: paddingRight,
      bottom: paddingBottom,
      left: paddingLeft,
    },
    engine: "pretext",
    lineHeightPx: 24, // 默认估计值
    fontSize: 16, // 默认估计值
  };
});

function getContainerWidth(): number {
  return containerRef.value?.clientWidth ?? window.innerWidth;
}

function showBoundary(message: string) {
  boundaryMsg.value = message;
  clearTimeout(boundaryTimer);
  boundaryTimer = window.setTimeout(() => {
    boundaryMsg.value = "";
  }, 1500);
}

function canRunAction(action: FlipAction): boolean {
  if (props.busy || chapterChanging || totalPages.value <= 0) {
    return false;
  }
  if (action === "next") {
    return hasNextPage.value || !!props.hasNextChapter;
  }
  return hasPrevPage.value || !!props.hasPrevChapter;
}

function actionLeavesCurrentChapter(action: FlipAction): boolean {
  return action === "next"
    ? !hasNextPage.value && !!props.hasNextChapter
    : !hasPrevPage.value && !!props.hasPrevChapter;
}

function clearChapterChangeVisualState() {
  chapterChanging = false;
  pendingStaticPageHtml.value = null;
  isAnimating.value = false;
  resetVisualState();
}

function emitAction(action: FlipAction) {
  if (action === "next") {
    if (hasNextPage.value) {
      emit("update:currentPage", props.currentPage + 1);
      return;
    }
    if (props.hasNextChapter) {
      chapterChanging = true;
      if (props.mode === "none") {
        pendingStaticPageHtml.value = nextPageHtml.value;
      }
      emit("request-next-chapter");
      return;
    }
    showBoundary("已经到最后一页了");
    return;
  }

  if (hasPrevPage.value) {
    emit("update:currentPage", props.currentPage - 1);
    return;
  }
  if (props.hasPrevChapter) {
    chapterChanging = true;
    if (props.mode === "none") {
      pendingStaticPageHtml.value = prevPageHtml.value;
    }
    emit("request-prev-chapter");
    return;
  }
  showBoundary("已经到最前了");
}

function clearAnimationTimer() {
  if (animationTimer !== null) {
    window.clearTimeout(animationTimer);
    animationTimer = null;
  }
}

function animationStaysInCurrentChapter(action: FlipAction): boolean {
  return action === "next" ? hasNextPage.value : hasPrevPage.value;
}

function finishAnimation(commit = true) {
  const action = activeAnimation.action;
  const shouldCommit = commit && activeAnimation.commitOnFinish && action !== null;
  const shouldHoldBoundaryFrame =
    shouldCommit && action !== null && actionLeavesCurrentChapter(action);

  animationRunId += 1;
  clearAnimationTimer();
  activeAnimation = {
    action: null,
    commitOnFinish: false,
  };

  if (shouldHoldBoundaryFrame && action) {
    emitAction(action);
    return;
  }

  isAnimating.value = false;
  resetVisualState();

  if (shouldCommit && action) {
    emitAction(action);
  }
}

function queueAction(action: FlipAction): boolean {
  if (props.busy || chapterChanging || totalPages.value <= 0) {
    return false;
  }

  if (isAnimating.value) {
    const interruptedAction = activeAnimation.action;
    const interruptedStaysInChapter =
      interruptedAction !== null &&
      activeAnimation.commitOnFinish &&
      animationStaysInCurrentChapter(interruptedAction);
    const interruptedWasBounce = !activeAnimation.commitOnFinish;

    finishAnimation(true);

    if (interruptedStaysInChapter || interruptedWasBounce) {
      const token = ++deferredActionToken;
      void nextTick(() => {
        if (token !== deferredActionToken) {
          return;
        }
        void runAction(action);
      });
    }

    return true;
  }

  if (!canRunAction(action)) {
    if (!props.busy && totalPages.value > 0) {
      showBoundary(action === "next" ? "已经到最后一页了" : "已经到最前了");
    }
    return false;
  }

  deferredActionToken += 1;
  void runAction(action);
  return true;
}

function bounceBack(direction: DragDir) {
  if (props.mode === "none") {
    return;
  }

  if (props.mode === "slide") {
    slideSnapOffset.value = 0;
    activeAnimation = {
      action: null,
      commitOnFinish: false,
    };
    isAnimating.value = true;
    clearAnimationTimer();
    animationTimer = window.setTimeout(() => finishAnimation(false), transitionDurationMs.value);
    return;
  }

  dragDir.value = direction;
  if (props.mode === "simulation") {
    layerSnapTarget.value = layerDragOffset.value;
    simulationTransitionArmed.value = false;
  } else {
    layerSnapTarget.value = 0;
  }
  activeAnimation = {
    action: null,
    commitOnFinish: false,
  };
  isAnimating.value = true;
  if (props.mode === "simulation") {
    void nextTick(async () => {
      if (!isAnimating.value || activeAnimation.action !== null) {
        return;
      }
      void containerRef.value?.offsetHeight;
      simulationTransitionArmed.value = true;
      await nextTick();
      if (!isAnimating.value || activeAnimation.action !== null) {
        return;
      }
      void containerRef.value?.offsetHeight;
      layerSnapTarget.value = 0;
    });
  }
  clearAnimationTimer();
  animationTimer = window.setTimeout(() => finishAnimation(false), transitionDurationMs.value);
}

async function runAction(action: FlipAction) {
  const runId = ++animationRunId;

  if (!canRunAction(action)) {
    showBoundary(action === "next" ? "已经到最后一页了" : "已经到最前了");
    return;
  }

  if (props.mode === "none") {
    emitAction(action);
    return;
  }

  if (props.mode === "slide") {
    slideSnapOffset.value = action === "next" ? -getContainerWidth() : getContainerWidth();
    activeAnimation = {
      action,
      commitOnFinish: true,
    };
    isAnimating.value = true;
    clearAnimationTimer();
    animationTimer = window.setTimeout(() => finishAnimation(true), transitionDurationMs.value);
    return;
  }

  dragDir.value = action === "next" ? "left" : "right";
  layerDragOffset.value = 0;
  layerSnapTarget.value = 0;
  simulationTransitionArmed.value = props.mode !== "simulation";
  activeAnimation = {
    action,
    commitOnFinish: true,
  };
  isAnimating.value = true;
  await nextTick();
  if (runId !== animationRunId || activeAnimation.action !== action) {
    return;
  }
  void containerRef.value?.offsetHeight;
  if (props.mode === "simulation") {
    simulationTransitionArmed.value = true;
    await nextTick();
    if (runId !== animationRunId || activeAnimation.action !== action) {
      return;
    }
    void containerRef.value?.offsetHeight;
  }
  layerSnapTarget.value = action === "next" ? -getContainerWidth() : getContainerWidth();
  clearAnimationTimer();
  animationTimer = window.setTimeout(() => finishAnimation(true), transitionDurationMs.value);
}

function flipNext() {
  return queueAction("next");
}

function flipPrev() {
  return queueAction("prev");
}

function goToPage(page: number) {
  if (page >= 0 && page < totalPages.value) {
    emit("update:currentPage", page);
  }
}

// 模式切换时清理动画/拖拽残留状态，防止旧模式的 animationTimer 在新模式下触发意外翻页
watch(
  () => props.mode,
  (newMode, oldMode) => {
    if (newMode !== oldMode) {
      clearAnimationTimer();
      finishAnimation(false);
      dragging = false;
    }
  },
);

watch(
  () => [props.currentPage, totalPages.value] as const,
  ([page, total]) => {
    if (total <= 0) {
      emit("progress", 0);
      return;
    }
    if (total === 1) {
      emit("progress", 1);
      return;
    }
    emit("progress", Math.min(1, Math.max(0, page / (total - 1))));
  },
  { immediate: true },
);

// 新章节真正切换后再解除翻章锁；只看 pages 会被旧章节异步重排误触发。
watch(
  () => props.chapterKey,
  (chapterKey, previousChapterKey) => {
    if (chapterKey !== previousChapterKey) {
      clearChapterChangeVisualState();
    }
  },
);

// 兼容未传 chapterKey 的外部用法。
watch(
  () => props.pages,
  () => {
    if (props.chapterKey === undefined) {
      clearChapterChangeVisualState();
    }
  },
);

watch(
  () => props.busy,
  async (busy) => {
    if (busy) {
      return;
    }
    await nextTick();
    if (chapterChanging) {
      clearChapterChangeVisualState();
    }
  },
);

watch(
  () => props.selectionMode,
  (active) => {
    if (!active) {
      return;
    }
    dragging = false;
    finishAnimation(false);
    resetVisualState();
  },
);

/* ---- Slide ---- */
const slideDragOffset = ref(0);
const slideSnapOffset = ref(0);

const slideTrackTranslateX = computed(
  () => -getContainerWidth() + (isAnimating.value ? slideSnapOffset.value : slideDragOffset.value),
);

/* ---- Cover / Simulation ---- */
const dragDir = ref<DragDir>(null);
const layerDragOffset = ref(0);
const layerSnapTarget = ref(0);

const fgHtml = computed(() =>
  dragDir.value === "right" ? prevPageHtml.value : currentPageHtml.value,
);
const bgHtml = computed(() =>
  dragDir.value === "right" ? currentPageHtml.value : nextPageHtml.value,
);

const coverTranslateX = computed(() => {
  const width = getContainerWidth();
  const base = dragDir.value === "right" ? -width : 0;
  return base + (isAnimating.value ? layerSnapTarget.value : layerDragOffset.value);
});

const simulationPointerX = ref<number | null>(null);

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

const CURL_SCALE_START = 0.86;
const CURL_SCALE_DELTA = 0.18;
const CURL_EXIT_OVERSHOOT = 24;
const SIMULATION_SHADOW_MAX_OPACITY = 0.34;

function getCurlScale(progress: number) {
  return CURL_SCALE_START - clamp(progress, 0, 1) * CURL_SCALE_DELTA;
}

function getVisualEdgeX(foldX: number, foldW: number, progress: number) {
  return foldX - foldW * getCurlScale(progress);
}

function getProgressByVisualEdge(edgeX: number, width: number) {
  if (width <= 0) {
    return 0;
  }

  const target = clamp(edgeX / width, 0, 1);
  const a = CURL_SCALE_DELTA;
  const b = -(1 + CURL_SCALE_START);
  const c = 1 - target;
  const d = Math.max(0, b * b - 4 * a * c);

  return clamp((-b - Math.sqrt(d)) / (2 * a), 0, 1);
}

function getSimulationVisualState(
  width: number,
  progress: number,
  direction: Exclude<DragDir, null>,
) {
  const clampedProgress = clamp(progress, 0, 1);
  const visualProgress = direction === "left" ? clampedProgress : 1 - clampedProgress;
  const forwardVisualState = getForwardSimulationVisualState(width, visualProgress);

  if (direction === "left") {
    return forwardVisualState;
  }

  return {
    ...forwardVisualState,
    revealClipPath: forwardVisualState.currentClipPath,
    currentClipPath: forwardVisualState.revealClipPath,
  };
}

function getForwardSimulationVisualState(width: number, progress: number) {
  const clampedProgress = clamp(progress, 0, 1);
  const foldX = width * (1 - clampedProgress);
  const foldW = width - foldX;
  const curlScale = getCurlScale(clampedProgress);
  const edgeX = getVisualEdgeX(foldX, foldW, clampedProgress);
  const curlWidth = Math.max(0, foldX - edgeX);
  const exitOffset = CURL_EXIT_OVERSHOOT * clampedProgress ** 3;
  const curlLeft = edgeX - exitOffset;
  const shadowLeft = foldX - 12 - exitOffset;
  const shadowOpacity =
    clampedProgress > 0
      ? Math.min(
          SIMULATION_SHADOW_MAX_OPACITY,
          0.12 + clampedProgress * SIMULATION_SHADOW_MAX_OPACITY,
        )
      : 0;

  return {
    foldX,
    foldW,
    curlWidth,
    curlLeft,
    curlContentOffset: width * curlScale,
    curlOpacity: clampedProgress > 0 ? 1 : 0,
    shadowLeft,
    shadowOpacity,
    visualProgress: clampedProgress,
    revealClipPath: `inset(0 0 0 ${foldX}px)`,
    currentClipPath: `inset(0 ${foldW}px 0 0)`,
  };
}

function updateSimulationPointer(event: MouseEvent | TouchEvent) {
  if (props.mode !== "simulation") {
    simulationPointerX.value = null;
    rawPointerX = 0;
    return;
  }

  const el = containerRef.value;
  if (!el) {
    simulationPointerX.value = null;
    rawPointerX = 0;
    return;
  }

  const rect = el.getBoundingClientRect();
  // 拖拽热路径：只写 plain 变量，不触发 Vue 响应式
  rawPointerX = clamp(getClientX(event) - rect.left, 0, rect.width);
}

function getSimulationProgressByPointer(
  pointerX: number,
  width: number,
  direction: Exclude<DragDir, null>,
) {
  const edgeX =
    direction === "left" ? clamp(pointerX, 0, width) : clamp(width - pointerX, 0, width);
  return getProgressByVisualEdge(edgeX, width);
}

/**
 * 在 rAF 回调中直接写 DOM style，完全绕过 Vue 响应式，
 * 是低端机上仿真翻页流畅度的关键优化路径。
 */
function applySimulationDragStyles() {
  pendingRafId = 0;
  const width = cachedContainerWidth || getContainerWidth();
  if (width <= 0) {
    return;
  }
  const direction = rawDragDir;
  const pointerX = rawPointerX;
  const progress = getSimulationProgressByPointer(pointerX, width, direction);
  const visualState = getSimulationVisualState(width, progress, direction);

  const revealEl = simulationRevealEl.value;
  const currentEl = simulationCurrentEl.value;
  const curlEl = simulationCurlEl.value;
  const curlContentEl = simulationCurlContentEl.value;
  const shadowEl = simulationShadowEl.value;
  if (!revealEl || !currentEl || !curlEl || !curlContentEl || !shadowEl) {
    return;
  }

  revealEl.style.clipPath = visualState.revealClipPath;
  currentEl.style.clipPath = visualState.currentClipPath;
  curlEl.style.left = `${visualState.curlLeft}px`;
  curlEl.style.width = `${visualState.curlWidth}px`;
  curlEl.style.opacity = `${visualState.curlOpacity}`;
  curlContentEl.style.width = `${width}px`;
  curlContentEl.style.opacity = "1";
  curlContentEl.style.left = `${visualState.curlContentOffset}px`;
  curlContentEl.style.right = "";
  curlContentEl.style.transform = `scaleX(${-getCurlScale(visualState.visualProgress)})`;
  curlContentEl.style.transformOrigin = "left center";

  shadowEl.style.left = `${visualState.shadowLeft}px`;
  shadowEl.style.opacity = `${visualState.shadowOpacity}`;
}

function scheduleSimulationFrame() {
  if (pendingRafId === 0) {
    pendingRafId = requestAnimationFrame(applySimulationDragStyles);
  }
}

/** 清除直接 DOM 写入的 simulation 拖拽 inline style（交还给 Vue 响应式 / snap CSS transition） */
function clearSimulationDragStyles() {
  if (pendingRafId !== 0) {
    cancelAnimationFrame(pendingRafId);
    pendingRafId = 0;
  }
  rawPointerX = 0;
  const revealEl = simulationRevealEl.value;
  const currentEl = simulationCurrentEl.value;
  const curlEl = simulationCurlEl.value;
  const curlContentEl = simulationCurlContentEl.value;
  const shadowEl = simulationShadowEl.value;
  if (revealEl) {
    revealEl.style.cssText = "";
  }
  if (currentEl) {
    currentEl.style.cssText = "";
  }
  if (curlEl) {
    curlEl.style.cssText = "";
  }
  if (curlContentEl) {
    curlContentEl.style.cssText = "";
  }
  if (shadowEl) {
    shadowEl.style.cssText = "";
  }
}

function syncSimulationOffsetFromPointer(direction: Exclude<DragDir, null>) {
  if (props.mode !== "simulation") {
    simulationPointerX.value = null;
    rawPointerX = 0;
    return;
  }

  // 取消未执行的 rAF，避免 snap 开始后还有一帧直接 DOM 写入覆盖 Vue 设置的值
  if (pendingRafId !== 0) {
    cancelAnimationFrame(pendingRafId);
    pendingRafId = 0;
  }

  const width = cachedContainerWidth || getContainerWidth();
  const pointerX = rawPointerX;
  // 先清除直接 DOM 写入，让 snap CSS transition 从 Vue 设置的 inline style 起始
  clearSimulationDragStyles();
  if (width <= 0 || pointerX === 0) {
    simulationPointerX.value = null;
    return;
  }

  const progress = getSimulationProgressByPointer(pointerX, width, direction);
  layerDragOffset.value = (direction === "left" ? -1 : 1) * width * progress;
  simulationPointerX.value = null;
}

const simulationState = computed(() => {
  const width = getContainerWidth();
  const direction: Exclude<DragDir, null> = dragDir.value === "right" ? "right" : "left";
  const offset = isAnimating.value ? layerSnapTarget.value : layerDragOffset.value;
  const progress =
    width <= 0
      ? 0
      : simulationPointerX.value !== null && dragDir.value !== null && !isAnimating.value
        ? getSimulationProgressByPointer(simulationPointerX.value, width, direction)
        : clamp(Math.abs(offset) / width, 0, 1);
  const visualState = getSimulationVisualState(width, progress, direction);
  const curlScale = getCurlScale(visualState.visualProgress);

  return {
    direction,
    stageDirection: "left",
    revealHtml: direction === "left" ? nextPageHtml.value : prevPageHtml.value,
    curlHtml: direction === "left" ? currentPageHtml.value : prevPageHtml.value,
    revealStyle: {
      clipPath: visualState.revealClipPath,
    },
    currentStyle: {
      clipPath: visualState.currentClipPath,
    },
    curlStyle: {
      left: `${visualState.curlLeft}px`,
      width: `${visualState.curlWidth}px`,
      opacity: `${visualState.curlOpacity}`,
    },
    curlContentStyle: {
      left: `${visualState.curlContentOffset}px`,
      width: `${width}px`,
      opacity: "1",
      transform: `scaleX(${-curlScale})`,
      transformOrigin: "left center",
    },
    shadowStyle: {
      left: `${visualState.shadowLeft}px`,
      opacity: `${visualState.shadowOpacity}`,
    },
  };
});

/* ---- Pointer / Touch ---- */
let dragging = false;
let startX = 0;
let startY = 0;
let startTime = 0;
let hasMoved = false;
let directionLocked = false;
let isHorizontal = false;
let animationTapCandidate = false;

const VELOCITY_THRESHOLD = 0.3;
const DISTANCE_RATIO = 0.3;

function resetVisualState() {
  animationTapCandidate = false;
  slideDragOffset.value = 0;
  slideSnapOffset.value = 0;
  layerDragOffset.value = 0;
  layerSnapTarget.value = 0;
  dragDir.value = null;
  simulationPointerX.value = null;
  simulationTransitionArmed.value = false;
  // 清除直接 DOM 写入的拖拽样式
  clearSimulationDragStyles();
}

function getClientX(e: MouseEvent | TouchEvent): number {
  if ("touches" in e && e.touches.length > 0) {
    return e.touches[0].clientX;
  }
  if ("changedTouches" in e && e.changedTouches.length > 0) {
    return e.changedTouches[0].clientX;
  }
  return (e as MouseEvent).clientX;
}

function getClientY(e: MouseEvent | TouchEvent): number {
  if ("touches" in e && e.touches.length > 0) {
    return e.touches[0].clientY;
  }
  if ("changedTouches" in e && e.changedTouches.length > 0) {
    return e.changedTouches[0].clientY;
  }
  return (e as MouseEvent).clientY;
}

function onPointerDown(e: MouseEvent | TouchEvent) {
  if (props.busy || props.selectionMode) {
    return;
  }
  if (!("touches" in e) && "button" in e && e.button !== 0) {
    return;
  }
  // 过滤移动端触摸后浏览器自动生成的合成 mousedown（ghost click 问题）
  // 真实触摸事件：TouchEvent（含 touches 属性）或 PointerEvent 且 pointerType='touch'
  // 合成鼠标事件：MouseEvent（不含 touches）且 pointerType 不为 'touch'
  const isTouch =
    "touches" in e || ("pointerType" in e && (e as PointerEvent).pointerType === "touch");
  if (isTouch) {
    lastTouchTime = Date.now();
  } else if (Date.now() - lastTouchTime < 600) {
    return; // 合成鼠标事件，忽略
  }

  if (isAnimating.value) {
    animationTapCandidate = true;
    hasMoved = false;
    directionLocked = false;
    isHorizontal = false;
    startX = getClientX(e);
    startY = getClientY(e);
    startTime = Date.now();
    return;
  }

  dragging = true;
  hasMoved = false;
  directionLocked = false;
  isHorizontal = false;
  startX = getClientX(e);
  startY = getClientY(e);
  startTime = Date.now();
  // 缓存容器宽度，避免拖拽热路径重复读取 layout
  cachedContainerWidth = containerRef.value?.clientWidth ?? window.innerWidth;
  resetVisualState();
  updateSimulationPointer(e);
  if ("pointerId" in e) {
    (e.currentTarget as HTMLElement | null)?.setPointerCapture?.((e as PointerEvent).pointerId);
  }
}

function onPointerMove(e: MouseEvent | TouchEvent) {
  if (props.selectionMode) {
    if (dragging) {
      dragging = false;
      resetVisualState();
    }
    return;
  }
  if (animationTapCandidate) {
    const dx = getClientX(e) - startX;
    const dy = getClientY(e) - startY;
    if (Math.hypot(dx, dy) > 10) {
      animationTapCandidate = false;
    }
    return;
  }
  if (!dragging || isAnimating.value) {
    return;
  }

  const x = getClientX(e);
  const y = getClientY(e);
  const dx = x - startX;
  const dy = y - startY;

  if (!directionLocked && (Math.abs(dx) > 5 || Math.abs(dy) > 5)) {
    directionLocked = true;
    isHorizontal = Math.abs(dx) > Math.abs(dy);
    if (isHorizontal && isLayeredMode.value) {
      dragDir.value = dx < 0 ? "left" : "right";
    }
  }

  if (props.mode === "none") {
    hasMoved = Math.abs(dx) > 10 || Math.abs(dy) > 10;
    if (isHorizontal && "cancelable" in e && e.cancelable) {
      e.preventDefault();
    }
    return;
  }

  if (!isHorizontal) {
    return;
  }

  if ("cancelable" in e && e.cancelable) {
    e.preventDefault();
  }

  hasMoved = true;
  updateSimulationPointer(e);
  const width = cachedContainerWidth || getContainerWidth();
  const atPrevBoundary = dx > 0 && !hasPrevPage.value && !props.hasPrevChapter;
  const atNextBoundary = dx < 0 && !hasNextPage.value && !props.hasNextChapter;
  const offset =
    atPrevBoundary || atNextBoundary ? dx * 0.2 : Math.max(-width, Math.min(width, dx));

  if (props.mode === "slide") {
    slideDragOffset.value = offset;
    return;
  }

  // simulation 模式：用 rAF 直接写 DOM，不走 Vue 响应式
  if (props.mode === "simulation") {
    rawDragDir = dx < 0 ? "left" : "right";
    scheduleSimulationFrame();
    return;
  }

  layerDragOffset.value = offset;
}

function handleTap(e: MouseEvent | TouchEvent) {
  const el = containerRef.value;
  if (!el || props.busy) {
    return;
  }
  if (emitParagraphCommentClick(e.target)) {
    if ("cancelable" in e && e.cancelable) {
      e.preventDefault();
    }
    return;
  }
  const selection = window.getSelection();
  if (selection && !selection.isCollapsed && selection.toString().trim()) {
    return;
  }

  const rect = el.getBoundingClientRect();
  const relX = (getClientX(e) - rect.left) / rect.width;

  if (relX < tapMenuStart.value) {
    queueAction(leftTapAction.value);
    return;
  }

  if (relX > tapMenuEnd.value) {
    queueAction(rightTapAction.value);
    return;
  }

  emit("tap", "center");
}

function emitParagraphCommentClick(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }
  const button = target.closest<HTMLElement>(".reader-paragraph-comment");
  if (!button) {
    return false;
  }
  const rangeKey = button.dataset.paragraphCommentKey;
  const start = Number.parseInt(button.dataset.paragraphCommentStart ?? "", 10);
  const end = Number.parseInt(button.dataset.paragraphCommentEnd ?? "", 10);
  const count = Number.parseInt(button.dataset.paragraphCommentCount ?? "", 10);
  const paragraphIndex = Number.parseInt(button.dataset.paragraphCommentParagraphIndex ?? "", 10);
  if (
    !rangeKey ||
    !Number.isFinite(start) ||
    !Number.isFinite(end) ||
    !Number.isFinite(count) ||
    !Number.isFinite(paragraphIndex)
  ) {
    return true;
  }
  emit("paragraph-comment-click", {
    key: rangeKey,
    start,
    end,
    count,
    ranges: [{ key: rangeKey, start, end, count }],
    paragraphIndex,
  });
  return true;
}

function onPointerUp(e: MouseEvent | TouchEvent) {
  if (props.selectionMode) {
    animationTapCandidate = false;
    dragging = false;
    resetVisualState();
    return;
  }
  if (animationTapCandidate) {
    const dx = getClientX(e) - startX;
    const dy = getClientY(e) - startY;
    animationTapCandidate = false;
    if (Math.hypot(dx, dy) <= 10) {
      handleTap(e);
    }
    return;
  }
  if (!dragging) {
    return;
  }

  dragging = false;
  if ("pointerId" in e) {
    (e.currentTarget as HTMLElement | null)?.releasePointerCapture?.((e as PointerEvent).pointerId);
  }

  if (!hasMoved) {
    simulationPointerX.value = null;
    handleTap(e);
    return;
  }

  if (!isHorizontal) {
    resetVisualState();
    return;
  }

  const dx = getClientX(e) - startX;
  const dy = getClientY(e) - startY;
  const duration = Date.now() - startTime;
  const velocity = Math.abs(dx) / Math.max(duration, 1);
  const shouldFlip =
    velocity > VELOCITY_THRESHOLD || Math.abs(dx) > getContainerWidth() * DISTANCE_RATIO;
  const direction: DragDir = dx < 0 ? "left" : "right";

  syncSimulationOffsetFromPointer(direction);

  if (!shouldFlip || Math.abs(dx) <= Math.abs(dy)) {
    bounceBack(direction);
    return;
  }

  if (direction === "left") {
    if (!canRunAction("next")) {
      showBoundary("已经到最后一页了");
      bounceBack(direction);
      return;
    }
    runAction("next");
    return;
  }

  if (!canRunAction("prev")) {
    showBoundary("已经到最前了");
    bounceBack(direction);
    return;
  }
  runAction("prev");
}

// ── TTS 高亮辅助 ─────────────────────────────────────────────────────────

/** 返回当前可见页内容的 DOM 容器 */
function getActivePageEl(): Element | null {
  const root = containerRef.value;
  if (!root) {
    return null;
  }
  if (props.mode === "slide") {
    // slide: 三列布局，中间列是当前页
    const track = root.querySelector(".paged-mode__slide-track");
    return track ? (track.children[1] ?? null) : null;
  }
  if (props.mode === "cover" || props.mode === "simulation") {
    return root.querySelector(".paged-mode__fg");
  }
  // none 模式
  return root.querySelector(".paged-mode__page--none");
}

/** 高亮当前页第 lineIdx 行（0-based），清除之前的高亮 */
function highlightLine(lineIdx: number): void {
  const root = containerRef.value;
  if (root) {
    root
      .querySelectorAll<HTMLElement>(".reader-line.tts-playing, .reader-block.tts-playing")
      .forEach((el) => el.classList.remove("tts-playing"));
  }

  const page = getActivePageEl();
  if (!page) {
    return;
  }

  const lines = page.querySelectorAll<HTMLElement>(".reader-line");
  lines[lineIdx]?.classList.add("tts-playing");
}

/** 高亮当前页第 paragraphIdx 个段落块（0-based），清除之前的高亮 */
function highlightParagraph(paragraphIdx: number): void {
  const root = containerRef.value;
  if (root) {
    root
      .querySelectorAll<HTMLElement>(".reader-line.tts-playing, .reader-block.tts-playing")
      .forEach((el) => el.classList.remove("tts-playing"));
  }

  const page = getActivePageEl();
  if (!page) {
    return;
  }

  const paragraphs = page.querySelectorAll<HTMLElement>(
    ".reader-block--paragraph, .reader-block--title",
  );
  paragraphs[paragraphIdx]?.classList.add("tts-playing");
}

/** 清除所有 TTS 高亮 */
function clearTtsHighlight(): void {
  containerRef.value
    ?.querySelectorAll<HTMLElement>(".reader-line.tts-playing, .reader-block.tts-playing")
    .forEach((el) => el.classList.remove("tts-playing"));
}

defineExpose({
  flipNext,
  flipPrev,
  goToPage,
  highlightLine,
  highlightParagraph,
  clearTtsHighlight,
  get currentPage() {
    return props.currentPage;
  },
  get totalPages() {
    return totalPages.value;
  },
});
</script>

<template>
  <div
    ref="containerRef"
    class="paged-mode"
    :class="`paged-mode--${mode}`"
    @mousedown="onPointerDown"
    @mousemove="onPointerMove"
    @mouseup="onPointerUp"
    @mouseleave="onPointerUp"
    @touchstart.passive="onPointerDown"
    @touchmove="onPointerMove"
    @touchend="onPointerUp"
    @touchcancel="onPointerUp"
  >
    <div
      v-if="mode !== 'none'"
      class="paged-mode__gesture"
      @click="totalPages <= 0 && emit('tap', 'center')"
    />

    <div v-else class="paged-mode__gesture paged-mode__gesture--none" />

    <template v-if="mode === 'slide'">
      <div
        class="paged-mode__slide-track"
        :class="{ 'paged-mode__slide-track--snapping': isAnimating }"
        :style="{ transform: `translateX(${slideTrackTranslateX}px)` }"
      >
        <div
          class="paged-mode__page"
          :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
          v-html="prevPageHtml"
        />
        <div
          class="paged-mode__page"
          :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
          v-html="currentPageHtml"
        />
        <div
          class="paged-mode__page"
          :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
          v-html="nextPageHtml"
        />
      </div>
    </template>

    <template v-else-if="mode === 'cover'">
      <div
        class="paged-mode__page paged-mode__bg"
        :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
        v-html="bgHtml"
      />
      <div
        class="paged-mode__page paged-mode__fg"
        :class="{
          'paged-mode__fg--snapping': isAnimating,
          'paged-mode__page--layout-debug': layoutDebug,
        }"
        :style="{ transform: `translateX(${coverTranslateX}px)` }"
        v-html="fgHtml"
      />
    </template>

    <template v-else-if="mode === 'simulation'">
      <div
        class="paged-mode__simulation-stage"
        :class="[
          `paged-mode__simulation-stage--${simulationState.stageDirection}`,
          {
            'paged-mode__simulation-stage--snapping': isAnimating && simulationTransitionArmed,
          },
        ]"
      >
        <div
          ref="simulationRevealEl"
          class="paged-mode__page paged-mode__bg paged-mode__simulation-reveal"
          :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
          :style="isAnimating ? simulationState.revealStyle : undefined"
          v-html="simulationState.revealHtml"
        />
        <div
          ref="simulationCurrentEl"
          class="paged-mode__page paged-mode__fg paged-mode__simulation-current"
          :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
          :style="isAnimating ? simulationState.currentStyle : undefined"
          v-html="currentPageHtml"
        />
        <div
          ref="simulationShadowEl"
          class="paged-mode__simulation-shadow"
          :style="isAnimating ? simulationState.shadowStyle : undefined"
        />
        <div
          ref="simulationCurlEl"
          class="paged-mode__simulation-curl"
          :style="isAnimating ? simulationState.curlStyle : undefined"
        >
          <div
            ref="simulationCurlContentEl"
            class="paged-mode__page paged-mode__simulation-curl-page"
            :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
            :style="isAnimating ? simulationState.curlContentStyle : undefined"
            v-html="simulationState.curlHtml"
          />
          <div class="paged-mode__simulation-curl-gloss" />
        </div>
      </div>
    </template>

    <template v-else>
      <div
        class="paged-mode__page paged-mode__page--none"
        :class="{ 'paged-mode__page--layout-debug': layoutDebug }"
        v-html="currentPageHtml"
      />
    </template>

    <div class="paged-mode__footer">
      <span v-if="pageInfo" class="paged-mode__page-info">{{ pageInfo }}</span>
    </div>

    <div v-if="showAnyDebug" class="paged-mode__debug">
      <div v-if="layoutDebug" class="paged-mode__debug-padding">
        <div class="paged-mode__debug-pad paged-mode__debug-pad--top" />
        <div class="paged-mode__debug-pad paged-mode__debug-pad--right" />
        <div class="paged-mode__debug-pad paged-mode__debug-pad--bottom" />
        <div class="paged-mode__debug-pad paged-mode__debug-pad--left" />
        <div class="paged-mode__debug-content-box" />
      </div>

      <div v-if="tapZoneDebug" class="paged-mode__debug-taps">
        <div
          class="paged-mode__debug-tap paged-mode__debug-tap--prev"
          :style="{ width: `${debugPrevWidth * 100}%` }"
        >
          <span>{{ leftTapLabel }}</span>
        </div>
        <div
          class="paged-mode__debug-tap paged-mode__debug-tap--center"
          :style="{ width: `${debugCenterWidth * 100}%` }"
        >
          <span>中区</span>
        </div>
        <div
          class="paged-mode__debug-tap paged-mode__debug-tap--next"
          :style="{ width: `${debugNextWidth * 100}%` }"
        >
          <span>{{ rightTapLabel }}</span>
        </div>
      </div>
    </div>

    <div v-if="totalPages <= 0 && !busy" class="paged-mode__empty">
      <span>正文排版中...</span>
    </div>

    <Transition name="boundary-toast">
      <div v-if="boundaryMsg" class="boundary-toast">{{ boundaryMsg }}</div>
    </Transition>

    <!-- 排版测量诊断指示器 -->
    <LayoutDebugIndicator :show="!!layoutDebug" :measurement="runtimeMeasurement" />

    <div v-if="busy && totalPages <= 0" class="paged-mode__busy">
      <n-spin size="large" />
    </div>
  </div>
</template>

<style scoped>
.paged-mode {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
  user-select: text;
  touch-action: pan-y;
  cursor: default;
}

.paged-mode--simulation {
  isolation: isolate;
}

.paged-mode__gesture {
  position: absolute;
  inset: 0;
  z-index: 10;
  pointer-events: none;
}

.paged-mode__gesture--none {
  z-index: 3;
}

.paged-mode__slide-track {
  display: flex;
  width: 300%;
  height: 100%;
  will-change: transform;
}

.paged-mode__slide-track--snapping {
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.paged-mode__page {
  position: absolute;
  inset: 0;
  box-sizing: border-box;
  padding: var(--reader-padding, 24px);
  overflow: hidden;
  background: var(--reader-bg-image, none), var(--reader-bg-color, var(--color-surface));
  background-size: var(--reader-bg-size, auto);
  background-position: var(--reader-bg-position, center);
  background-repeat: var(--reader-bg-repeat, no-repeat);
  background-attachment: var(--reader-bg-attachment, scroll);
  background-blend-mode: var(--reader-bg-blend-mode, normal);
}

.paged-mode__slide-track .paged-mode__page {
  position: static;
  width: calc(100% / 3);
  height: 100%;
  flex-shrink: 0;
}

.paged-mode__bg {
  z-index: 0;
}

.paged-mode__fg {
  z-index: 1;
  will-change: transform;
  box-shadow: 4px 0 20px rgba(0, 0, 0, 0.12);
}

.paged-mode__fg--snapping {
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.paged-mode__simulation-stage {
  position: absolute;
  inset: 0;
}

.paged-mode__simulation-reveal {
  z-index: 0;
  will-change: clip-path;
}

.paged-mode__simulation-current {
  z-index: 1;
  will-change: clip-path;
}

.paged-mode__simulation-curl {
  position: absolute;
  top: 0;
  left: 0;
  width: 0;
  height: 100%;
  z-index: 3;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
  border-radius: 0 0 18px 0;
  will-change: left, width, opacity;
  box-shadow:
    -8px 0 10px rgba(0, 0, 0, 0.08),
    inset 6px 0 8px rgba(0, 0, 0, 0.04);
}

.paged-mode__simulation-stage--right .paged-mode__simulation-curl {
  border-radius: 0 0 0 18px;
  box-shadow:
    8px 0 10px rgba(0, 0, 0, 0.08),
    inset -6px 0 8px rgba(0, 0, 0, 0.04);
}

.paged-mode__simulation-curl-page {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  color: var(--reader-text-color, var(--color-text-primary));
  will-change: transform;
}

.paged-mode__simulation-curl-gloss {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    linear-gradient(
      to right,
      rgba(255, 255, 255, 0.2) 0%,
      rgba(255, 255, 255, 0.05) 28%,
      rgba(0, 0, 0, 0.08) 100%
    ),
    linear-gradient(to left, rgba(0, 0, 0, 0.12) 0%, rgba(0, 0, 0, 0.04) 36%, rgba(0, 0, 0, 0) 100%);
}

.paged-mode__simulation-stage--right .paged-mode__simulation-curl-gloss {
  transform: scaleX(-1);
}

.paged-mode__simulation-shadow {
  position: absolute;
  top: 0;
  left: 0;
  width: 12px;
  height: 100%;
  z-index: 2;
  opacity: 0;
  pointer-events: none;
  will-change: left, opacity;
  background: linear-gradient(
    to left,
    rgba(0, 0, 0, 0.16) 0%,
    rgba(0, 0, 0, 0.08) 45%,
    rgba(0, 0, 0, 0) 100%
  );
}

.paged-mode__simulation-stage--right .paged-mode__simulation-shadow {
  transform: scaleX(-1);
}

.paged-mode__simulation-stage--snapping .paged-mode__simulation-reveal,
.paged-mode__simulation-stage--snapping .paged-mode__simulation-current,
.paged-mode__simulation-stage--snapping .paged-mode__simulation-curl,
.paged-mode__simulation-stage--snapping .paged-mode__simulation-shadow {
  transition:
    clip-path 0.36s cubic-bezier(0.2, 0.72, 0.18, 1),
    left 0.36s cubic-bezier(0.2, 0.72, 0.18, 1),
    width 0.36s cubic-bezier(0.2, 0.72, 0.18, 1),
    opacity 0.26s ease;
}

.paged-mode__simulation-stage--snapping .paged-mode__simulation-curl-page {
  transition:
    left 0.36s cubic-bezier(0.2, 0.72, 0.18, 1),
    right 0.36s cubic-bezier(0.2, 0.72, 0.18, 1),
    transform 0.36s cubic-bezier(0.2, 0.72, 0.18, 1),
    opacity 0.26s ease;
}

.paged-mode__page :deep(.reader-para) {
  margin: 0 0 var(--reader-paragraph-spacing, 12px) 0;
  padding: 0;
  font-family: var(--reader-font-family);
  font-size: var(--reader-font-size);
  line-height: var(--reader-line-height);
  letter-spacing: var(--reader-letter-spacing);
  word-spacing: var(--reader-word-spacing);
  font-weight: var(--reader-font-weight);
  font-style: var(--reader-font-style);
  text-align: var(--reader-text-align);
  text-decoration: var(--reader-text-decoration);
  font-variant: var(--reader-font-variant);
  -webkit-text-stroke-width: var(--reader-text-stroke-width);
  -webkit-text-stroke-color: var(--reader-text-stroke-color);
  text-shadow: var(--reader-text-shadow);
  color: var(--reader-text-color);
  word-break: break-all;
  overflow-wrap: break-word;
  font-synthesis: weight style;
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}

.paged-mode__page :deep(.reader-para:last-child) {
  margin-bottom: 0 !important;
}

.paged-mode__page :deep(.reader-paragraph-comment) {
  display: inline-flex;
  position: relative;
  align-items: center;
  justify-content: center;
  width: 2.2em;
  height: 1.45em;
  margin: 0 0.16em 0 0.38em;
  padding: 0;
  border: 1px solid currentColor;
  border-radius: 0.35em;
  background: color-mix(in srgb, var(--reader-bg-color, transparent) 82%, transparent);
  color: var(--reader-text-color);
  font: inherit;
  font-size: 0.68em;
  line-height: 1;
  vertical-align: 0.05em;
  opacity: 0.72;
  cursor: pointer;
}

.paged-mode__page :deep(.reader-paragraph-comment::after) {
  content: "";
  position: absolute;
  right: 0.34em;
  bottom: -0.25em;
  width: 0.4em;
  height: 0.4em;
  border-right: 1px solid currentColor;
  border-bottom: 1px solid currentColor;
  background: inherit;
  transform: rotate(45deg);
}

.paged-mode__page :deep(.reader-paragraph-comment:hover),
.paged-mode__page :deep(.reader-paragraph-comment:focus-visible) {
  opacity: 1;
}

.paged-mode__page :deep(.reader-paragraph-comment__count) {
  min-width: 1.2em;
  max-width: 100%;
  overflow: hidden;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.paged-mode__page :deep(.reader-paragraph-comment-row) {
  display: block;
}

/* TTS 当前行高亮：借用主题选区色，自动适配明暗主题 */
.paged-mode__page :deep(.reader-line.tts-playing),
.paged-mode__page :deep(.reader-block.tts-playing .reader-line) {
  background-color: var(--reader-tts-hl-bg, rgba(99, 226, 183, 0.2));
  border-radius: 3px;
  transition: background-color 0.2s ease;
}

/* 书签高亮 */
.paged-mode__page :deep(.reader-bookmark) {
  background-color: rgba(250, 204, 21, 0.4);
  border-radius: 2px;
}

.paged-mode__page--layout-debug :deep(.reader-block) {
  position: relative;
  box-shadow: inset 0 0 0 1px rgba(14, 165, 233, 0.9);
  background: rgba(14, 165, 233, 0.05);
}

.paged-mode__page--layout-debug :deep(.reader-page-fragments--manual-lines .reader-line) {
  box-shadow: inset 0 0 0 1px rgba(249, 115, 22, 0.18);
}

.paged-mode__page--layout-debug :deep(.reader-gap) {
  box-sizing: border-box;
  background: rgba(244, 63, 94, 0.34);
  border-top: 1px dashed rgba(244, 63, 94, 0.95);
  border-bottom: 1px dashed rgba(244, 63, 94, 0.95);
}

.paged-mode ::selection {
  background-color: var(--reader-selection-color);
}

.paged-mode__footer {
  position: absolute;
  bottom: 4px;
  left: 0;
  right: 0;
  text-align: center;
  pointer-events: none;
  z-index: 12;
}

.paged-mode__page-info {
  font-size: 0.6875rem;
  opacity: 0.35;
  color: var(--reader-text-color);
}

.paged-mode__empty {
  position: absolute;
  inset: 0;
  z-index: 11;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--reader-text-color);
  opacity: 0.55;
  pointer-events: none;
}

.paged-mode__busy {
  position: absolute;
  inset: 0;
  z-index: 13;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.08);
  backdrop-filter: blur(2px);
}

.paged-mode__debug {
  position: absolute;
  inset: 0;
  z-index: 9;
  pointer-events: none;
}

.paged-mode__debug-padding {
  position: absolute;
  inset: 0;
}

.paged-mode__debug-pad {
  position: absolute;
}

.paged-mode__debug-pad--top {
  left: 0;
  right: 0;
  top: 0;
  height: var(--reader-padding-top, 24px);
  background: rgba(239, 68, 68, 0.18);
}

.paged-mode__debug-pad--right {
  top: var(--reader-padding-top, 24px);
  right: 0;
  bottom: var(--reader-padding-bottom, 24px);
  width: var(--reader-padding-right, 24px);
  background: rgba(245, 158, 11, 0.18);
}

.paged-mode__debug-pad--bottom {
  left: 0;
  right: 0;
  bottom: 0;
  height: var(--reader-padding-bottom, 24px);
  background: rgba(168, 85, 247, 0.18);
}

.paged-mode__debug-pad--left {
  top: var(--reader-padding-top, 24px);
  left: 0;
  bottom: var(--reader-padding-bottom, 24px);
  width: var(--reader-padding-left, 24px);
  background: rgba(59, 130, 246, 0.18);
}

.paged-mode__debug-content-box,
.paged-mode__debug-content-box {
  position: absolute;
  top: var(--reader-padding-top, 24px);
  right: var(--reader-padding-right, 24px);
  bottom: var(--reader-padding-bottom, 24px);
  left: var(--reader-padding-left, 24px);
}

.paged-mode__debug-content-box {
  border: 1px dashed rgba(34, 197, 94, 0.95);
  background: rgba(34, 197, 94, 0.08);
}

.paged-mode__debug-taps {
  position: absolute;
  inset: 0;
  display: flex;
}

.paged-mode__debug-tap {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
}

.paged-mode__debug-tap--prev {
  background: rgba(59, 130, 246, 0.15);
}

.paged-mode__debug-tap--center {
  background: rgba(16, 185, 129, 0.12);
}

.paged-mode__debug-tap--next {
  background: rgba(249, 115, 22, 0.15);
}
</style>
