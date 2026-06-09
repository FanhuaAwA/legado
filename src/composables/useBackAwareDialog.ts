/**
 * src/composables/useBackAwareDialog.ts — Naive UI 程序化弹窗的返回栈包装
 *
 * 背景：
 *   `useDialog()` 返回的 `dialog.create / warning / info / success / error`
 *   会创建独立挂载的 `n-dialog`，**不会**经过 `<n-modal>` 的关闭流程，因此
 *   `useOverlay` 兜不住，Android 硬件返回键 / 系统手势返回 默认无法关闭它们。
 *
 * 解决方案：
 *   提供一个 drop-in 替代 `useDialog()`，每次调用 dialog API 时同步在返回栈
 *   `useBackStackStore` 中注册一个 handler；handler 触发时销毁实例并尝试执行
 *   用户的 `onNegativeClick`（作为天然的"取消"语义）。用户主动关闭（点按钮 / 点
 *   遮罩 / Esc）时通过 `onAfterLeave` 反向同步消耗对应 history 记录。
 *
 * 用法：
 *   import { useBackAwareDialog as useDialog } from '@/composables/useBackAwareDialog';
 *   const dialog = useDialog();
 *   dialog.warning({ title: '...', content: '...', ... });
 */
import { type DialogOptions, type DialogReactive, useDialog as useNaiveDialog } from "naive-ui";

import { useBackStackStore } from "@/stores";
import { log } from "@/utils/logger";

type DialogApi = ReturnType<typeof useNaiveDialog>;
type DialogMethod = "create" | "info" | "success" | "warning" | "error";

const METHODS: DialogMethod[] = ["create", "info", "success", "warning", "error"];

export function useBackAwareDialog(): DialogApi {
  const base = useNaiveDialog();
  const backStack = useBackStackStore();

  function wrap(method: DialogMethod, options: DialogOptions): DialogReactive {
    let active = true;
    let instance: DialogReactive | null = null;

    const handler = () => {
      if (!active) {
        return;
      }
      active = false;
      // 硬件返回：直接销毁实例 + 调用原始 onNegativeClick（语义化的"取消"）。
      try {
        const result = options.onNegativeClick?.(new MouseEvent("click"));
        // 用户可能返回 Promise；这里不阻塞，让 Naive UI 自己处理。
        void result;
      } catch (e) {
        log.error("Dialog", e instanceof Error ? e : String(e));
      }
      try {
        instance?.destroy();
      } catch {
        // 实例可能已销毁
      }
    };

    const userOnAfterLeave = options.onAfterLeave;

    instance = base[method]({
      ...options,
      onAfterLeave: (...args: unknown[]) => {
        if (active) {
          active = false;
          // 用户主动关闭（按钮 / 遮罩 / Esc）→ 同步消耗 history 记录。
          backStack.remove(handler);
        }
        return (userOnAfterLeave as ((...a: unknown[]) => unknown) | undefined)?.(...args);
      },
    });

    backStack.push(handler);
    return instance;
  }

  // 用 Proxy 透明替换 dialog API 上的方法，其它属性（如 destroyAll）原样转发。
  return new Proxy(base, {
    get(target, prop, receiver) {
      if (typeof prop === "string" && (METHODS as string[]).includes(prop)) {
        return (options: DialogOptions) => wrap(prop as DialogMethod, options);
      }
      return Reflect.get(target, prop, receiver);
    },
  });
}
