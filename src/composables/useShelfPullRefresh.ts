/**
 * useShelfPullRefresh — 书架下拉刷新逻辑
 *
 * 下拉时触发刷新回调，显示视觉反馈（指示器动画），
 * 刷新完成后自动隐藏指示器。
 */

import { ref, computed, onBeforeUnmount } from "vue";

export interface UseShelfPullRefreshOptions {
  /** 刷新回调，返回 Promise（刷新完成时 resolve） */
  onRefresh: () => Promise<void>;
}

export function useShelfPullRefresh(options: UseShelfPullRefreshOptions) {
  const { onRefresh } = options;

  // ── 常量 ──────────────────────────────────────────────────────────
  /** 下拉触发阈值（px） */
  const PULL_THRESHOLD = 60;
  /** 阻尼系数 */
  const PULL_DAMPING = 0.35;
  /** 最大下拉距离（px） */
  const MAX_PULL = 120;

  // ── 状态 ──────────────────────────────────────────────────────────
  const pullDistance = ref(0);
  const isRefreshing = ref(false);
  const isReady = ref(false);
  /** 主动拖动中（用于模板禁用 CSS transition，避免拖动时抖动） */
  const isActivePulling = ref(false);

  // ── rAF 节流 ─────────────────────────────────────────────────────
  // touchmove / mousemove 高频触发，直接写 Vue ref 会每帧触发完整响应式更新
  // 改为攒到下一帧统一提交，降低 Vue 更新频率
  let rafId: number | null = null;
  let pendingDistance = 0;
  let pendingReady = false;

  function scheduleUpdate(distance: number, ready: boolean) {
    pendingDistance = distance;
    pendingReady = ready;
    if (rafId === null) {
      rafId = requestAnimationFrame(() => {
        rafId = null;
        pullDistance.value = pendingDistance;
        isReady.value = pendingReady;
      });
    }
  }

  function cancelPendingRaf() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  }

  /** 同步刷新待提交的状态，onTouchEnd 前必须先调用，避免 rAF 竞态导致 isReady 读取时进 false */
  function flushPendingUpdate() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
      pullDistance.value = pendingDistance;
      isReady.value = pendingReady;
    }
  }

  // ── 触摸事件 ──────────────────────────────────────────────────────
  let touchStartY = 0;
  let touchContainer: HTMLElement | null = null;
  let isPulling = false;

  function onTouchStart(e: TouchEvent) {
    touchStartY = e.touches[0].clientY;
    touchContainer = e.currentTarget as HTMLElement;
    isPulling = false;
    isActivePulling.value = false;
    cancelPendingRaf();
    pullDistance.value = 0;
    isReady.value = false;
  }

  function onTouchMove(e: TouchEvent) {
    if (!touchContainer) {
      return;
    }

    const currentY = e.touches[0].clientY;
    const dy = currentY - touchStartY;

    // 如果还没开始下拉，且不是向下滑动，或不在顶部，则忽略
    if (!isPulling) {
      if (dy <= 0 || touchContainer.scrollTop > 0) {
        return;
      }
      // 开始下拉
      isPulling = true;
      isActivePulling.value = true;
    } else {
      // 正在下拉中，如果向上滑动则停止下拉状态
      if (dy <= 0) {
        isPulling = false;
        isActivePulling.value = false;
        cancelPendingRaf();
        pullDistance.value = 0;
        isReady.value = false;
        return;
      }
    }

    // 阻止浏览器默认滚动行为
    e.preventDefault();

    // 应用阻尼，通过 rAF 节流写入 ref，避免每帧触发 Vue 响应式更新
    const d = Math.min(dy * PULL_DAMPING, MAX_PULL);
    scheduleUpdate(d, d >= PULL_THRESHOLD);
  }

  function onTouchEnd() {
    flushPendingUpdate();
    const shouldRefresh = isReady.value && !isRefreshing.value;
    pullDistance.value = 0;
    isReady.value = false;
    isPulling = false;
    isActivePulling.value = false;
    touchContainer = null;

    if (shouldRefresh) {
      isRefreshing.value = true;
      onRefresh().finally(() => {
        isRefreshing.value = false;
      });
    }
  }

  // ── 鼠标拖拽（桌面端） ──────────────────────────────────────────
  let isDragging = false;
  let mouseStartY = 0;

  function onMouseDown(e: MouseEvent) {
    // 只有在滚动到顶部时才响应
    const el = e.currentTarget as HTMLElement;
    if (el.scrollTop > 0) {
      return;
    }
    isDragging = true;
    isActivePulling.value = true;
    mouseStartY = e.clientY;
    cancelPendingRaf();
    pullDistance.value = 0;
    isReady.value = false;

    document.addEventListener("mousemove", onGlobalMouseMove);
    document.addEventListener("mouseup", onGlobalMouseUp, { once: true });
  }

  function onGlobalMouseMove(e: MouseEvent) {
    if (!isDragging) {
      return;
    }

    const dy = e.clientY - mouseStartY;
    if (dy <= 0) {
      scheduleUpdate(0, false);
      return;
    }

    const d = Math.min(dy * PULL_DAMPING, MAX_PULL);
    scheduleUpdate(d, d >= PULL_THRESHOLD);
  }

  function onGlobalMouseUp() {
    isDragging = false;
    isActivePulling.value = false;
    flushPendingUpdate();
    document.removeEventListener("mousemove", onGlobalMouseMove);

    if (isReady.value && !isRefreshing.value) {
      pullDistance.value = 0;
      isReady.value = false;
      isRefreshing.value = true;
      onRefresh().finally(() => {
        isRefreshing.value = false;
      });
    } else {
      pullDistance.value = 0;
      isReady.value = false;
    }
  }

  // ── 计算属性 ─────────────────────────────────────────────────────
  const pullProgress = computed(() => Math.min(pullDistance.value / PULL_THRESHOLD, 1));

  // ── 清理 ──────────────────────────────────────────────────────────
  onBeforeUnmount(() => {
    cancelPendingRaf();
    document.removeEventListener("mousemove", onGlobalMouseMove);
  });

  return {
    pullDistance,
    pullProgress,
    isRefreshing,
    isReady,
    isActivePulling,
    PULL_THRESHOLD,
    MAX_PULL,
    onTouchStart,
    onTouchMove,
    onTouchEnd,
    onMouseDown,
  };
}
