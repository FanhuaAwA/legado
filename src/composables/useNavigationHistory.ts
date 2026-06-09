/**
 * src/composables/useNavigationHistory.ts — 动态多级导航返回栈集成
 *
 * 用于在单个 composable / 组件内管理多层子页面导航（如阅读器设置子页面）。
 * 每次调用 push() 都向返回栈注册一个 handler，逻辑上等价于多次调用 useOverlay；
 * 硬件返回键按 LIFO 顺序依次弹出各层。
 *
 * 与 useOverlay 的区别：
 *   useOverlay 适合"show/hide"双态 overlay（一次 push / 一次 remove）；
 *   useNavigationHistory 适合"用户连续导航到多级子页面"场景（动态 push 次数）。
 *
 * 用法：
 *   const nav = useNavigationHistory();
 *
 *   function openSubPage(page: SubPage) {
 *     currentPage.value = page;
 *     nav.push(() => { currentPage.value = 'none'; });
 *   }
 *
 *   // UI 返回按钮：
 *   function goBack() { nav.pop(); }
 */
import { onBeforeUnmount } from "vue";

import { useBackStackStore } from "@/stores";

interface HistoryEntry {
  handler: () => void;
  onBack: () => void;
}

export interface NavigationHistoryApi {
  /**
   * 向返回栈压入一层；onBack 在硬件返回时自动调用。
   * 返回一个 triggerClose 函数，供 UI 按钮调用（等价于 useOverlay.triggerClose）。
   */
  push: (onBack: () => void) => () => void;
  /**
   * UI 主动弹出最顶层（等价于调用最近一次 push 返回的 triggerClose）。
   * 适合统一的"返回"按钮入口，不需要保存每层的 triggerClose 引用。
   */
  pop: () => void;
  /** 当前压入的层数 */
  depth: () => number;
}

export function useNavigationHistory(): NavigationHistoryApi {
  const backStack = useBackStackStore();
  const _entries: HistoryEntry[] = [];

  function push(onBack: () => void): () => void {
    const handler = () => {
      // 硬件返回触发：从 _entries 移除并执行 onBack
      const idx = _entries.findIndex((e) => e.handler === handler);
      if (idx >= 0) {
        _entries.splice(idx, 1);
      }
      onBack();
    };

    const entry: HistoryEntry = { handler, onBack };
    _entries.push(entry);
    backStack.push(handler);

    // 返回 triggerClose（UI 主动关闭路径）
    return () => {
      const idx = _entries.findIndex((e) => e.handler === handler);
      if (idx >= 0) {
        _entries.splice(idx, 1);
      }
      backStack.remove(handler); // 标记 synthetic + 消耗 history 记录
      onBack(); // 同步执行 UI 更新
    };
  }

  function pop() {
    if (_entries.length === 0) {
      return;
    }
    const entry = _entries.pop()!;
    backStack.remove(entry.handler); // 标记 synthetic + 消耗 history 记录
    entry.onBack(); // 同步执行 UI 更新
  }

  function depth() {
    return _entries.length;
  }

  onBeforeUnmount(() => {
    // 组件卸载时清理所有残留 handler，但不消耗 history（交给浏览器自然回收）
    for (const entry of _entries) {
      backStack.detach(entry.handler);
    }
    _entries.length = 0;
  });

  return { push, pop, depth };
}
