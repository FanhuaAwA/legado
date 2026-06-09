<script setup lang="ts">
import { ref, watch } from "vue";
import { useOverlay } from "@/composables/useOverlay";

interface Props {
  show?: boolean;
  enabledCount: number;
}

const props = withDefaults(defineProps<Props>(), {
  show: false,
});

const emits = defineEmits<{
  "update:show": [value: boolean];
}>();

const showDialog = ref(props.show);

// ── 返回栈集成 ────────────────────────────────────────────────────────────
useOverlay(
  () => showDialog.value,
  () => {
    showDialog.value = false;
  },
);

function handleClose() {
  showDialog.value = false;
}

// ── 观察 props 变化 ─────────────────────────────────────────────────────
watch(
  () => props.show,
  (newVal) => {
    showDialog.value = newVal;
  },
);

// ── 同步状态回父组件 ──────────────────────────────────────────────────────
watch(
  () => showDialog.value,
  (newVal) => {
    emits("update:show", newVal);
  },
);
</script>

<template>
  <n-dialog
    v-model:show="showDialog"
    :show-icon="true"
    type="warning"
    title="书源过多会影响性能"
    positive-text="知道了"
    :mask-closable="false"
    preset="dialog"
    @positive-click="handleClose"
    @negative-click="handleClose"
  >
    <template #default>
      <div class="warning-content">
        <n-alert type="warning" :bordered="false" class="warning-alert">
          您已启用
          <strong>{{ enabledCount }}</strong>
          个书源。过多启用会导致搜索、探索、发现速度严重下降，应用响应延迟明显。
        </n-alert>
        <p class="warning-text">
          建议您只保留常用的书源，禁用或删除不需要的书源。通常 3-8 个优质书源就足以满足日常需求。
        </p>
        <p class="warning-text">可以在「书源管理」中对书源进行启用/禁用或删除操作。</p>
      </div>
    </template>
  </n-dialog>
</template>

<style scoped>
.warning-content {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.warning-alert {
  margin-bottom: 0.25rem;
}

.warning-text {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.5;
  color: var(--color-text);
  opacity: 0.75;
}
</style>
