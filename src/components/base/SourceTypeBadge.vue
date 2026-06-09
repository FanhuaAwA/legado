<!-- SourceTypeBadge — 统一显示书源类型图标，并标记音频/视频的完全体模式锁定状态。 -->
<script setup lang="ts">
import { Film, Image, Music, BookOpen, Lock, Unlock } from "lucide-vue-next";
import { computed } from "vue";
import { usePreferencesStore } from "@/stores/preferences";

const props = withDefaults(
  defineProps<{
    sourceType?: string;
    /** false = 半透明（书架卡片用）；true = 不透明（发现页用） */
    opaque?: boolean;
    size?: number;
  }>(),
  { sourceType: "", opaque: false, size: 13 },
);

const ICON_MAP: Record<string, typeof Film> = {
  comic: Image,
  video: Film,
  music: Music,
  novel: BookOpen,
};

const TITLE_MAP: Record<string, string> = {
  comic: "漫画",
  video: "视频",
  music: "音频",
  novel: "小说",
};

const prefStore = usePreferencesStore();

const isUnlockRequired = computed(
  () => props.sourceType === "video" || props.sourceType === "music",
);
const isUnlocked = computed(() => prefStore.devTools.fullModeEnabled);
const lockIcon = computed(() => (isUnlocked.value ? Unlock : Lock));

// novel 是默认类型，不显示
const icon = computed(() => {
  const t = props.sourceType;
  if (!t || t === "novel") {
    return null;
  }
  return ICON_MAP[t] ?? null;
});

const title = computed(() => {
  const label = TITLE_MAP[props.sourceType ?? ""] ?? props.sourceType;
  if (!isUnlockRequired.value) {
    return label;
  }
  return isUnlocked.value ? `${label}（已解锁）` : `${label}（已锁定，需要解锁完全体模式）`;
});
</script>

<template>
  <span
    v-if="icon"
    class="source-type-badge"
    :class="{ 'source-type-badge--opaque': opaque }"
    :style="{ width: `${size + 9}px`, height: `${size + 9}px`, borderRadius: '50%' }"
    :title="title"
  >
    <component :is="icon" :size="size" :stroke-width="2.2" />
    <span v-if="isUnlockRequired" class="source-type-badge__lock">
      <component :is="lockIcon" :size="Math.max(8, Math.round(size * 0.72))" :stroke-width="2.6" />
    </span>
  </span>
</template>

<style scoped>
.source-type-badge {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.38);
  color: rgba(255, 255, 255, 0.72);
  backdrop-filter: blur(4px);
  flex-shrink: 0;
}

.source-type-badge__lock {
  position: absolute;
  right: -4px;
  bottom: -4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: var(--color-surface);
  color: var(--color-text-primary);
  box-shadow: 0 0 0 1px var(--color-border);
}

.source-type-badge--opaque {
  background: rgba(24, 160, 88, 0.9);
  color: #fff;
  backdrop-filter: none;
}

.source-type-badge--opaque .source-type-badge__lock {
  background: rgba(0, 0, 0, 0.72);
  color: #fff;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.35);
}
</style>
