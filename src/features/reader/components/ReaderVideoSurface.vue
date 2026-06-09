<!-- ReaderVideoSurface — 阅读器视频承载层；当前在未开放播放器时显示解锁提示。 -->
<script setup lang="ts">
import { Lock } from "lucide-vue-next";
import { useMessage } from "naive-ui";
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
const message = useMessage();

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

let closeTimer: ReturnType<typeof setTimeout> | null = null;

onMounted(() => {
  message.warning("需要解锁完全体模式后才能使用音频/视频播放", {
    duration: 2500,
    keepAliveOnHover: false,
  });
  closeTimer = setTimeout(() => {
    readerActionsStore.close();
  }, 2500);
});

onBeforeUnmount(() => {
  if (closeTimer !== null) {
    clearTimeout(closeTimer);
    closeTimer = null;
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
  <div class="video-unavailable">
    <Lock :size="24" :stroke-width="2.4" />
    <span class="video-unavailable__text">需要解锁完全体模式后才能使用音频/视频播放</span>
  </div>
</template>

<style scoped>
.video-unavailable {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 100%;
  height: 100%;
  background: #000;
  color: rgba(255, 255, 255, 0.6);
  font-size: 15px;
}
</style>
