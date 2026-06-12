<!--
  阅读器菜单层，承载顶部栏、底部栏、目录、TTS 控制和换源入口。
-->
<script setup lang="ts">
import { Bookmark } from "lucide-vue-next";
import { storeToRefs } from "pinia";
import { computed, nextTick, ref } from "vue";
import ReaderSourceSwitchBridge from "@/features/reader/components/ReaderSourceSwitchBridge.vue";
import {
  useReaderActionsStore,
  useReaderSessionStore,
  useReaderUiStore,
  useReaderViewStore,
} from "@/stores";
import ReaderBottomBar from "../reader/ReaderBottomBar.vue";
import ReaderTocPanel from "../reader/ReaderTocPanel.vue";
import ReaderTopBar from "../reader/ReaderTopBar.vue";
import TtsControlBar from "../reader/TtsControlBar.vue";

const readerActionsStore = useReaderActionsStore();
const readerUiStore = useReaderUiStore();
const readerSessionStore = useReaderSessionStore();
const readerViewStore = useReaderViewStore();
const {
  showMenu,
  showToc,
  settingsVisible,
  showTtsBar,
  showSourceSwitchDialog,
  sourceSwitchMode,
  menuOpenTime,
} = storeToRefs(readerUiStore);
const { readIndices, cachedIndices } = storeToRefs(readerSessionStore);
const {
  addingToShelf,
  bookInfo,
  chapters,
  currentChapterName,
  currentChapterOverride,
  currentChapterUrl,
  currentShelfId,
  fileName,
  hasNext,
  hasPrev,
  isOnShelf,
  isVideoMode,
  readingChapterIndex,
  refreshingToc,
  sourceType,
  ttsProgressText,
} = storeToRefs(readerViewStore);
const bottomBarRef = ref<InstanceType<typeof ReaderBottomBar> | null>(null);
const showTopBar = computed(() => showMenu.value && !settingsVisible.value);
const readerBookName = computed(() => bookInfo.value?.name ?? "");
const readerSourceName = computed(
  () => currentChapterOverride.value?.sourceName ?? bookInfo.value?.sourceName ?? "",
);

function closeSettings() {
  bottomBarRef.value?.closeSettings();
}

function onOverlayClick() {
  if (Date.now() - menuOpenTime.value <= 200) {
    return;
  }
  if (settingsVisible.value) {
    closeSettings();
    return;
  }
  readerUiStore.closeMenu();
}

async function onOpenToc() {
  closeSettings();
  await nextTick();
  await readerUiStore.openToc();
}

function onSettingsVisibleChange(val: boolean) {
  settingsVisible.value = val;
}

defineExpose({ closeSettings });
</script>

<template>
  <!-- 菜单遮罩 -->
  <div v-if="showMenu" class="reader-modal__overlay" @click="onOverlayClick" />

  <!-- 顶部工具栏 -->
  <ReaderTopBar
    v-if="showTopBar"
    :book-name="readerBookName"
    :chapter-name="currentChapterName"
    :source-name="readerSourceName"
    :current-index="readingChapterIndex"
    :total-chapters="chapters.length"
    :chapter-url="currentChapterUrl"
    :source-type="sourceType"
    :can-whole-book-switch="!!currentShelfId && !isVideoMode"
    :can-temporary-switch="!isVideoMode"
    :has-temporary-override="!!currentChapterOverride"
    @close="readerActionsStore.close"
    @refresh-chapter="readerActionsStore.forceRefreshChapter"
    @cache-chapters="readerActionsStore.prefetchChapters"
    @whole-book-switch="readerActionsStore.openWholeBookSourceSwitch"
    @temporary-switch="readerActionsStore.openTemporaryChapterSwitch"
    @clear-temporary-switch="readerActionsStore.clearTemporaryChapterSwitch"
  />

  <!-- 加入书架按钮 -->
  <button
    v-if="showMenu && !settingsVisible && !isOnShelf && bookInfo"
    class="reader-modal__shelf-btn"
    :disabled="addingToShelf"
    @click="readerActionsStore.handleAddToShelf"
  >
    <Bookmark :size="16" aria-hidden="true" />
    {{ addingToShelf ? "加入中…" : "加入书架" }}
  </button>

  <!-- 底部工具栏 -->
  <ReaderBottomBar
    v-if="showMenu"
    ref="bottomBarRef"
    :chapters="chapters"
    :current-index="readingChapterIndex"
    :has-prev="hasPrev"
    :has-next="hasNext"
    :source-type="sourceType"
    @prev="readerActionsStore.gotoPrevChapter"
    @next="readerActionsStore.gotoNextChapter"
    @goto="readerActionsStore.gotoChapter"
    @open-toc="onOpenToc"
    @settings-visible="onSettingsVisibleChange($event)"
    @dump-pagination-layout="readerActionsStore.dumpPaginationLayoutDebug"
    @tts-toggle="readerActionsStore.onTtsToggle"
  />

  <!-- TTS 浮动控制条 -->
  <TtsControlBar
    :visible="showTtsBar"
    :progress-text="ttsProgressText"
    @close="showTtsBar = false"
  />

  <!-- 目录面板 -->
  <ReaderTocPanel
    :show="showToc"
    :chapters="chapters"
    :current-index="readingChapterIndex"
    :book-info="bookInfo"
    :read-indices="readIndices"
    :cached-indices="cachedIndices"
    :refreshing-toc="refreshingToc"
    :source-type="sourceType"
    @update:show="showToc = $event"
    @select="readerActionsStore.gotoChapter"
    @refresh-toc="readerActionsStore.emitRefreshToc"
    @clear-chapter-cache="readerActionsStore.handleClearChapterCache"
    @clear-all-cache="readerActionsStore.handleClearAllCache"
  />

  <ReaderSourceSwitchBridge
    :show="showSourceSwitchDialog"
    :mode="sourceSwitchMode"
    :book-info="bookInfo"
    :file-name="fileName"
    :source-type="sourceType"
    :chapters="chapters"
    :active-chapter-index="readingChapterIndex"
    :current-chapter-url="currentChapterUrl"
    :current-shelf-id="currentShelfId"
    @update:show="showSourceSwitchDialog = $event"
    @chapter-temp-switched="readerActionsStore.handleTemporaryChapterSourceSwitched"
    @whole-book-switched="readerActionsStore.handleWholeBookSourceSwitched"
  />
</template>

<style scoped>
.reader-modal__overlay {
  position: absolute;
  inset: 0;
  background: var(--reader-menu-overlay-bg);
  opacity: 1;
  z-index: 10;
}

.reader-modal__shelf-btn {
  position: absolute;
  top: calc(var(--safe-area-inset-top, env(safe-area-inset-top, 0px)) + 58px);
  left: 50%;
  transform: translateX(-50%);
  z-index: 12;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 16px;
  border: none;
  border-radius: 20px;
  background: var(--color-accent);
  color: #fff;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  backdrop-filter: blur(8px);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
  white-space: nowrap;
  transition:
    opacity 0.2s,
    transform 0.2s;
}

.reader-modal__shelf-btn:disabled {
  opacity: 0.65;
  cursor: default;
}

.reader-modal__shelf-btn:not(:disabled):hover {
  opacity: 0.88;
  transform: translateX(-50%) scale(1.03);
}
</style>
