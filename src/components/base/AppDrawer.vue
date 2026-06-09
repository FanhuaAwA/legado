<!--
  src/components/base/AppDrawer.vue — n-drawer 的薄壳，自动接入返回栈

  同 AppDialog 的设计目标：消除业务方写 n-drawer 时漏接 useOverlay 的隐患。

  对外尽量贴近 n-drawer 的常用接口（placement / width / height / title / closable）。
  内部把 update:show=false 替换为 useOverlay.triggerClose。
-->
<script setup lang="ts">
import { useOverlay } from "@/composables/useOverlay";

interface Props {
  show: boolean;
  title?: string;
  placement?: "top" | "right" | "bottom" | "left";
  width?: string | number;
  height?: string | number;
  maskClosable?: boolean;
  closable?: boolean;
  closeOnEsc?: boolean;
  showMask?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  placement: "right",
  maskClosable: true,
  closable: true,
  closeOnEsc: true,
  showMask: true,
});

const emit = defineEmits<{
  "update:show": [value: boolean];
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
    emit("update:show", true);
    return;
  }
  triggerClose();
}

defineExpose({ triggerClose });
</script>

<template>
  <n-drawer
    :show="show"
    :placement="placement"
    :width="width"
    :height="height"
    :mask-closable="maskClosable"
    :close-on-esc="closeOnEsc"
    :show-mask="showMask"
    @update:show="onUpdateShow"
  >
    <n-drawer-content :title="title" :closable="closable">
      <slot />
      <template v-if="$slots.header" #header><slot name="header" /></template>
      <template v-if="$slots.footer" #footer><slot name="footer" /></template>
    </n-drawer-content>
  </n-drawer>
</template>
