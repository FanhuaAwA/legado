<script setup lang="ts">
import { computed } from "vue";
import type { BookItem } from "@/stores";
import type { TaggedBookItem, AggregatedBook } from "./types";
export type { TaggedBookItem, AggregatedBook };
import { aggregateTaggedResults } from "@/utils/searchAggregation";
import AppEmpty from "../base/AppEmpty.vue";
import StackedBookCard from "./StackedBookCard.vue";

const props = defineProps<{
  keyword: string;
  results?: TaggedBookItem[];
  groups?: AggregatedBook[];
  showCovers?: boolean;
  loading?: boolean;
  emptyDescription?: string;
}>();

const emit = defineEmits<{
  (e: "select", book: BookItem, fileName: string, sourceDir?: string): void;
}>();

/** 聚合 & 排序 */
const aggregatedBooks = computed<AggregatedBook[]>(() => {
  if (props.groups) {
    return props.groups;
  }
  return aggregateTaggedResults(props.results ?? [], props.keyword);
});
</script>

<template>
  <div class="agg-results">
    <!-- 加载中提示 -->
    <div v-if="loading" class="agg-results__loading">
      <n-spin size="small" />
      <span>搜索中…</span>
    </div>

    <!-- 结果网格 -->
    <TransitionGroup
      v-if="aggregatedBooks.length"
      name="agg-card"
      tag="div"
      class="agg-results__grid"
    >
      <StackedBookCard
        v-for="group in aggregatedBooks"
        :key="group.primary.book.name + '::' + group.primary.sourceName"
        :group="group"
        :show-cover="showCovers ?? true"
        @select="(book, fileName, sourceDir) => emit('select', book, fileName, sourceDir)"
      />
    </TransitionGroup>

    <!-- 空状态 -->
    <AppEmpty v-else-if="!loading" :title="emptyDescription ?? '暂无搜索结果'" />
  </div>
</template>

<style scoped>
.agg-results {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.agg-results__loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-8);
  font-size: var(--fs-14);
  color: var(--color-text-muted);
}
.agg-results__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--book-card-col-min, 220px), 1fr));
  gap: 10px;
}

.agg-card-enter-active {
  transition:
    opacity 0.3s ease,
    transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.agg-card-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}
.agg-card-enter-from {
  opacity: 0;
  transform: scale(0.92) translateY(8px);
}
.agg-card-leave-to {
  opacity: 0;
  transform: scale(0.95);
}
.agg-card-move {
  transition: transform 0.25s ease;
}
</style>
