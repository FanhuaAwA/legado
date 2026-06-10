/**
 * useTocAutoUpdate — 书架目录自动更新
 *
 * 根据偏好设置，在打开图书或 App 启动时自动检测目录更新。
 * 每本书的最小检测间隔由 preferences.tocAutoUpdate.minIntervalSecs 控制（默认 2 小时）。
 * 最近检测时间以 bookId 为 key 持久化在前端存储中。
 */

import type { ShelfBook } from "@/composables/useBookshelf";
import { LOCAL_TXT_FILE_NAME, useBookshelfStore } from "@/stores/bookshelf";
import { usePreferencesStore } from "@/stores/preferences";
import { useScriptBridgeStore } from "@/stores/scriptBridge";
import { useDynamicConfig } from "./useDynamicConfig";

interface RefreshAllTocOptions {
  /** 手动刷新时跳过自动更新开关和间隔限制。 */
  force?: boolean;
}

interface RawChapterItem {
  name: string;
  url: string;
  group?: string;
  vip?: boolean;
  isVip?: boolean;
  price?: unknown;
  currency?: string;
}

// ── 最近检测时间持久化（bookId → timestamp ms） ───────────────────────────

const lastCheckConfig = useDynamicConfig<Record<string, number>>({
  namespace: "toc.lastCheckTimes",
  version: 1,
  defaults: () => ({}),
});

function getLastCheckTime(bookId: string): number {
  return lastCheckConfig.state[bookId] ?? 0;
}

function setLastCheckTime(bookId: string, time: number) {
  void lastCheckConfig.replace({ ...lastCheckConfig.state, [bookId]: time });
}

// ── 核心刷新逻辑 ─────────────────────────────────────────────────────────

/**
 * 刷新单本书的目录，并更新书架中的 totalChapters。
 * @returns 新增章节数量（-1 表示失败）
 */
async function refreshBookToc(
  book: ShelfBook,
  bookshelfStore: ReturnType<typeof useBookshelfStore>,
  scriptBridgeStore: ReturnType<typeof useScriptBridgeStore>,
): Promise<number> {
  bookshelfStore.beginTocRefresh(book.id);
  try {
    const info = await scriptBridgeStore.runBookInfo(book.fileName, book.bookUrl, book.sourceDir);
    const tocUrl = (info as { tocUrl?: string }).tocUrl ?? book.bookUrl;
    const raw = await scriptBridgeStore.runChapterList(
      book.fileName,
      tocUrl,
      undefined,
      book.sourceDir,
    );
    const fetched = (raw as RawChapterItem[]).map((chapter, index) => ({
      index,
      name: chapter.name,
      url: chapter.url,
      group: chapter.group,
      vip: chapter.vip ?? chapter.isVip,
      price: chapter.price,
      currency: chapter.currency,
    }));

    const cached = await bookshelfStore.getChapters(book.id);
    const oldUrls = new Set(cached.map((c) => c.url));
    const newCount = fetched.filter((c) => !oldUrls.has(c.url)).length;

    await bookshelfStore.saveChapters(book.id, fetched);

    // 同步更新书架中的 totalChapters
    if (fetched.length !== book.totalChapters) {
      await bookshelfStore.patchBook(book.id, {
        totalChapters: fetched.length,
      });
    }

    setLastCheckTime(book.id, Date.now());
    return newCount;
  } catch {
    return -1;
  } finally {
    bookshelfStore.endTocRefresh(book.id);
  }
}

// ── 导出函数 ─────────────────────────────────────────────────────────────

export function useTocAutoUpdate() {
  const preferencesStore = usePreferencesStore();
  const bookshelfStore = useBookshelfStore();
  const scriptBridgeStore = useScriptBridgeStore();

  function isAutoUpdateEnabled(trigger: "onBookOpen" | "onAppStart" | "onShelfView"): boolean {
    const cfg = preferencesStore.tocAutoUpdate;
    return cfg.enabled && cfg[trigger];
  }

  function canRefreshBook(book: ShelfBook): boolean {
    return !!book.fileName && !!book.bookUrl && book.fileName !== LOCAL_TXT_FILE_NAME;
  }

  /** 检查某本书是否需要更新（根据最小间隔时间限制） */
  function isRefreshDue(bookId: string): boolean {
    const minInterval = preferencesStore.tocAutoUpdate.minIntervalSecs * 1000;
    const last = getLastCheckTime(bookId);
    return Date.now() - last >= minInterval;
  }

  /**
   * 打开图书时触发的自动检测。
   * 仅在设置 enabled + onBookOpen 都开启，且距离上次检测超过最小间隔时才执行。
   * @returns 新增章节数（-1 = 跳过或失败，0 = 已是最新）
   */
  async function refreshOnBookOpen(book: ShelfBook): Promise<number> {
    if (!isAutoUpdateEnabled("onBookOpen") || !canRefreshBook(book)) {
      return -1;
    }
    if (!isRefreshDue(book.id)) {
      return -1;
    }
    return refreshBookToc(book, bookshelfStore, scriptBridgeStore);
  }

  /**
   * App 启动时对所有书架书籍触发自动检测（逐本串行，避免并发打爆书源）。
   * 仅在设置 enabled + onAppStart 开启，且单本书距离上次超过最小间隔时才执行。
   */
  async function refreshAllOnAppStart(): Promise<void> {
    if (!isAutoUpdateEnabled("onAppStart")) {
      return;
    }

    for (const book of bookshelfStore.books) {
      if (!isAutoUpdateEnabled("onAppStart")) {
        break;
      }
      if (!canRefreshBook(book)) {
        continue;
      }
      if (!isRefreshDue(book.id)) {
        continue;
      }
      await refreshBookToc(book, bookshelfStore, scriptBridgeStore);
    }
  }

  /**
   * 切换到书架视图时对所有书架书籍触发自动检测（逐本串行）。
   * 仅在设置 enabled + onShelfView 开启，且单本书距离上次超过最小间隔时才执行。
   * @returns 刷新结果摘要 { success: 成功数, failed: 失败数, updated: 新增章节总数 }
   */
  async function refreshAllOnShelfView(options: RefreshAllTocOptions = {}): Promise<{
    success: number;
    failed: number;
    updated: number;
  }> {
    const force = options.force === true;
    const result = { success: 0, failed: 0, updated: 0 };

    if (!force && !isAutoUpdateEnabled("onShelfView")) {
      return result;
    }

    for (const book of bookshelfStore.books) {
      if (!canRefreshBook(book)) {
        continue;
      }
      if (!force) {
        if (!isAutoUpdateEnabled("onShelfView")) {
          break;
        }
        if (!isRefreshDue(book.id)) {
          continue;
        }
      }
      const count = await refreshBookToc(book, bookshelfStore, scriptBridgeStore);
      if (count >= 0) {
        result.success++;
        result.updated += count;
      } else {
        result.failed++;
      }
    }

    return result;
  }

  return {
    refreshOnBookOpen,
    refreshAllOnAppStart,
    refreshAllOnShelfView,
  };
}
