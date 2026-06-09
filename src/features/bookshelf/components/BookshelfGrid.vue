<!--
  BookshelfGrid — 书架滚动网格和下拉刷新容器。
-->
<script setup lang="ts">
import { BookOpen } from "lucide-vue-next";
import type { ShelfBook } from "@/stores";
import ShelfBookCard from "@/components/bookshelf/ShelfBookCard.vue";
import { useShelfPullRefresh } from "@/composables/useShelfPullRefresh";

const props = defineProps<{
  loading: boolean;
  books: ShelfBook[];
  filteredBooks: ShelfBook[];
  privacyModeEnabled: boolean;
  openingBookId: string | null;
  tocRefreshingBookIds: Set<string>;
  editMode?: boolean;
  selectedBookIds?: Set<string>;
  shelfOrder?: number;
  showShelfTitle?: boolean;
  shelfTitle?: string;
}>();

const emit = defineEmits<{
  (e: "select", book: ShelfBook): void;
  (e: "contextmenu", book: ShelfBook, event: MouseEvent): void;
  (e: "refresh"): Promise<void>;
}>();

// 下拉刷新
const {
  pullDistance,
  isRefreshing,
  isReady,
  isActivePulling,
  PULL_THRESHOLD,
  onTouchStart,
  onTouchMove,
  onTouchEnd,
  onMouseDown,
} = useShelfPullRefresh({
  onRefresh: async () => {
    await emit("refresh");
  },
});
</script>

<template>
  <div class="bs-wrapper">
    <!-- 下拉刷新指示器：用 height 驱动展开，拖动期间禁用 transition 避免弹跳，
         rAF 节流保证每帧最多一次 Vue 更新。-->
    <div
      class="bs-pull-indicator"
      :class="{
        'bs-pull-indicator--ready': isReady,
        'bs-pull-indicator--refreshing': isRefreshing,
      }"
      :style="{
        height: `${isRefreshing ? PULL_THRESHOLD : pullDistance}px`,
        transition: isActivePulling ? 'none' : 'height 0.2s ease-out',
      }"
    >
      <div class="bs-pull-indicator__content">
        <!-- 刷新动画 -->
        <div v-if="isRefreshing" class="bs-pull-indicator__spinner">
          <svg viewBox="0 0 24 24" fill="none">
            <circle
              class="bs-pull-indicator__spinner-track"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              stroke-width="2"
            />
            <circle
              class="bs-pull-indicator__spinner-head"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-dasharray="31.4"
              stroke-dashoffset="10"
            />
          </svg>
        </div>
        <!-- 下拉箭头 -->
        <div
          v-else
          class="bs-pull-indicator__arrow"
          :class="{ 'bs-pull-indicator__arrow--up': isReady }"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          >
            <path d="M12 19V5M5 12l7-7 7 7" />
          </svg>
        </div>
        <!-- 提示文字 -->
        <span class="bs-pull-indicator__text">
          <template v-if="isRefreshing">刷新中...</template>
          <template v-else-if="isReady">释放刷新</template>
          <template v-else>下拉刷新</template>
        </span>
      </div>
    </div>

    <div
      ref="containerRef"
      class="bs-content app-scrollbar"
      @touchstart="onTouchStart"
      @touchmove="onTouchMove"
      @touchend="onTouchEnd"
      @mousedown="onMouseDown"
    >
      <n-spin :show="loading" class="bs-spin" style="flex: 1">
        <div class="bs-home-modules">
          <slot name="before-grid" />

          <div class="bs-shelf-module" :style="{ order: shelfOrder ?? 0 }">
            <div v-if="showShelfTitle !== false" class="bs-shelf-head">
              <div class="bs-shelf-head__title-block">
                <div class="bs-shelf-head__title-row">
                  <BookOpen :size="16" />
                  <h2 class="bs-shelf-head__title">
                    {{ shelfTitle?.trim() || "我的书架" }}
                  </h2>
                </div>
                <div class="bs-shelf-head__meta">{{ filteredBooks.length }} 本书籍</div>
              </div>
            </div>

            <div v-if="!loading && !books.length" class="bs-empty">
              <div class="bs-empty__icon">
                <BookOpen :size="56" :stroke-width="1" />
              </div>
              <h3 class="bs-empty__title">书架空空如也</h3>
              <p class="bs-empty__desc">去发现页搜索书籍，加入书架开始阅读吧</p>
            </div>

            <div v-else class="bs-grid">
              <ShelfBookCard
                v-for="book in filteredBooks"
                :key="book.id"
                :book="book"
                :privacy-mode-enabled="privacyModeEnabled"
                :loading="openingBookId === book.id || tocRefreshingBookIds.has(book.id)"
                :loading-blocks-interaction="openingBookId === book.id"
                :loading-label="tocRefreshingBookIds.has(book.id) ? '检查更新中' : '加载中'"
                :edit-mode="editMode"
                :selected="selectedBookIds?.has(book.id)"
                @select="emit('select', $event)"
                @contextmenu="(_, e: MouseEvent) => emit('contextmenu', book, e)"
              />
            </div>
          </div>
        </div>
      </n-spin>
    </div>
  </div>
</template>

<style scoped>
.bs-wrapper {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
}

.bs-pull-indicator {
  flex-shrink: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding-bottom: 10px;
  height: 0;
  overflow: hidden;
  /* transition 由 style 绑定控制，拖动时为 none，释放后才启用 */
}

.bs-pull-indicator__content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.bs-pull-indicator--refreshing .bs-pull-indicator__content,
.bs-pull-indicator--ready .bs-pull-indicator__content {
  opacity: 1;
}

.bs-pull-indicator__spinner {
  width: 20px;
  height: 20px;
  color: var(--color-accent);
}

.bs-pull-indicator__spinner svg {
  width: 100%;
  height: 100%;
}

.bs-pull-indicator__spinner-track {
  opacity: 0.25;
}

.bs-pull-indicator__spinner-head {
  transform-origin: center;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.bs-pull-indicator__arrow {
  width: 20px;
  height: 20px;
  color: var(--color-text-muted);
  transition:
    transform 0.2s ease,
    color 0.2s ease;
}

.bs-pull-indicator__arrow svg {
  width: 100%;
  height: 100%;
}

.bs-pull-indicator--ready .bs-pull-indicator__arrow {
  color: var(--color-accent);
}

.bs-pull-indicator__arrow--up {
  transform: rotate(180deg);
}

.bs-pull-indicator__text {
  font-size: 12px;
  color: var(--color-text-muted);
  white-space: nowrap;
}

.bs-pull-indicator--ready .bs-pull-indicator__text {
  color: var(--color-accent);
}

.bs-pull-indicator--refreshing .bs-pull-indicator__text {
  color: var(--color-accent);
}

.bs-content {
  flex: 1;
  overflow-y: auto;
  padding: 0 24px 16px;
}

.bs-spin {
  display: block;
  width: 100%;
  min-height: 100%;
}

.bs-spin :deep(.n-spin-container),
.bs-spin :deep(.n-spin-content) {
  width: 100%;
  min-height: 100%;
}

.bs-home-modules {
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: 0;
  min-height: 100%;
}

.bs-shelf-module {
  width: 100%;
  min-width: 0;
}

.bs-shelf-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.bs-shelf-head__title-block {
  min-width: 0;
}

.bs-shelf-head__title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--color-accent);
}

.bs-shelf-head__title {
  margin: 0;
  color: var(--color-text);
  font-size: var(--fs-16);
  font-weight: var(--fw-bold);
  line-height: 1.35;
}

.bs-shelf-head__meta {
  margin-top: 2px;
  color: var(--color-text-muted);
  font-size: var(--fs-12);
  line-height: 1.4;
}

.bs-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--book-card-col-min, 120px), 1fr));
  gap: 12px;
  padding-top: 4px;
}

.bs-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 80px 0;
  gap: 8px;
}

.bs-empty__icon {
  opacity: 0.25;
  color: var(--color-text-muted);
}

.bs-empty__title {
  font-size: 1rem;
  font-weight: var(--fw-semibold);
  color: var(--color-text-soft);
  margin: 0;
}

.bs-empty__desc {
  font-size: var(--fs-13);
  color: var(--color-text-muted);
  margin: 0;
}

@media (pointer: coarse), (max-width: 640px) {
  .bs-content {
    padding: 0 16px 16px;
  }

  .bs-grid {
    grid-template-columns: repeat(var(--bs-mobile-cols, 4), 1fr);
    gap: 8px;
  }
}
</style>
