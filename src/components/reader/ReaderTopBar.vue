<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ChevronLeft,
  MoreVertical,
  RefreshCw,
  ArrowRightLeft,
  Pencil,
  X,
  Download,
  ExternalLink,
  ShieldCheck,
} from "lucide-vue-next";
import { storeToRefs } from "pinia";
import { ref, computed } from "vue";
import { useOverlay } from "@/composables/useOverlay";
import { useAppConfigStore } from "@/stores";

const props = withDefaults(
  defineProps<{
    bookName?: string;
    chapterName: string;
    sourceName?: string;
    currentIndex: number;
    totalChapters: number;
    chapterUrl?: string;
    /** 书源类型："novel" | "comic" | "video" */
    sourceType?: string;
    canWholeBookSwitch?: boolean;
    canTemporarySwitch?: boolean;
    hasTemporaryOverride?: boolean;
    /** 控制工具栏显隐（CSS transition，不卸载 DOM） */
    visible?: boolean;
  }>(),
  {
    visible: true,
  },
);

const emit = defineEmits<{
  (e: "close"): void;
  (e: "refresh-chapter"): void;
  (e: "cache-chapters", count: number): void;
  (e: "whole-book-switch"): void;
  (e: "temporary-switch"): void;
  (e: "clear-temporary-switch"): void;
}>();

const _appCfg = useAppConfigStore();
const { config } = storeToRefs(_appCfg);

// ── 三点菜单状态 ──────────────────────────────────────────────────────────
const menuOpen = ref(false);
/** 是否展示缓存章节面板 */
const showCachePanel = ref(false);
/** 缓存章节数滑块值，-1 = 全部 */
const cacheCount = ref(computed(() => config.value.cache_prefetch_count).value);

const maxCacheCount = computed(() => props.totalChapters || 50);

const cacheCountDisplay = computed(() => {
  if (cacheCount.value < 0) {
    return "全部章节";
  }
  if (cacheCount.value === 0) {
    return "关闭（0章）";
  }
  return `${cacheCount.value} 章`;
});

/** 是否支持缓存（视频不支持） */
const supportCache = computed(() => props.sourceType !== "video");
const displayBookName = computed(() => props.bookName?.trim() || props.chapterName);
const displaySourceName = computed(() => props.sourceName?.trim() || "未知书源");
const canSourceSwitch = computed(() => !!props.canWholeBookSwitch || !!props.canTemporarySwitch);
const currentChapterUrl = computed(() => props.chapterUrl?.trim() ?? "");

function openMenu() {
  menuOpen.value = true;
}

function closeMenu() {
  menuOpen.value = false;
  showCachePanel.value = false;
}

async function handleOpenOriginalPage() {
  if (!currentChapterUrl.value) {
    return;
  }
  await openUrl(currentChapterUrl.value);
}

function handleRefresh() {
  closeMenu();
  emit("refresh-chapter");
}

function handleShowCachePanel() {
  menuOpen.value = true;
  showCachePanel.value = true;
}

function handleSourceSwitch() {
  closeMenu();
  if (props.canWholeBookSwitch) {
    emit("whole-book-switch");
    return;
  }
  if (props.canTemporarySwitch) {
    emit("temporary-switch");
  }
}

function handleStartCache() {
  closeMenu();
  emit("cache-chapters", cacheCount.value < 0 ? -1 : cacheCount.value);
}

function handleWholeBookSwitch() {
  closeMenu();
  emit("whole-book-switch");
}

function handleTemporarySwitch() {
  closeMenu();
  emit("temporary-switch");
}

function handleClearTemporarySwitch() {
  closeMenu();
  emit("clear-temporary-switch");
}

// 三点下拉菜单接入返回栈
useOverlay(() => menuOpen.value, closeMenu);
</script>

<template>
  <div
    class="reader-top-bar reader-toolbar"
    :class="{ 'reader-toolbar--hidden': visible === false }"
    :aria-hidden="visible === false ? 'true' : undefined"
  >
    <div class="reader-top-bar__main">
      <button class="reader-top-bar__back" @click="emit('close')" title="返回" aria-label="返回">
        <ChevronLeft :size="22" />
      </button>
      <div class="reader-top-bar__center">
        <span class="reader-top-bar__title" :title="displayBookName">{{ displayBookName }}</span>
      </div>

      <div class="reader-top-bar__actions">
        <button
          v-if="canSourceSwitch"
          class="reader-top-bar__action"
          title="换源"
          aria-label="换源"
          @click.stop="handleSourceSwitch"
        >
          <ArrowRightLeft :size="20" />
        </button>
        <button
          class="reader-top-bar__action"
          title="刷新本章"
          aria-label="刷新本章"
          @click.stop="handleRefresh"
        >
          <RefreshCw :size="20" />
        </button>
        <button
          v-if="supportCache"
          class="reader-top-bar__action"
          title="缓存章节"
          aria-label="缓存章节"
          @click.stop="handleShowCachePanel"
        >
          <Download :size="20" />
        </button>
        <button
          class="reader-top-bar__more"
          @click.stop="openMenu"
          title="更多操作"
          aria-label="更多操作"
        >
          <MoreVertical :size="20" />
        </button>
      </div>
    </div>

    <div class="reader-top-bar__details">
      <div class="reader-top-bar__chapter">
        <span class="reader-top-bar__chapter-name" :title="chapterName">{{ chapterName }}</span>
        <a
          v-if="currentChapterUrl"
          class="reader-top-bar__url"
          href="#"
          :title="currentChapterUrl"
          @click.prevent="handleOpenOriginalPage"
          >{{ currentChapterUrl }}</a
        >
        <div v-if="currentChapterUrl" class="reader-top-bar__url-actions">
          <button class="reader-top-bar__url-action" @click.stop="handleOpenOriginalPage">
            <ExternalLink :size="13" />
            <span>查看原网页</span>
          </button>
          <span class="reader-top-bar__url-mode">
            <ShieldCheck :size="13" />
            <span>去广告模式</span>
          </span>
        </div>
      </div>
      <span class="reader-top-bar__source" :title="displaySourceName">
        {{ displaySourceName }}
      </span>
    </div>

    <!-- 遮罩 -->
    <Transition name="reader-menu-fade">
      <div v-if="menuOpen" class="reader-top-bar__overlay" @click="closeMenu" />
    </Transition>

    <!-- 下拉菜单面板 -->
    <Transition name="reader-menu-slide">
      <div v-if="menuOpen" class="reader-top-bar__dropdown" @click.stop>
        <!-- 默认菜单列表 -->
        <template v-if="!showCachePanel">
          <button class="reader-top-bar__menu-item" @click="handleRefresh">
            <RefreshCw :size="16" />
            <span>刷新本章</span>
          </button>
          <button
            v-if="currentChapterUrl"
            class="reader-top-bar__menu-item"
            @click="handleOpenOriginalPage"
          >
            <ExternalLink :size="16" />
            <span>查看原网页</span>
          </button>
          <div v-if="currentChapterUrl" class="reader-top-bar__menu-status">
            <ShieldCheck :size="16" />
            <span>当前为去广告模式</span>
          </div>
          <button
            v-if="canWholeBookSwitch"
            class="reader-top-bar__menu-item"
            @click="handleWholeBookSwitch"
          >
            <ArrowRightLeft :size="16" />
            <span>整本换源</span>
          </button>
          <button
            v-if="canTemporarySwitch"
            class="reader-top-bar__menu-item"
            @click="handleTemporarySwitch"
          >
            <Pencil :size="16" />
            <span>本章临时换源</span>
          </button>
          <button
            v-if="hasTemporaryOverride"
            class="reader-top-bar__menu-item"
            @click="handleClearTemporarySwitch"
          >
            <X :size="16" />
            <span>恢复原始正文</span>
          </button>
          <button
            v-if="supportCache"
            class="reader-top-bar__menu-item"
            @click="handleShowCachePanel"
          >
            <Download :size="16" />
            <span>缓存章节</span>
          </button>
          <div class="reader-top-bar__menu-info">{{ currentIndex + 1 }}/{{ totalChapters }}</div>
        </template>

        <!-- 缓存章节面板 -->
        <template v-else>
          <div class="reader-top-bar__cache-panel">
            <div class="reader-top-bar__cache-header">
              <button class="reader-top-bar__cache-back" @click="showCachePanel = false">
                <ChevronLeft :size="16" />
              </button>
              <span>缓存章节</span>
            </div>
            <div class="reader-top-bar__cache-desc">从当前章节开始向后缓存</div>
            <div class="reader-top-bar__cache-label">
              {{ cacheCountDisplay }}
            </div>
            <n-slider
              v-model:value="cacheCount"
              :min="-1"
              :max="maxCacheCount"
              :step="1"
              :marks="{ '-1': '全部', 0: '关闭' }"
              style="margin: 8px 4px 12px"
            />
            <n-button
              type="primary"
              size="small"
              block
              :disabled="cacheCount === 0"
              @click="handleStartCache"
            >
              开始缓存
            </n-button>
          </div>
        </template>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.reader-top-bar {
  position: absolute;
  top: var(--reader-top-top, 0px);
  left: var(--reader-top-left, 0px);
  right: var(--reader-top-right, 0px);
  z-index: 11;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  min-height: var(--reader-top-height, auto);
  padding: var(--reader-top-padding, 8px 12px);
  padding-top: var(
    --reader-top-padding-top,
    calc(var(--safe-area-inset-top, env(safe-area-inset-top, 0px)) + 8px)
  );
  max-width: var(--reader-top-max-width, none);
  margin: var(--reader-top-margin, 0);
  border: var(--reader-top-border, none);
  border-radius: var(--reader-top-radius, 0px);
  box-shadow: var(--reader-top-shadow, none);
  background: var(--reader-top-bar-bg, rgba(0, 0, 0, 0.65));
  backdrop-filter: var(--reader-top-bar-backdrop-filter, blur(12px));
  color: var(--reader-top-bar-color, #e0e0e0);
  transform: translateY(0);
}

.reader-top-bar::before {
  content: var(--reader-top-menu-label-text, "");
  display: var(--reader-top-menu-label-display, none);
  align-items: center;
  flex: var(--reader-top-menu-label-flex, 0 0 auto);
  height: var(--reader-top-menu-label-height, auto);
  color: var(--reader-top-menu-label-color, inherit);
  font: var(--reader-top-menu-label-font, inherit);
  white-space: pre;
}

.reader-top-bar__back,
.reader-top-bar__action,
.reader-top-bar__more {
  display: var(--reader-top-icon-display, inline-flex);
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  width: 32px;
  height: 32px;
  padding: 0;
  border-radius: 4px;
  transition: background 0.15s;
  flex-shrink: 0;
}

.reader-top-bar__back {
  display: var(--reader-top-back-display, var(--reader-top-icon-display, inline-flex));
}

.reader-top-bar__more {
  display: var(--reader-top-more-display, var(--reader-top-icon-display, inline-flex));
}

.reader-top-bar__main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.reader-top-bar__actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.reader-top-bar__back:hover,
.reader-top-bar__action:hover,
.reader-top-bar__more:hover {
  background: rgba(255, 255, 255, 0.1);
}

.reader-top-bar__center {
  flex: 1;
  min-width: 0;
  display: var(--reader-top-center-display, flex);
  flex-direction: column;
  gap: 2px;
}

.reader-top-bar__title {
  font-size: 0.875rem;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reader-top-bar__details {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  min-width: 0;
  padding: 0 2px 2px 40px;
}

.reader-top-bar__chapter {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.reader-top-bar__chapter-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--reader-top-bar-color, #e0e0e0);
  font-size: 0.8125rem;
  line-height: 1.25;
  opacity: 0.86;
}

.reader-top-bar__source {
  max-width: min(34vw, 112px);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
  margin-top: 1px;
  padding: 4px 8px;
  border-radius: 6px;
  background: var(--reader-top-source-bg, rgba(210, 162, 62, 0.9));
  color: var(--reader-top-source-color, #fff7dd);
  font-size: 0.75rem;
  font-weight: 600;
  line-height: 1.2;
}

.reader-top-bar__url {
  font-size: 0.6875rem;
  color: var(--reader-top-bar-link-color, #63b3ed);
  opacity: 0.7;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-decoration: none;
  cursor: pointer;
}

.reader-top-bar__url:hover {
  opacity: 1;
  text-decoration: underline;
}

.reader-top-bar__url-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 3px;
}

.reader-top-bar__url-action,
.reader-top-bar__url-mode {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-height: 24px;
  padding: 3px 8px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 5px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--reader-top-bar-color, #e0e0e0);
  font-size: 0.6875rem;
  line-height: 1;
  cursor: pointer;
  transition:
    background 0.15s,
    opacity 0.15s;
}

.reader-top-bar__url-action:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.14);
}

.reader-top-bar__url-action:disabled {
  cursor: wait;
  opacity: 0.55;
}

.reader-top-bar__url-mode {
  cursor: default;
  border-color: rgba(99, 226, 183, 0.28);
  background: rgba(99, 226, 183, 0.12);
  color: var(--reader-top-bar-accent, #63e2b7);
}

/* 遮罩 */
.reader-top-bar__overlay {
  position: fixed;
  inset: 0;
  z-index: 10;
}

/* 下拉菜单 */
.reader-top-bar__dropdown {
  position: absolute;
  top: var(--reader-top-dropdown-top, calc(100% + 4px));
  right: var(--reader-top-dropdown-right, 8px);
  z-index: 20;
  min-width: var(--reader-top-dropdown-min-width, 160px);
  background: var(--reader-top-dropdown-bg, rgba(30, 30, 30, 0.92));
  backdrop-filter: var(--reader-top-dropdown-backdrop-filter, blur(16px));
  border: var(--reader-top-dropdown-border, 1px solid rgba(255, 255, 255, 0.12));
  border-radius: var(--reader-top-dropdown-radius, 10px);
  box-shadow: var(--reader-top-dropdown-shadow, 0 8px 24px rgba(0, 0, 0, 0.5));
  overflow: hidden;
  padding: 4px 0;
}

.reader-top-bar__menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 10px 16px;
  background: none;
  border: none;
  color: var(--reader-top-dropdown-color, #e0e0e0);
  cursor: pointer;
  font-size: 0.875rem;
  transition: background 0.15s;
  text-align: left;
}

.reader-top-bar__menu-item:hover {
  background: rgba(255, 255, 255, 0.08);
}

.reader-top-bar__menu-status {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 16px;
  color: var(--reader-top-bar-accent, #63e2b7);
  font-size: 0.8125rem;
  background: rgba(99, 226, 183, 0.08);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.reader-top-bar__menu-info {
  padding: 6px 16px 8px;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.4);
  text-align: right;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  margin-top: 2px;
}

/* 缓存面板 */
.reader-top-bar__cache-panel {
  padding: 12px 14px;
  min-width: 220px;
}

.reader-top-bar__cache-header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.875rem;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--reader-top-dropdown-color, #e0e0e0);
}

.reader-top-bar__cache-back {
  display: inline-flex;
  align-items: center;
  background: none;
  border: none;
  color: var(--reader-top-dropdown-color, #e0e0e0);
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
}

.reader-top-bar__cache-back:hover {
  background: rgba(255, 255, 255, 0.1);
}

.reader-top-bar__cache-desc {
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.5);
  margin-bottom: 8px;
}

.reader-top-bar__cache-label {
  font-size: 0.8125rem;
  color: #63b3ed;
  text-align: center;
  margin-bottom: 4px;
  font-weight: 500;
}

/* toolbar 显隐过渡 */
.reader-toolbar {
  transition:
    opacity var(--dur-base) var(--ease-standard),
    transform var(--dur-base) var(--ease-standard);
}

.reader-toolbar--hidden {
  opacity: 0;
  pointer-events: none;
  transform: translateY(-0.5rem);
}

/* 菜单动画 */
.reader-menu-fade-enter-active,
.reader-menu-fade-leave-active {
  transition: opacity 0.15s;
}

.reader-menu-fade-enter-from,
.reader-menu-fade-leave-to {
  opacity: 0;
}

.reader-menu-slide-enter-active,
.reader-menu-slide-leave-active {
  transition:
    opacity 0.15s,
    transform 0.15s;
}

.reader-menu-slide-enter-from,
.reader-menu-slide-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.97);
}
</style>
