<script setup lang="ts">
import type { ReaderSettings, ReaderPagePadding } from "@/components/reader/types";
import ReaderSettingsSubHeader from "./ReaderSettingsSubHeader.vue";

defineProps<{
  settings: ReaderSettings;
}>();

const emit = defineEmits<{
  back: [];
  "update-page-padding": [patch: Partial<ReaderPagePadding>];
}>();
</script>

<template>
  <ReaderSettingsSubHeader title="页边距设置" @back="emit('back')" />

  <div class="reader-settings__row">
    <span class="reader-settings__label">上边距</span>
    <n-slider
      :value="settings.pagePadding.top"
      @update:value="(v: number) => emit('update-page-padding', { top: v })"
      :min="4"
      :max="96"
      :step="4"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 40px">{{ settings.pagePadding.top }}px</span>
  </div>
  <div class="reader-settings__row">
    <span class="reader-settings__label">右边距</span>
    <n-slider
      :value="settings.pagePadding.right"
      @update:value="(v: number) => emit('update-page-padding', { right: v })"
      :min="4"
      :max="96"
      :step="4"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 40px">{{ settings.pagePadding.right }}px</span>
  </div>
  <div class="reader-settings__row">
    <span class="reader-settings__label">下边距</span>
    <n-slider
      :value="settings.pagePadding.bottom"
      @update:value="(v: number) => emit('update-page-padding', { bottom: v })"
      :min="4"
      :max="96"
      :step="4"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 40px"
      >{{ settings.pagePadding.bottom }}px</span
    >
  </div>
  <div class="reader-settings__row">
    <span class="reader-settings__label">左边距</span>
    <n-slider
      :value="settings.pagePadding.left"
      @update:value="(v: number) => emit('update-page-padding', { left: v })"
      :min="4"
      :max="96"
      :step="4"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 40px">{{ settings.pagePadding.left }}px</span>
  </div>
</template>

<style scoped>
.reader-settings__row {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.reader-settings__label {
  flex-shrink: 0;
  width: 52px;
  color: rgba(255, 255, 255, 0.65);
  font-size: 13px;
  line-height: 1.35;
  text-align: left;
  white-space: nowrap;
}

.reader-settings__row :deep(.n-slider) {
  flex: 1;
  min-width: 84px;
}

.reader-settings__val {
  flex-shrink: 0;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.5);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

@media (max-width: 420px) {
  .reader-settings__row {
    gap: 8px;
  }

  .reader-settings__label {
    width: 48px;
  }
}
</style>
