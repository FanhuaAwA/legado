<!-- ReaderVideoSurface — 阅读器视频承载层；当前在未开放播放器时显示解锁提示。 -->
<script setup lang="ts">
import { Lock, X } from "lucide-vue-next";
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { ChapterGroup } from "@/stores";
// TODO: 视频功能暂时屏蔽，待启用时取消下方注释并删除临时屏蔽逻辑
// import { storeToRefs } from 'pinia';
// import VideoPlayerPage from '@/components/reader/modes/VideoPlayerPage.vue';
// import { useReaderActionsStore, useReaderSessionStore, useReaderViewStore } from '@/stores';
import { useReaderActionsStore } from "@/stores";

defineProps<{
  chapterGroups?: ChapterGroup[];
  initialGroupIndex?: number;
  inlineGroupTabs?: boolean;
  episodeProgress?: Record<string, { time: number; duration: number; lastPlayedAt: number }>;
}>();

const readerActionsStore = useReaderActionsStore();

// TODO: 待启用时恢复下方 ref 与 store 绑定
// const readerSessionStore = useReaderSessionStore();
// const readerViewStore = useReaderViewStore();
// const playerRef = ref<{
//   getCurrentTime?: () => number;
//   getDuration?: () => number;
// } | null>(null);
// const { activeChapterIndex, content, error, pendingResumePlaybackTime } =
//   storeToRefs(readerSessionStore);
// const { blockingLoading, bookInfo, chapters, fileName, hasNext, hasPrev } =
//   storeToRefs(readerViewStore);

const playerRef = ref<null>(null);
void playerRef; // 保留 ref 声明以兼容父组件的 expose 接口

function getCurrentTime() {
  return 0;
  // return playerRef.value?.getCurrentTime?.() ?? 0;
}

function getDuration() {
  return 0;
  // return playerRef.value?.getDuration?.() ?? 0;
}

defineExpose({ getCurrentTime, getDuration });

const dismissed = ref(false);
let dismissTimer: ReturnType<typeof setTimeout> | null = null;

onMounted(() => {
  dismissTimer = setTimeout(() => {
    dismissed.value = true;
  }, 4500);
});

onBeforeUnmount(() => {
  if (dismissTimer !== null) {
    clearTimeout(dismissTimer);
    dismissTimer = null;
  }
});
</script>

<template>
  <!-- TODO: 待启用时替换为 VideoPlayerPage -->
  <!-- <VideoPlayerPage
    ref="playerRef"
    :content="content"
    :chapters="chapters"
    :active-chapter-index="activeChapterIndex"
    :book-info="bookInfo"
    :loading="blockingLoading"
    :error="error"
    :has-prev="hasPrev"
    :has-next="hasNext"
    :file-name="fileName"
    :resume-time="pendingResumePlaybackTime"
    :chapter-groups="chapterGroups"
    :initial-group-index="initialGroupIndex"
    :inline-group-tabs="inlineGroupTabs"
    :episode-progress="episodeProgress"
    @close="readerActionsStore.close"
    @goto-chapter="readerActionsStore.gotoChapter"
    @prev-chapter="readerActionsStore.gotoPrevChapter"
    @next-chapter="readerActionsStore.gotoNextChapter"
    @progress="readerActionsStore.onVideoProgress"
    @ended="readerActionsStore.onVideoEnded"
    @retry="readerActionsStore.retryCurrentChapter"
  /> -->
  <Transition name="video-lock-fade">
    <div v-if="!dismissed" class="video-unavailable">
      <div class="video-unavailable__card">
        <Lock class="video-unavailable__icon" :size="28" :stroke-width="2" />
        <div class="video-unavailable__body">
          <span class="video-unavailable__title">功能暂未开放</span>
          <span class="video-unavailable__desc">需要解锁完全体模式后才能使用音频/视频播放</span>
        </div>
        <button
          class="video-unavailable__close"
          aria-label="关闭"
          @click="readerActionsStore.close()"
        >
          <X :size="16" />
        </button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.video-unavailable {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  background: radial-gradient(ellipse at center, #1a1a2e 0%, #0a0a0f 100%);
}

.video-unavailable__card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 18px 24px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  max-width: 420px;
}

.video-unavailable__icon {
  flex-shrink: 0;
  color: rgba(255, 255, 255, 0.5);
}

.video-unavailable__body {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.video-unavailable__title {
  font-size: var(--fs-14);
  font-weight: var(--fw-semibold);
  color: rgba(255, 255, 255, 0.85);
}

.video-unavailable__desc {
  font-size: var(--fs-13);
  color: rgba(255, 255, 255, 0.45);
}

.video-unavailable__close {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: none;
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.55);
  cursor: pointer;
  transition:
    background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}

@media (hover: hover) and (pointer: fine) {
  .video-unavailable__close:hover {
    background: rgba(255, 255, 255, 0.16);
    color: rgba(255, 255, 255, 0.85);
  }
}

.video-lock-fade-enter-active {
  transition: opacity 0.35s ease;
}
.video-lock-fade-leave-active {
  transition: opacity 0.5s ease;
}
.video-lock-fade-enter-from,
.video-lock-fade-leave-to {
  opacity: 0;
}
</style>
