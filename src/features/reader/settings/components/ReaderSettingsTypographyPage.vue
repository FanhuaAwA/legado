<script setup lang="ts">
import type { ReaderSettings, ReaderTypography } from "@/components/reader/types";
import ReaderSettingsSubHeader from "./ReaderSettingsSubHeader.vue";

defineProps<{
  settings: ReaderSettings;
  textAlignOptions: Array<{
    label: string;
    value: ReaderTypography["textAlign"];
  }>;
  textShadowPresets: Array<{ label: string; value: string }>;
}>();

const emit = defineEmits<{
  back: [];
  "update-typography": [patch: Partial<ReaderTypography>];
}>();
</script>

<template>
  <ReaderSettingsSubHeader title="字体样式" @back="emit('back')" />

  <!-- 斜体 -->
  <div class="reader-settings__row">
    <span class="reader-settings__label">斜体</span>
    <div class="reader-settings__pill-group">
      <button
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.fontStyle === 'normal',
        }"
        @click="emit('update-typography', { fontStyle: 'normal' })"
      >
        正常
      </button>
      <button
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.fontStyle === 'italic',
        }"
        style="font-style: italic"
        @click="emit('update-typography', { fontStyle: 'italic' })"
      >
        斜体
      </button>
      <button
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.fontStyle === 'oblique',
        }"
        style="font-style: oblique"
        @click="emit('update-typography', { fontStyle: 'oblique' })"
      >
        倾斜
      </button>
    </div>
  </div>

  <!-- 对齐 -->
  <div class="reader-settings__row">
    <span class="reader-settings__label">对齐</span>
    <div class="reader-settings__pill-group">
      <button
        v-for="a in textAlignOptions"
        :key="a.value"
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.textAlign === a.value,
        }"
        @click="emit('update-typography', { textAlign: a.value })"
      >
        {{ a.label }}
      </button>
    </div>
  </div>

  <!-- 文字装饰 -->
  <div class="reader-settings__row">
    <span class="reader-settings__label">装饰</span>
    <div class="reader-settings__pill-group">
      <button
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.textDecoration === 'none',
        }"
        @click="emit('update-typography', { textDecoration: 'none' })"
      >
        无
      </button>
      <button
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.textDecoration === 'underline',
        }"
        style="text-decoration: underline"
        @click="emit('update-typography', { textDecoration: 'underline' })"
      >
        下划线
      </button>
      <button
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.textDecoration === 'line-through',
        }"
        style="text-decoration: line-through"
        @click="emit('update-typography', { textDecoration: 'line-through' })"
      >
        删除线
      </button>
    </div>
  </div>

  <!-- 文字描边 -->
  <div class="reader-settings__row">
    <span class="reader-settings__label">描边</span>
    <n-slider
      :value="settings.typography.textStrokeWidth"
      @update:value="(v: number) => emit('update-typography', { textStrokeWidth: v })"
      :min="0"
      :max="3"
      :step="0.5"
      :format-tooltip="(v: number) => v + 'px'"
      style="flex: 1"
    />
    <span class="reader-settings__val" style="width: 40px"
      >{{ settings.typography.textStrokeWidth }}px</span
    >
    <label class="reader-settings__color-swatch" title="描边颜色">
      <input
        ref="strokeColorInputRef"
        type="color"
        :value="
          settings.typography.textStrokeColor === 'transparent'
            ? '#000000'
            : settings.typography.textStrokeColor
        "
        @input="
          (e) =>
            emit('update-typography', {
              textStrokeColor: (e.target as HTMLInputElement).value,
            })
        "
      />
      <span
        :style="{
          background:
            settings.typography.textStrokeColor === 'transparent'
              ? '#555'
              : settings.typography.textStrokeColor,
        }"
      />
    </label>
  </div>

  <!-- 文字阴影 -->
  <div class="reader-settings__row">
    <span class="reader-settings__label">阴影</span>
    <div class="reader-settings__pill-group">
      <button
        v-for="sh in textShadowPresets"
        :key="sh.label"
        class="reader-settings__pill"
        :class="{
          'reader-settings__pill--active': settings.typography.textShadow === sh.value,
        }"
        @click="emit('update-typography', { textShadow: sh.value })"
      >
        {{ sh.label }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.reader-settings__row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.reader-settings__label {
  flex-shrink: 0;
  width: 44px;
  color: rgba(255, 255, 255, 0.65);
  font-size: 13px;
  text-align: left;
}

.reader-settings__val {
  flex-shrink: 0;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.5);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.reader-settings__pill-group {
  display: flex;
  gap: 6px;
  flex: 1;
  flex-wrap: wrap;
}

.reader-settings__pill-group .reader-settings__pill {
  flex: 1;
  min-width: 0;
}

.reader-settings__pill {
  padding: 6px 10px;
  border-radius: 8px;
  border: none;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.75);
  font-size: 0.75rem;
  cursor: pointer;
  transition:
    background 0.15s,
    color 0.15s;
  white-space: nowrap;
}

.reader-settings__pill:hover {
  background: rgba(255, 255, 255, 0.12);
}

.reader-settings__pill--active {
  background: rgba(99, 226, 183, 0.16);
  color: #63e2b7;
  font-weight: 500;
}

.reader-settings__color-swatch {
  position: relative;
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  cursor: pointer;
}

.reader-settings__color-swatch input[type="color"] {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  cursor: pointer;
}

.reader-settings__color-swatch span {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: 8px;
  border: 2px solid rgba(255, 255, 255, 0.2);
  transition: border-color 0.15s;
}

.reader-settings__color-swatch:hover span {
  border-color: rgba(255, 255, 255, 0.5);
}
</style>
