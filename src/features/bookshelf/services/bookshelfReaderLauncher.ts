/**
 * bookshelfReaderLauncher — 书架打开阅读器、加载目录和手动刷新目录的业务入口。
 */

import type { MessageApi } from "naive-ui";
import { useTocAutoUpdate } from "@/composables/useTocAutoUpdate";
import {
  useBookshelfStore,
  // useMusicPlayerStore, // TODO: 音乐功能暂时屏蔽，待启用时取消注释
  useScriptBridgeStore,
  type ChapterItem,
  type ShelfBook,
} from "@/stores";
import { LOCAL_TXT_FILE_NAME } from "@/stores/bookshelf";
import { usePreferencesStore } from "@/stores/preferences";
import { useBookshelfReaderStore } from "../stores/bookshelfReader";
import { useBookshelfUiStore } from "../stores/bookshelfUi";
import { chapterItemsToCachedChapters } from "../utils/readerBookInfo";

interface RawChapterItem {
  name: string;
  url: string;
  group?: string;
  vip?: boolean;
  isVip?: boolean;
  price?: unknown;
  currency?: string;
}

export function useBookshelfReaderLauncher(message: MessageApi) {
  const readerStore = useBookshelfReaderStore();
  const uiStore = useBookshelfUiStore();
  const bookshelfStore = useBookshelfStore();
  const scriptBridgeStore = useScriptBridgeStore();
  // const musicPlayer = useMusicPlayerStore(); // TODO: 音乐功能暂时屏蔽，待启用时取消注释
  const prefStore = usePreferencesStore();
  const tocAutoUpdate = useTocAutoUpdate();

  async function openBook(book: ShelfBook) {
    readerStore.setBookMeta(book);

    try {
      const cached = await bookshelfStore.getChapters(book.id);
      readerStore.setCachedChapters(cached);
    } catch {
      readerStore.setChapters([]);
    }
    // 异步加载集数播放进度（不阻塞打开）
    readerStore.loadEpisodeProgress(book.id).catch(() => {});

    if (!readerStore.readerChapters.length) {
      // 本地 TXT 书籍没有书源，章节列表丢失时直接提示重新导入
      if (book.fileName === LOCAL_TXT_FILE_NAME) {
        message.error("本地书籍章节记录已丢失，请重新导入 TXT 文件");
        return;
      }
      if (!book.bookUrl || !book.fileName) {
        message.warning("无法获取书籍地址，请从发现页重新打开");
        return;
      }
      if (uiStore.openingBookId) {
        return;
      }
      uiStore.openingBookId = book.id;
      try {
        const info = await scriptBridgeStore.runBookInfo(
          book.fileName,
          book.bookUrl,
          book.sourceDir,
        );
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
        readerStore.setChapters(
          fetched.map((chapter) => ({
            name: chapter.name,
            url: chapter.url,
            group: chapter.group,
            vip: chapter.vip,
            price: chapter.price,
            currency: chapter.currency,
          })),
        );
        await bookshelfStore.saveChapters(book.id, fetched);
      } catch (error: unknown) {
        message.error(
          `加载章节目录失败：${error instanceof Error ? error.message : String(error)}`,
        );
        return;
      } finally {
        uiStore.openingBookId = null;
      }
      if (!readerStore.readerChapters.length) {
        message.warning("书源未返回章节列表");
        return;
      }
    }

    const index = book.readChapterIndex >= 0 ? book.readChapterIndex : 0;
    // TODO: 视频/音乐功能暂时屏蔽，待启用时删除此块并取消下方注释
    if (
      !prefStore.devTools.fullModeEnabled &&
      (book.sourceType === "music" || book.sourceType === "video")
    ) {
      message.warning("需要解锁完全体模式后才能使用音频/视频播放");
      return;
    }
    // if (book.sourceType === 'music') {
    //   await musicPlayer.playList(
    //     {
    //       shelfId: book.id,
    //       fileName: book.fileName,
    //       bookUrl: book.bookUrl,
    //       name: book.name,
    //       author: book.author,
    //       coverUrl: book.coverUrl,
    //       intro: book.intro,
    //       sourceName: book.sourceName,
    //     },
    //     readerStore.readerChapters.map((c) => ({ chapterUrl: c.url, name: c.name })),
    //     index,
    //   );
    //   return;
    // }
    readerStore.openAt(index);

    // 后台静默检测目录更新（不阻塞打开，不弹任何消息）
    tocAutoUpdate.refreshOnBookOpen(book).then((newCount) => {
      if (newCount > 0) {
        // 更新阅读器中的章节列表
        bookshelfStore.getChapters(book.id).then((cached) => {
          if (readerStore.readerShelfId === book.id) {
            readerStore.setCachedChapters(cached);
          }
        });
      }
    });
  }

  async function refreshToc() {
    if (readerStore.refreshingToc) {
      return;
    }
    const bookUrl = readerStore.readerBookInfo?.bookUrl;
    const fileName = readerStore.readerFileName;
    const sourceDir = readerStore.readerSourceDir || readerStore.readerBookInfo?.sourceDir;
    if (!bookUrl || !fileName || !readerStore.readerShelfId) {
      message.warning("无法获取书籍地址，请先确保书籍已加入书架");
      return;
    }
    readerStore.refreshingToc = true;
    bookshelfStore.beginTocRefresh(readerStore.readerShelfId);
    try {
      const info = await scriptBridgeStore.runBookInfo(fileName, bookUrl, sourceDir);
      const tocUrl = (info as { tocUrl?: string }).tocUrl ?? bookUrl;
      const raw = await scriptBridgeStore.runChapterList(fileName, tocUrl, undefined, sourceDir);
      const fetched = (raw as RawChapterItem[]).map((chapter, index) => ({
        index,
        name: chapter.name,
        url: chapter.url,
        group: chapter.group,
        vip: chapter.vip ?? chapter.isVip,
        price: chapter.price,
        currency: chapter.currency,
      }));
      const oldUrls = new Set(readerStore.readerChapters.map((chapter) => chapter.url));
      const newCount = fetched.filter((chapter) => !oldUrls.has(chapter.url)).length;
      readerStore.setChapters(
        fetched.map((chapter) => ({
          name: chapter.name,
          url: chapter.url,
          group: chapter.group,
          vip: chapter.vip,
          price: chapter.price,
          currency: chapter.currency,
        })),
      );
      await bookshelfStore.saveChapters(readerStore.readerShelfId, fetched);
      if (newCount > 0) {
        message.success(`目录已更新，新增 ${newCount} 章`);
      } else {
        message.info("目录已是最新，无新章节");
      }
    } catch (error: unknown) {
      message.error(`更新目录失败：${error instanceof Error ? error.message : String(error)}`);
    } finally {
      bookshelfStore.endTocRefresh(readerStore.readerShelfId);
      readerStore.refreshingToc = false;
    }
  }

  function syncOpenReaderBookInfo(bookId: string) {
    readerStore.syncOpenReaderBookInfo(bookshelfStore.books.find((book) => book.id === bookId));
  }

  function currentChaptersForSwitch(targetBook: ShelfBook | null): ChapterItem[] {
    return readerStore.readerShelfId === targetBook?.id
      ? readerStore.readerChapters
      : uiStore.switchTargetChapters;
  }

  function saveReaderChaptersToPayload() {
    return chapterItemsToCachedChapters(readerStore.readerChapters);
  }

  return {
    openBook,
    refreshToc,
    syncOpenReaderBookInfo,
    currentChaptersForSwitch,
    saveReaderChaptersToPayload,
  };
}
