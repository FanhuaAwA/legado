<script setup lang="ts">
import type { ReaderSettings, ReaderTypography } from "@/components/reader/types";
import ReaderSettingsSubHeader from "./ReaderSettingsSubHeader.vue";

defineProps<{
  settings: ReaderSettings;
}>();

const emit = defineEmits<{
  back: [];
  "update-typography": [patch: Partial<ReaderTypography>];
}>();
</script>

<template>
  <ReaderSettingsSubHeader title="间距设置" @back="emit('back')" />

  <div class="reader-settings__row">
    <span class="reader-settings__label">行距</span>
    <n-slider
      :value="settings.typography.lineHeight"
      @update:value="(v: number) => emit('update-typography', { lineHeight: v })"
      :min="1.0"
      :max="3.0"
      :step="0.1"
      :format-tooltip="(v: number) => v.toFixed(1)"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 36px">{{
      settings.typography.lineHeight.toFixed(1)
    }}</span>
  </div>
  <div class="reader-settings__row">
    <span class="reader-settings__label">段距</span>
    <n-slider
      :value="settings.typography.paragraphSpacing"
      @update:value="(v: number) => emit('update-typography', { paragraphSpacing: v })"
      :min="0"
      :max="40"
      :step="2"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 36px"
      >{{ settings.typography.paragraphSpacing }}px</span
    >
  </div>
  <div class="reader-settings__row">
    <span class="reader-settings__label">缩进</span>
    <n-slider
      :value="settings.typography.textIndent"
      @update:value="(v: number) => emit('update-typography', { textIndent: v })"
      :min="0"
      :max="4"
      :step="0.5"
      :format-tooltip="(v: number) => v.toFixed(1) + 'em'"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 36px"
      >{{ settings.typography.textIndent }}em</span
    >
  </div>
  <div class="reader-settings__row">
    <span class="reader-settings__label">字距</span>
    <n-slider
      :value="settings.typography.letterSpacing"
      @update:value="(v: number) => emit('update-typography', { letterSpacing: v })"
      :min="0"
      :max="6"
      :step="0.5"
      :format-tooltip="(v: number) => v.toFixed(1) + 'px'"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 36px"
      >{{ settings.typography.letterSpacing }}px</span
    >
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
  width: 44px;
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
    width: 40px;
  }
}
</style>
