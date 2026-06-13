<script setup lang="ts">
import { BookOpen, Compass, Search, LayoutGrid, Package, SlidersHorizontal } from "lucide-vue-next";
import { type Component } from "vue";
import type { NavItem } from "./types";
export type { NavItem };

const props = withDefaults(
  defineProps<{
    items?: NavItem[];
    activeId?: string;
  }>(),
  {
    items: () => [],
    activeId: "",
  },
);

const emit = defineEmits<{
  select: [id: string];
}>();

function selectItem(id: string) {
  emit("select", id);
}

function onItemKeyDown(event: KeyboardEvent, index: number) {
  const itemEls = document.querySelectorAll<HTMLElement>(".side-bar__item[tabindex]");
  const len = itemEls.length;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    itemEls[(index + 1) % len]?.focus();
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    itemEls[(index - 1 + len) % len]?.focus();
  } else if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    selectItem(props.items[index].id);
  }
}

/**
 * 内置导航图标映射（使用 lucide-vue-next 组件）
 */
const ICON_COMPONENTS: Record<string, Component> = {
  bookshelf: BookOpen,
  explore: Compass,
  search: Search,
  booksource: LayoutGrid,
  extensions: Package,
  settings: SlidersHorizontal,
};
</script>

<template>
  <nav class="side-bar" aria-label="主导航">
    <!-- ── 导航列表 ─────────────────────────────────── -->
    <ul class="side-bar__list app-scrollbar--hidden" role="menubar">
      <li
        v-for="(item, index) in items"
        :key="item.id"
        class="side-bar__item focusable"
        :class="{ 'side-bar__item--active': activeId === item.id }"
        role="option"
        :aria-selected="activeId === item.id"
        :aria-label="item.label"
        tabindex="0"
        @click="selectItem(item.id)"
        @keydown="onItemKeyDown($event, index)"
      >
        <span class="side-bar__icon" aria-hidden="true">
          <component :is="ICON_COMPONENTS[item.icon]" :size="18" :stroke-width="1.75" />
        </span>
        <span class="side-bar__label">{{ item.label }}</span>
        <span v-if="item.badge" class="side-bar__badge">{{ item.badge }}</span>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
/* ── 侧边栏容器 ──────────────────────────────────────────── */
.side-bar {
  grid-area: sidebar;
  display: flex;
  flex-direction: column;
  width: var(--sidebar-w);
  background: var(--color-sidebar-bg);
  border-right: 1px solid var(--color-sidebar-border);
  overflow: hidden;
  transition:
    background 160ms ease,
    border-color 160ms ease;
}

/* ── 导航列表 ─────────────────────────────────────────────── */
.side-bar__list {
  flex: 1;
  list-style: none;
  margin: 0;
  padding: var(--space-4) var(--space-2) var(--space-2);
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  overflow-x: hidden;
}

/* ── 菜单项 ──────────────────────────────────────────────── */
.side-bar__item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: 40px;
  padding: 0 var(--space-3) 0 14px;
  border-radius: 10px;
  cursor: pointer;
  color: var(--color-sidebar-text-muted);
  transition:
    background 120ms ease,
    color 120ms ease;
  white-space: nowrap;
  overflow: hidden;
  position: relative;
  outline: none;
  border: none;
}

@media (hover: hover) and (pointer: fine) {
  .side-bar__item:hover {
    background: var(--color-sidebar-hover);
    color: var(--color-sidebar-text);
  }
}

.side-bar__item:focus-visible {
  box-shadow: inset 0 0 0 2px var(--color-accent);
}

.side-bar__item--active {
  background: var(--color-sidebar-active-bg);
  color: var(--color-sidebar-active-text);
  font-weight: var(--fw-semibold);
}

.side-bar__item--active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 22%;
  bottom: 22%;
  width: 3px;
  border-radius: 0 3px 3px 0;
  background: currentColor;
}

@media (hover: hover) and (pointer: fine) {
  .side-bar__item--active:hover {
    background: var(--color-sidebar-active-bg);
  }
}

/* ── 图标 ─────────────────────────────────────────────────── */
.side-bar__icon {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ── 文字标签 ─────────────────────────────────────────────── */
.side-bar__label {
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
  letter-spacing: 0.015em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

/* ── 徽标 ─────────────────────────────────────────────────── */
.side-bar__badge {
  flex-shrink: 0;
  min-width: 18px;
  padding: 0 var(--space-1);
  height: 16px;
  border-radius: var(--radius-pill);
  background: var(--color-accent);
  color: #fff;
  font-size: var(--fs-11);
  font-weight: var(--fw-bold);
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
