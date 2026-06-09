<!--
  src/components/base/AppDialog.vue — n-modal 的薄壳，自动接入返回栈

  目的：消除"业务方写 n-modal 时漏接 useOverlay"的隐患。新代码请优先使用 <AppDialog>。

  约定：
    - 用 v-model:show 控制显示；
    - 关闭路径（mask / Esc / 关闭按钮 / 业务方调 triggerClose）全部统一走返回栈；
    - 复杂场景可通过 defineExpose 的 triggerClose 显式触发关闭。

  对外几乎 100% 兼容 n-modal 的 props/slots/事件，只是把 update:show=false 替换成
  "走 useOverlay.triggerClose"，从而保证 history / 返回栈状态同步。
-->
<script setup lang="ts">
import type { CSSProperties } from "vue";

import { useOverlay } from "@/composables/useOverlay";

interface Props {
  show: boolean;
  title?: string;
  preset?: "card" | "dialog" | "confirm";
  maskClosable?: boolean;
  closable?: boolean;
  closeOnEsc?: boolean;
  width?: string | number;
  style?: CSSProperties | string;
  class?: string;
}

const props = withDefaults(defineProps<Props>(), {
  preset: "card",
  maskClosable: true,
  closable: true,
  closeOnEsc: true,
});

const emit = defineEmits<{
  "update:show": [value: boolean];
  /** 关闭完成时触发（在 onClose 之后）；业务侧需要"关闭后清理"逻辑可监听此事件 */
  closed: [];
}>();

function doClose() {
  if (props.show) {
    emit("update:show", false);
  }
  emit("closed");
}

const { triggerClose } = useOverlay(() => props.show, doClose);

function onUpdateShow(value: boolean) {
  if (value) {
    // n-modal 内部从未主动把 show 改为 true，这里保留以防异常
    emit("update:show", true);
    return;
  }
  // Naive UI 在用户点 mask / Esc / 关闭按钮时都会触发 update:show(false)
  // 统一改走 triggerClose 路径，保证 history 与栈一致
  triggerClose();
}

const computedStyle = (() => {
  if (props.width === undefined) {
    return props.style;
  }
  const widthStr = typeof props.width === "number" ? `${props.width}px` : props.width;
  if (typeof props.style === "string") {
    return `${props.style}; width: ${widthStr};`;
  }
  return { ...props.style, width: widthStr };
})();

defineExpose({ triggerClose });
</script>

<template>
  <n-modal
    :show="show"
    :preset="preset"
    :title="title"
    :mask-closable="maskClosable"
    :closable="closable"
    :close-on-esc="closeOnEsc"
    :style="computedStyle"
    :class="$props.class"
    @update:show="onUpdateShow"
  >
    <slot />
    <template v-if="$slots.header" #header><slot name="header" /></template>
    <template v-if="$slots.footer" #footer><slot name="footer" /></template>
    <template v-if="$slots.action" #action><slot name="action" /></template>
  </n-modal>
</template>
