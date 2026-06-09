<script setup lang="ts">
import type { StyleValue } from "vue";

defineProps<{
  styleValue: StyleValue;
  skinPresetId?: string;
}>();
</script>

<template>
  <div class="reader-modal" :style="styleValue" :data-reader-skin="skinPresetId || ''">
    <slot />
    <div class="reader-modal__brightness-overlay" aria-hidden="true"></div>
  </div>
</template>

<style scoped>
.reader-modal__brightness-overlay {
  position: absolute;
  inset: 0;
  z-index: 8;
  pointer-events: none;
  background: #000;
  opacity: var(--reader-brightness-overlay-opacity, 0);
  transition: opacity 0.12s ease;
}
</style>
