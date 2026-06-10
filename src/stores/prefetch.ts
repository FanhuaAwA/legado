import { defineStore } from "pinia";
import { ref } from "vue";
import { isHarmonyNative } from "@/composables/useEnv";
import { eventListen } from "@/composables/useEventBus";
import { invokeWithTimeout } from "@/composables/useInvoke";

export interface PrefetchPayload {
  id: string;
  fileName: string;
  sourceDir?: string;
  bookUrl: string;
  bookName: string;
  sourceType: string;
  chapters: {
    index: number;
    name: string;
    url: string;
    group?: string;
    vip?: boolean;
    price?: unknown;
    currency?: string;
  }[];
  startIndex: number;
  count: number;
  /** 并发数，不填则后端使用配置默认值 */
  concurrency?: number;
}

export interface PrefetchProgressPayload {
  taskId: string;
  done: number;
  total: number;
  chapterIndex: number;
  error?: string;
}

function customEventDetail(ev: Event): unknown {
  return ev instanceof CustomEvent ? ev.detail : null;
}

function parseDonePayload(value: unknown): { taskId: string } | null {
  if (value === null || typeof value !== "object" || !("taskId" in value)) {
    return null;
  }
  const taskId = value.taskId;
  return typeof taskId === "string" ? { taskId } : null;
}

function parseProgressPayload(value: unknown): PrefetchProgressPayload | null {
  if (
    value === null ||
    typeof value !== "object" ||
    !("taskId" in value) ||
    !("done" in value) ||
    !("total" in value) ||
    !("chapterIndex" in value)
  ) {
    return null;
  }
  const taskId = value.taskId;
  const done = value.done;
  const total = value.total;
  const chapterIndex = value.chapterIndex;
  if (
    typeof taskId !== "string" ||
    typeof done !== "number" ||
    typeof total !== "number" ||
    typeof chapterIndex !== "number"
  ) {
    return null;
  }
  const error = "error" in value && typeof value.error === "string" ? value.error : undefined;
  return { taskId, done, total, chapterIndex, error };
}

export const usePrefetchStore = defineStore("prefetch", () => {
  // ── 主动缓存状态 ─────────────────────────────────────────────────────
  const manualRunning = ref(false);
  const manualProgress = ref({ done: 0, total: 0 });
  const manualBookName = ref("");
  const manualTaskId = ref("");

  // ── 静默缓存状态 ─────────────────────────────────────────────────────
  const silentRunning = ref(false);
  const silentTaskId = ref("");

  let _progressUnlisten: (() => void) | null = null;
  let _doneUnlisten: (() => void) | null = null;
  let _silentProgressUnlisten: (() => void) | null = null;
  let _silentDoneUnlisten: (() => void) | null = null;
  let _onChapterCached: ((chapterIndex: number, progress: PrefetchProgressPayload) => void) | null =
    null;
  let _onSilentChapterCached: ((chapterIndex: number) => void) | null = null;

  function cleanupManual() {
    _progressUnlisten?.();
    _progressUnlisten = null;
    _doneUnlisten?.();
    _doneUnlisten = null;
    _onChapterCached = null;
  }

  function cleanupSilent() {
    _silentProgressUnlisten?.();
    _silentProgressUnlisten = null;
    _silentDoneUnlisten?.();
    _silentDoneUnlisten = null;
    _onSilentChapterCached = null;
  }

  /**
   * 按运行环境注册 shelf:prefetch-* 监听（R-P2-011：不得直连 @tauri-apps/api/event）。
   *
   * - 鸿蒙 ArkWeb：Index.ets 的 EventBus 监听器通过 runJavaScript 把进度推送为
   *   shelf:prefetch-* DOM CustomEvent，不经过统一传输层，必须走 DOM 监听。
   * - Tauri / WS：统一事件层（useEventBus → transportListen）。
   *
   * @returns [unlistenProgress, unlistenDone]
   */
  async function setupPrefetchListeners(
    tid: string,
    onProgress: (payload: PrefetchProgressPayload) => void,
    onDone: () => void,
  ): Promise<[() => void, () => void]> {
    if (isHarmonyNative) {
      const progressHandler = (ev: Event) => {
        const payload = parseProgressPayload(customEventDetail(ev));
        if (payload === null || payload.taskId !== tid) {
          return;
        }
        onProgress(payload);
      };
      const doneHandler = (ev: Event) => {
        const payload = parseDonePayload(customEventDetail(ev));
        if (payload === null || payload.taskId !== tid) {
          return;
        }
        onDone();
      };
      window.addEventListener("shelf:prefetch-progress", progressHandler);
      window.addEventListener("shelf:prefetch-done", doneHandler);
      return [
        () => window.removeEventListener("shelf:prefetch-progress", progressHandler),
        () => window.removeEventListener("shelf:prefetch-done", doneHandler),
      ];
    }

    const unlistenProgress = await eventListen<PrefetchProgressPayload>(
      "shelf:prefetch-progress",
      (ev) => {
        if (ev.payload.taskId !== tid) {
          return;
        }
        onProgress(ev.payload);
      },
    );
    const unlistenDone = await eventListen<{ taskId: string }>("shelf:prefetch-done", (ev) => {
      if (ev.payload.taskId !== tid) {
        return;
      }
      onDone();
    });
    return [unlistenProgress, unlistenDone];
  }

  async function setupManualListeners(tid: string) {
    const [unlistenProgress, unlistenDone] = await setupPrefetchListeners(
      tid,
      (payload) => {
        manualProgress.value = { done: payload.done, total: payload.total };
        _onChapterCached?.(payload.chapterIndex, payload);
      },
      () => {
        manualRunning.value = false;
        manualTaskId.value = "";
        cleanupManual();
      },
    );
    _progressUnlisten = unlistenProgress;
    _doneUnlisten = unlistenDone;
  }

  async function setupSilentListeners(tid: string) {
    const [unlistenProgress, unlistenDone] = await setupPrefetchListeners(
      tid,
      (payload) => {
        _onSilentChapterCached?.(payload.chapterIndex);
      },
      () => {
        silentRunning.value = false;
        silentTaskId.value = "";
        cleanupSilent();
      },
    );
    _silentProgressUnlisten = unlistenProgress;
    _silentDoneUnlisten = unlistenDone;
  }

  /**
   * 主动缓存：显示全局进度条，有取消按钮。
   * @param onChapterCached 每章完成时回调
   */
  async function startManualPrefetch(
    payload: PrefetchPayload,
    onChapterCached?: (chapterIndex: number, progress: PrefetchProgressPayload) => void,
  ): Promise<void> {
    if (manualTaskId.value) {
      try {
        await invokeWithTimeout("booksource_cancel", { taskId: manualTaskId.value }, 3000);
      } catch {
        // 忽略
      }
      cleanupManual();
    }

    const tid = `prefetch-manual-${Date.now()}`;
    manualTaskId.value = tid;
    manualRunning.value = true;
    manualProgress.value = { done: 0, total: 0 };
    manualBookName.value = payload.bookName;

    cleanupManual();
    _onChapterCached = onChapterCached ?? null;
    await setupManualListeners(tid);

    try {
      await invokeWithTimeout(
        "bookshelf_prefetch_chapters",
        { payload: { ...payload, taskId: tid } },
        10_000,
      );
    } catch (e) {
      manualRunning.value = false;
      manualTaskId.value = "";
      cleanupManual();
      throw e;
    }
  }

  /** 取消当前主动缓存任务 */
  async function cancelManualPrefetch(): Promise<void> {
    if (!manualTaskId.value) {
      return;
    }
    const tid = manualTaskId.value;
    manualTaskId.value = "";
    manualRunning.value = false;
    cleanupManual();
    try {
      await invokeWithTimeout("booksource_cancel", { taskId: tid }, 3000);
    } catch {
      // 忽略
    }
  }

  /**
   * 静默自动缓存：后台运行，不显示任何 UI。
   * @param onChapterCached 每章完成时回调
   */
  async function startSilentPrefetch(
    payload: PrefetchPayload,
    onChapterCached?: (chapterIndex: number) => void,
  ): Promise<void> {
    if (silentRunning.value) {
      return;
    }

    const tid = `prefetch-auto-${Date.now()}`;
    silentTaskId.value = tid;
    silentRunning.value = true;

    cleanupSilent();
    _onSilentChapterCached = onChapterCached ?? null;
    await setupSilentListeners(tid);

    try {
      await invokeWithTimeout(
        "bookshelf_prefetch_chapters",
        { payload: { ...payload, taskId: tid } },
        10_000,
      );
    } catch {
      silentRunning.value = false;
      silentTaskId.value = "";
      cleanupSilent();
    }
  }

  return {
    manualRunning,
    manualProgress,
    manualBookName,
    manualTaskId,
    silentRunning,
    silentTaskId,
    startManualPrefetch,
    cancelManualPrefetch,
    startSilentPrefetch,
  };
});
