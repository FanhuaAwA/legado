/**
 * src/composables/useOverlay.ts — 权威的弹层 / 子页面返回栈集成 API
 *
 * 配合 src/stores/backStack.ts（id-based synthetic Set 模型）使用。
 *
 * 设计原则：
 *   1. 单一关闭路径：所有 UI 关闭最终通过 history.back() / history.go(-N) 触发 popstate；
 *   2. handler 必须幂等：onClose 重复调用必须无副作用（业务方一般通过 `if (show.value)` 守卫）；
 *   3. 模板里关闭按钮应优先使用 `triggerClose()`，而不是直接 `show = false`；
 *      两者结果等价，但前者显式表达"通过返回栈关闭"的意图，更易审计。
 *
 * 用法：
 *
 *   // 函数式（推荐用于已有 ref 的情况，与旧 useOverlayBackstack 完全兼容）
 *   const { triggerClose } = useOverlay(() => show.value, () => { show.value = false; });
 *
 *   // 对象式（适合命名清晰）
 *   const { triggerClose } = useOverlay({
 *     show: () => visible.value,
 *     close: () => { visible.value = false; },
 *   });
 *
 *   // 在模板中
 *   <button @click="triggerClose()">关闭</button>
 *
 * 这是项目内**唯一**允许的弹层返回栈接入方式。
 * 自定义全局 popstate / keydown 监听器、直接调 history.back/go/pushState 都视为反模式。
 */
import { onBeforeUnmount, watch } from "vue";

import { useBackStackStore } from "@/stores";

export type OverlayActiveSource = () => boolean;

export interface UseOverlayApi {
  /**
   * 模板层关闭按钮应该调用此函数，而不是直接 `show.value = false`。
   * 同步触发：
   *   - 立即把对应 entry 从返回栈中标记为已处理（synthetic）；
   *   - 异步消耗一条 history 记录（history.go(-1)），让 popstate 路径走完；
   *   - 同步调用 onClose（通常等价于 show.value = false）。
   * 即使 overlay 当前不活跃，也会直接调用一次 onClose（安全兜底）。
   */
  triggerClose: () => void;
  /**
   * 当 onClose 不实际关闭 overlay 时（如"最小化"行为）调用此函数，
   * 可在 backHandler 已被消耗但 isActive() 仍为 true 时重新向返回栈注册。
   */
  reactivate: () => void;
}

export interface UseOverlayOptions {
  show: OverlayActiveSource;
  close: () => void;
}

export function useOverlay(isActive: OverlayActiveSource, onClose: () => void): UseOverlayApi;
export function useOverlay(options: UseOverlayOptions): UseOverlayApi;
export function useOverlay(
  arg1: OverlayActiveSource | UseOverlayOptions,
  arg2?: () => void,
): UseOverlayApi {
  const isActive: OverlayActiveSource = typeof arg1 === "function" ? arg1 : arg1.show;
  const onClose: () => void = typeof arg1 === "function" ? (arg2 as () => void) : arg1.close;

  const backStack = useBackStackStore();
  let backHandler: (() => void) | null = null;

  function activate() {
    if (backHandler) {
      return;
    }
    backHandler = () => {
      backHandler = null;
      onClose();
    };
    backStack.push(backHandler);
  }

  function deactivate(options?: { consume?: boolean }) {
    if (!backHandler) {
      return;
    }
    const handler = backHandler;
    backHandler = null;
    if (options?.consume === false) {
      backStack.detach(handler);
    } else {
      backStack.remove(handler);
    }
  }

  watch(
    isActive,
    (active) => {
      if (active) {
        activate();
      } else {
        deactivate();
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    deactivate({ consume: false });
  });

  function triggerClose() {
    if (!backHandler) {
      // 已经不活跃；直接调用一次 onClose 作为安全兜底
      onClose();
      return;
    }
    const handler = backHandler;
    backHandler = null;
    backStack.remove(handler);
    onClose();
  }

  function reactivate() {
    if (isActive()) {
      activate();
    }
  }

  return { triggerClose, reactivate };
}
