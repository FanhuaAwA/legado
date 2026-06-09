<!-- ReaderSettingsSubHeader — 阅读器设置子页面统一标题栏与返回入口。 -->
<script setup lang="ts">
import { ChevronLeft } from "lucide-vue-next";

defineProps<{
  title: string;
}>();

const emit = defineEmits<{
  back: [];
}>();
</script>

<template>
  <div class="reader-settings-sub-header">
    <button
      class="reader-settings-sub-header__back"
      type="button"
      aria-label="返回"
      @click="emit('back')"
    >
      <ChevronLeft :size="16" aria-hidden="true" />
    </button>
    <span class="reader-settings-sub-header__title">{{ title }}</span>
    <div v-if="$slots.actions" class="reader-settings-sub-header__actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.reader-settings-sub-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.reader-settings-sub-header__back {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: none;
  color: inherit;
  cursor: pointer;
  transition: background 0.15s;
}

.reader-settings-sub-header__back:hover {
  background: rgba(255, 255, 255, 0.1);
}

.reader-settings-sub-header__back:focus-visible {
  outline: 2px solid rgba(99, 226, 183, 0.75);
  outline-offset: 2px;
}

.reader-settings-sub-header__title {
  min-width: 0;
  flex: 1;
  font-size: 0.875rem;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reader-settings-sub-header__actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}
</style>
