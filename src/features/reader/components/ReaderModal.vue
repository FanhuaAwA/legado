<script setup lang="ts">
defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  (e: "update:show", value: boolean): void;
}>();
</script>

<template>
  <n-modal
    v-if="show"
    :show="true"
    :mask-closable="false"
    :auto-focus="false"
    :trap-focus="false"
    display-directive="if"
    transform-origin="center"
    @update:show="emit('update:show', $event)"
  >
    <slot />
  </n-modal>
</template>
