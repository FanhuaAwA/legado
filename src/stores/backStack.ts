/**
 * src/stores/backStack.ts — 全局返回键堆栈（v2，基于 id 标记）
 *
 * 统一管理 Android 硬件返回键（popstate）和键盘 Escape / BrowserBack / Backspace。
 *
 * 设计契约：
 *   - 每个 push 分配一个唯一 id，并写入 `history.state._legadoId`；
 *   - 关闭路径只有一种：popstate → 弹栈 → 调用 handler；
 *   - UI 主动关闭（点 ✕、点遮罩、Esc）通过 `history.go(-N)` 触发 popstate，
 *     remove() 会先把对应 entry 从内存栈移除，再记录一次 pending synthetic
 *     navigation；popstate 回来时只清账，不调用 handler（因为外部已经把
 *     show=false，不需要再调一次）；
 *   - 真实硬件返回 / 系统手势：popstate 中目标 id 不在 synthetic 集 → 调 handler；
 *   - **不再使用全局 _skip 计数器**：旧实现的 _skip 在 history.go(-1) 未触发
 *     popstate（如根历史栈、部分 Android WebView）时会永久淤积，
 *     吃掉后续真实返回事件，导致"时灵时不灵"。
 *
 * 调用约定（handler 必须是幂等的）：
 *   const backStack = useBackStackStore();
 *
 *   // 组件打开时注册
 *   const handler = () => closeMyOverlay();   // 多次调用必须无副作用
 *   backStack.push(handler);
 *
 *   // 组件通过 UI 按钮关闭时（show=false 已经设置）：
 *   backStack.remove(handler);  // 消耗对应 history 记录，不重复调 handler
 *
 *   // 组件卸载等场景，仅清理栈，不动 history：
 *   backStack.detach(handler);
 */
import { defineStore } from "pinia";

export type BackHandler = () => void;

interface Entry {
  id: number;
  handler: BackHandler;
}

export const useBackStackStore = defineStore("backStack", () => {
  const _stack: Entry[] = [];
  /** UI 主动关闭已经走过 onClose 的 entry id；popstate 命中时跳过 handler 调用 */
  const _synthetic = new Set<number>();
  let _pendingSyntheticNavigations = 0;
  let _pendingSyntheticResetTimer: ReturnType<typeof setTimeout> | null = null;
  let _seq = 0;

  function _currentHistoryId(): number | undefined {
    return (history.state as { _legadoId?: number } | null)?._legadoId;
  }

  function _realignHistoryToTop(): void {
    const top = _stack.at(-1);
    if (!top || _currentHistoryId() === top.id) {
      return;
    }
    try {
      history.pushState({ _legadoId: top.id }, "");
    } catch {
      // 极少数受限环境下 pushState 不可用，退化为仅栈管理
    }
  }

  function _clearSyntheticResetTimer(): void {
    if (_pendingSyntheticResetTimer === null) {
      return;
    }
    clearTimeout(_pendingSyntheticResetTimer);
    _pendingSyntheticResetTimer = null;
  }

  function _scheduleSyntheticReset(): void {
    _clearSyntheticResetTimer();
    _pendingSyntheticResetTimer = setTimeout(() => {
      _pendingSyntheticResetTimer = null;
      if (_pendingSyntheticNavigations === 0) {
        return;
      }
      _pendingSyntheticNavigations = 0;
      _synthetic.clear();
      _realignHistoryToTop();
    }, 1200);
  }

  function _consumeSyntheticPopState(): boolean {
    if (_pendingSyntheticNavigations > 0) {
      _pendingSyntheticNavigations -= 1;
      if (_pendingSyntheticNavigations === 0) {
        _synthetic.clear();
        _clearSyntheticResetTimer();
      }
      _realignHistoryToTop();
      return true;
    }

    const landedId = _currentHistoryId();
    if (landedId === undefined || !_synthetic.delete(landedId)) {
      return false;
    }

    _realignHistoryToTop();
    return true;
  }

  function _findIndex(handler: BackHandler): number {
    for (let i = _stack.length - 1; i >= 0; i--) {
      if (_stack[i].handler === handler) {
        return i;
      }
    }
    return -1;
  }

  /**
   * 注册一个返回处理器，并向浏览器历史压入一条记录（用于硬件/系统返回）。
   */
  function push(handler: BackHandler): void {
    const id = ++_seq;
    _stack.push({ id, handler });
    try {
      history.pushState({ _legadoId: id }, "");
    } catch {
      // 极少数受限环境下 pushState 不可用，退化为仅栈管理
    }
  }

  /**
   * 标记 handler（及其上方所有 entry）为 synthetic，并通过 history.go(-N)
   * 触发 popstate 来真正清理。popstate 命中标记 id 时跳过 handler。
   *
   * 适用于"组件通过 UI 按钮关闭"——外部已把 show=false，不需要再调 handler。
   */
  function remove(handler: BackHandler): void {
    const idx = _findIndex(handler);
    if (idx < 0) {
      return;
    }
    const tail = _stack.splice(idx);
    for (const e of tail) {
      _synthetic.add(e.id);
    }
    const steps = tail.length;
    try {
      _pendingSyntheticNavigations += 1;
      _scheduleSyntheticReset();
      history.go(-steps);
    } catch {
      _pendingSyntheticNavigations = Math.max(0, _pendingSyntheticNavigations - 1);
      for (const e of tail) {
        _synthetic.delete(e.id);
      }
      if (_pendingSyntheticNavigations === 0) {
        _clearSyntheticResetTimer();
      }
    }
  }

  /**
   * 仅从堆栈中移除 handler，不消耗 history 记录。
   * 适用于组件卸载、或历史记录已被外部手段消耗时的清理。
   * 残留的 history 记录会在下一次 popstate 时被吸收为 no-op（栈为空 → 返回 false）。
   */
  function detach(handler: BackHandler): void {
    const idx = _findIndex(handler);
    if (idx >= 0) {
      _synthetic.delete(_stack[idx].id);
      _stack.splice(idx, 1);
    }
  }

  /**
   * 由全局 popstate 监听器调用。返回 true 表示已由堆栈处理。
   *
   * 行为：
   *   1. 栈为空 → 返回 false，交由上层（App.vue handleGlobalDismiss）决定是否退出/切换 tab；
   *   2. 弹出栈顶 entry：
   *      - 若 id 在 synthetic 集中：这是 UI 主动关闭路径触发的 history.go，
   *        外部已经调用过 onClose，不重复触发 handler；
   *      - 否则视为真实返回，调用 handler（设 show=false 等）。
   */
  function onPopState(): boolean {
    if (_consumeSyntheticPopState()) {
      return true;
    }

    if (_stack.length === 0) {
      // 多个 overlay 在同一 tick 内级联关闭时，每个 remove() 都会标记 synthetic
      // 并调 history.go(-1)，但浏览器可能把多次 go(-1) 合并为一次 popstate；
      // 此时 _stack 已被清空，_synthetic 里的孤儿 id 永远不会被 delete →
      // 形成无界增长。直接清空兜底，等价于"历史已追平栈"。
      if (_synthetic.size > 0) {
        _synthetic.clear();
      }
      return false;
    }
    const entry = _stack.pop()!;
    if (_synthetic.delete(entry.id)) {
      return true;
    }
    try {
      entry.handler();
    } catch (e) {
      console.error("[backStack] handler error", e);
    }
    return true;
  }

  /**
   * 由键盘 Escape / BrowserBack / Backspace 路径调用（via handleGlobalBack）。
   * 走原生 history.back()，让 popstate 作为单一关闭入口处理，避免双路径竞态。
   */
  function onKeyBack(): boolean {
    if (_stack.length === 0) {
      return false;
    }
    try {
      history.back();
    } catch {
      // fallback：直接走 onPopState
      onPopState();
    }
    return true;
  }

  return { push, remove, detach, onPopState, onKeyBack };
});
