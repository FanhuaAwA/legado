import { computed, readonly, ref } from "vue";
import { useCapabilities } from "./useCapabilities";
import { useDynamicConfig } from "./useDynamicConfig";
import { eventListen } from "./useEventBus";
import {
  useFrontendPlugins,
  type FrontendTtsEngineRecord,
  type TtsSpeakContext,
  type TtsVoiceDefinition,
} from "./useFrontendPlugins";
import { invokeWithTimeout } from "./useInvoke";

const SYSTEM_ENGINE_ID = "system";
const QUEUE_LOAD_AHEAD = 6;
const TTS_PRELOAD_AHEAD = 2;
export const TTS_MAX_PARAGRAPH_CHARS = 300;
const COMMAND_TIMEOUT_MS = 12_000;
const SYSTEM_TTS_READY_TIMEOUT_MS = 30_000;
const SYSTEM_TTS_SPEAK_TIMEOUT_MS = 35_000;
const POLL_INTERVAL_MS = 180;
const READY_POLL_INTERVAL_MS = 300;
const SETTINGS_KEY = "legado.tts.settings";
const SETTINGS_NAMESPACE = "reader.tts";

export interface TtsOptions {
  engineId?: string;
  voiceId?: string;
  voice?: string;
  language?: string;
  rate?: number;
  volume?: number;
  pitch?: number;
}

export interface TtsStartOptions extends TtsOptions {
  initialSegments: string[];
  onNeedMore?: () => Promise<string[] | null>;
  onSegmentStart?: (globalIdx: number) => void;
  onAllDone?: () => void;
}

export interface TtsVoice {
  id: string;
  name: string;
  language?: string;
}

export interface TtsEngineRecord {
  id: string;
  name: string;
  description: string;
  category: string;
  source: "system" | "plugin";
  pluginId?: string;
  fileName?: string;
}

interface QueueItem {
  text: string;
  globalIdx: number;
}

interface TtsPreloadEntry {
  globalIdx: number;
  key: string;
  controller: AbortController;
  promise: Promise<unknown>;
  loaded: boolean;
  value?: unknown;
}

interface TtsPreparedValue {
  loaded: boolean;
  value?: unknown;
}

interface PersistedTtsSettings {
  engineId?: string;
  voiceId?: string;
  playbackRate?: number;
  volume?: number;
  pitch?: number;
}

interface SystemTtsSpeakingState {
  speaking: boolean;
}

interface SystemTtsInitializedState {
  initialized: boolean;
  voiceCount: number;
}

interface SystemTtsEventPayload {
  eventType?: string;
  id?: string;
  error?: string;
  interrupted?: boolean;
  reason?: string;
}

interface SystemTtsSpeakPayload {
  text: string;
  language?: string;
  voiceId?: string;
  rate: number;
  pitch: number;
  volume: number;
  queueMode: "flush" | "add";
}

const frontendPlugins = useFrontendPlugins();
const appCapabilities = useCapabilities();
const nativeTtsCapability = appCapabilities.getCapability("tts");
void appCapabilities.loadCapabilities();

const systemEngine: TtsEngineRecord = {
  id: SYSTEM_ENGINE_ID,
  name: "系统 TTS",
  description: "使用操作系统原生朗读引擎",
  category: "内置",
  source: "system",
};

function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, value));
}

function readNonEmptyString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function normalizePersistedSettings(value: unknown): PersistedTtsSettings | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const record = value as PersistedTtsSettings;
  return {
    engineId: readNonEmptyString(record.engineId) ?? SYSTEM_ENGINE_ID,
    voiceId: readNonEmptyString(record.voiceId) ?? "",
    playbackRate: clamp(Number(record.playbackRate ?? 1), 0.5, 2),
    volume: clamp(Number(record.volume ?? 1), 0, 1),
    pitch: clamp(Number(record.pitch ?? 1), 0.5, 2),
  };
}

const ttsSettingsStore = useDynamicConfig<PersistedTtsSettings>({
  namespace: SETTINGS_NAMESPACE,
  version: 1,
  defaults: () => ({
    engineId: SYSTEM_ENGINE_ID,
    voiceId: "",
    playbackRate: 1,
    volume: 1,
    pitch: 1,
  }),
  migrate: ({ readLegacy }) => {
    const raw = readLegacy(SETTINGS_KEY);
    if (!raw) {
      return null;
    }
    try {
      return normalizePersistedSettings(JSON.parse(raw));
    } catch {
      return null;
    }
  },
  legacyKeys: [SETTINGS_KEY],
});

function normalizePluginEngine(record: FrontendTtsEngineRecord): TtsEngineRecord {
  return {
    id: record.id,
    name: record.name,
    description: record.description,
    category: record.category,
    source: "plugin",
    pluginId: record.pluginId,
    fileName: record.fileName,
  };
}

function nonEmpty(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed) {
    return undefined;
  }
  return trimmed;
}

function delay(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal?.aborted) {
      resolve();
      return;
    }
    const timer = window.setTimeout(resolve, ms);
    signal?.addEventListener(
      "abort",
      () => {
        window.clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
  });
}

function splitLongParagraph(paragraph: string): string[] {
  const text = paragraph.trim();
  if (!text) {
    return [];
  }
  if (text.length <= TTS_MAX_PARAGRAPH_CHARS) {
    return [text];
  }

  const segments: string[] = [];
  let remaining = text;
  while (remaining.length > TTS_MAX_PARAGRAPH_CHARS) {
    let cutAt = TTS_MAX_PARAGRAPH_CHARS;
    for (
      let index = TTS_MAX_PARAGRAPH_CHARS - 1;
      index > Math.floor(TTS_MAX_PARAGRAPH_CHARS * 0.55);
      index--
    ) {
      if (/[。！？…；.!?;,，、]/.test(remaining[index] ?? "")) {
        cutAt = index + 1;
        break;
      }
    }

    const segment = remaining.slice(0, cutAt).trim();
    if (segment) {
      segments.push(segment);
    }
    remaining = remaining.slice(cutAt).trim();
  }
  if (remaining) {
    segments.push(remaining);
  }
  return segments;
}

export function splitIntoSegments(text: string): string[] {
  if (!text.trim()) {
    return [];
  }
  return text
    .split(/\r?\n+/)
    .flatMap((paragraph) => splitLongParagraph(paragraph))
    .filter(Boolean);
}

function getBrowserSpeechVoices(): TtsVoice[] {
  if (!("speechSynthesis" in window)) {
    return [];
  }
  return window.speechSynthesis.getVoices().map((voice) => ({
    id: voice.voiceURI || voice.name,
    name: voice.name,
    language: voice.lang,
  }));
}

async function waitForBrowserVoices(): Promise<TtsVoice[]> {
  const voices = getBrowserSpeechVoices();
  if (voices.length > 0 || !("speechSynthesis" in window)) {
    return voices;
  }
  return new Promise((resolve) => {
    const timer = window.setTimeout(() => resolve(getBrowserSpeechVoices()), 800);
    window.speechSynthesis.onvoiceschanged = () => {
      window.clearTimeout(timer);
      resolve(getBrowserSpeechVoices());
    };
  });
}

async function stopSystemSpeech(): Promise<void> {
  if ("speechSynthesis" in window) {
    window.speechSynthesis.cancel();
  }
  await appCapabilities.loadCapabilities();
  if (!nativeTtsCapability.value.supported) {
    return;
  }
  try {
    await invokeWithTimeout<void>("tts_stop", undefined, COMMAND_TIMEOUT_MS);
  } catch {
    // Browser/Web fallback may not have the native command available.
  }
}

async function waitForSystemTtsReady(signal: AbortSignal): Promise<void> {
  const startedAt = Date.now();

  while (!signal.aborted) {
    const state = await invokeWithTimeout<SystemTtsInitializedState>(
      "tts_is_initialized",
      undefined,
      COMMAND_TIMEOUT_MS,
    );
    if (state.initialized) {
      return;
    }
    if (Date.now() - startedAt >= SYSTEM_TTS_READY_TIMEOUT_MS) {
      throw new Error("系统 TTS 初始化超时，请确认系统语音引擎已安装并启用");
    }
    await delay(READY_POLL_INTERVAL_MS, signal);
  }
  throw new Error("系统 TTS 初始化已取消");
}

function waitForSystemSpeechDone(signal: AbortSignal, text: string): Promise<void> {
  const startedAt = Date.now();
  const estimatedMs = clamp(text.length * 180, 1_000, 45_000);

  return new Promise((resolve, reject) => {
    let resolved = false;
    let sawSpeaking = false;
    let pollTimer: number | null = null;
    const unlisteners: Array<() => void> = [];

    const cleanup = () => {
      if (pollTimer !== null) {
        window.clearTimeout(pollTimer);
        pollTimer = null;
      }
      signal.removeEventListener("abort", finish);
      for (const unlisten of unlisteners.splice(0)) {
        unlisten();
      }
    };

    function finish() {
      if (resolved) {
        return;
      }
      resolved = true;
      cleanup();
      resolve();
    }

    function fail(message: string) {
      if (resolved) {
        return;
      }
      resolved = true;
      cleanup();
      reject(new Error(message));
    }

    async function registerSpeechEvent(
      eventName: string,
      handler: (payload: SystemTtsEventPayload) => void,
    ) {
      try {
        const unlisten = await eventListen<SystemTtsEventPayload>(eventName, (event) => {
          handler(event.payload ?? {});
        });
        if (resolved) {
          unlisten();
        } else {
          unlisteners.push(unlisten);
        }
      } catch {
        // Some browser/Web fallback modes do not provide native TTS events.
      }
    }

    async function pollSpeakingState() {
      if (resolved || signal.aborted) {
        finish();
        return;
      }

      try {
        const state = await invokeWithTimeout<SystemTtsSpeakingState>(
          "tts_is_speaking",
          undefined,
          COMMAND_TIMEOUT_MS,
        );
        if (state.speaking) {
          sawSpeaking = true;
        }
        if (sawSpeaking && !state.speaking) {
          finish();
          return;
        }
      } catch {
        // Keep the estimated timer path alive when polling is unavailable.
      }

      if (!sawSpeaking && Date.now() - startedAt > estimatedMs) {
        finish();
        return;
      }
      pollTimer = window.setTimeout(pollSpeakingState, POLL_INTERVAL_MS);
    }

    signal.addEventListener("abort", finish, { once: true });
    void registerSpeechEvent("tts://speech:finish", finish);
    void registerSpeechEvent("tts://speech:cancel", finish);
    void registerSpeechEvent("tts://speech:error", (payload) => {
      fail(payload.error ?? "系统 TTS 朗读失败");
    });
    pollTimer = window.setTimeout(pollSpeakingState, POLL_INTERVAL_MS);
  });
}

async function speakWithBrowserSpeech(
  text: string,
  options: Required<Pick<TtsOptions, "rate" | "volume" | "pitch">> & {
    voiceId?: string;
    language?: string;
  },
  signal: AbortSignal,
): Promise<void> {
  if (!("speechSynthesis" in window)) {
    throw new Error("当前环境没有可用的系统朗读引擎");
  }

  await waitForBrowserVoices();
  if (signal.aborted) {
    return;
  }

  await new Promise<void>((resolve, reject) => {
    const utterance = new SpeechSynthesisUtterance(text);
    const voices = window.speechSynthesis.getVoices();
    const voice = voices.find(
      (item) => item.voiceURI === options.voiceId || item.name === options.voiceId,
    );
    if (voice) {
      utterance.voice = voice;
      utterance.lang = voice.lang;
    } else if (options.language) {
      utterance.lang = options.language;
    }
    utterance.rate = clamp(options.rate, 0.1, 4);
    utterance.volume = clamp(options.volume, 0, 1);
    utterance.pitch = clamp(options.pitch, 0.5, 2);

    const cleanup = () => {
      utterance.onend = null;
      utterance.onerror = null;
    };
    utterance.onend = () => {
      cleanup();
      resolve();
    };
    utterance.onerror = (event) => {
      cleanup();
      reject(new Error(`浏览器朗读失败: ${event.error}`));
    };
    signal.addEventListener(
      "abort",
      () => {
        cleanup();
        window.speechSynthesis.cancel();
        resolve();
      },
      { once: true },
    );
    window.speechSynthesis.cancel();
    window.speechSynthesis.speak(utterance);
  });
}

async function speakWithSystemEngine(text: string, context: TtsSpeakContext): Promise<void> {
  await appCapabilities.loadCapabilities();
  if (!nativeTtsCapability.value.supported) {
    await speakWithBrowserSpeech(text, context, context.signal);
    return;
  }

  const payload: SystemTtsSpeakPayload = {
    text,
    language: context.language,
    voiceId: context.voiceId,
    rate: context.rate,
    pitch: context.pitch,
    volume: context.volume,
    queueMode: "flush",
  };

  const waitController = new AbortController();
  const abortWait = () => waitController.abort();
  context.signal.addEventListener("abort", abortWait, { once: true });

  try {
    await waitForSystemTtsReady(context.signal);
    const done = waitForSystemSpeechDone(waitController.signal, text);
    await invokeWithTimeout("tts_speak", { payload }, SYSTEM_TTS_SPEAK_TIMEOUT_MS);
    await done;
  } catch (error) {
    waitController.abort();
    if (context.signal.aborted) {
      return;
    }
    await speakWithBrowserSpeech(text, context, context.signal).catch(() => {
      throw error;
    });
  } finally {
    context.signal.removeEventListener("abort", abortWait);
  }
}

function normalizePluginVoices(value: TtsVoiceDefinition[]): TtsVoice[] {
  return value.map((voice) => ({
    id: voice.id,
    name: voice.name,
    language: voice.language,
  }));
}

const isPlaying = ref(false);
const isLoading = ref(false);
const hasSession = ref(false);
const playbackRate = computed({
  get: () => clamp(ttsSettingsStore.state.playbackRate ?? 1, 0.5, 2),
  set: (value: number) => {
    ttsSettingsStore.state.playbackRate = clamp(value, 0.5, 2);
  },
});
const volume = computed({
  get: () => clamp(ttsSettingsStore.state.volume ?? 1, 0, 1),
  set: (value: number) => {
    ttsSettingsStore.state.volume = clamp(value, 0, 1);
  },
});
const pitch = computed({
  get: () => clamp(ttsSettingsStore.state.pitch ?? 1, 0.5, 2),
  set: (value: number) => {
    ttsSettingsStore.state.pitch = clamp(value, 0.5, 2);
  },
});
const error = ref<string | null>(null);
const currentGlobalIdx = ref(-1);
const currentSegmentIndex = ref(0);
const totalSegmentsKnown = ref(0);
const totalFinalized = ref(false);
const currentSegmentText = ref("");
const selectedEngineId = computed({
  get: () => nonEmpty(ttsSettingsStore.state.engineId) ?? SYSTEM_ENGINE_ID,
  set: (engineId: string) => {
    ttsSettingsStore.state.engineId = nonEmpty(engineId) ?? SYSTEM_ENGINE_ID;
  },
});
const selectedVoiceId = computed({
  get: () => nonEmpty(ttsSettingsStore.state.voiceId) ?? "",
  set: (voiceId: string) => {
    ttsSettingsStore.state.voiceId = nonEmpty(voiceId) ?? "";
  },
});
const availableVoices = ref<TtsVoice[]>([]);
const voicesLoading = ref(false);

const availableEngines = computed<TtsEngineRecord[]>(() => [
  systemEngine,
  ...frontendPlugins.ttsEngines.value.map(normalizePluginEngine),
]);
const selectedEngine = computed(
  () =>
    availableEngines.value.find((engine) => engine.id === selectedEngineId.value) ?? systemEngine,
);
const currentSegmentOrdinal = computed(() =>
  totalSegmentsKnown.value > 0
    ? Math.min(currentSegmentIndex.value + 1, totalSegmentsKnown.value)
    : 0,
);
const progressRatio = computed(() => {
  if (totalSegmentsKnown.value <= 0) {
    return 0;
  }
  return currentSegmentOrdinal.value / totalSegmentsKnown.value;
});

const items: QueueItem[] = [];
let nextGlobalIdx = 0;
let playIndex = 0;
let activeOptions: TtsStartOptions | null = null;
let loadingMore = false;
let endReached = false;
let loadMorePromise: Promise<void> | null = null;
let currentAbort: AbortController | null = null;
let playGeneration = 0;
const ttsPreloadEntries = new Map<number, TtsPreloadEntry>();
const ttsPreloadUnsupportedEngineIds = new Set<string>();

function saveCurrentSettings(): void {
  void ttsSettingsStore.replace({
    engineId: selectedEngineId.value,
    voiceId: selectedVoiceId.value,
    playbackRate: playbackRate.value,
    volume: volume.value,
    pitch: pitch.value,
  });
}

function syncProgressRefs(): void {
  currentSegmentIndex.value = Math.max(0, playIndex);
  totalSegmentsKnown.value = items.length;
  totalFinalized.value = endReached;
}

function appendSegments(segments: string[]): void {
  for (const text of segments) {
    const trimmed = text.trim();
    if (trimmed) {
      items.push({ text: trimmed, globalIdx: nextGlobalIdx++ });
    }
  }
  syncProgressRefs();
}

async function loadMore(): Promise<void> {
  if (loadingMore || endReached || !activeOptions?.onNeedMore) {
    return loadMorePromise ?? Promise.resolve();
  }

  loadingMore = true;
  isLoading.value = true;
  loadMorePromise = (async () => {
    try {
      const more = await activeOptions?.onNeedMore?.();
      if (more && more.length > 0) {
        appendSegments(more);
        if (hasSession.value && isPlaying.value) {
          scheduleTtsPreloadBuffer(playGeneration);
        }
      } else {
        endReached = true;
        syncProgressRefs();
      }
    } finally {
      loadingMore = false;
      isLoading.value = false;
      loadMorePromise = null;
    }
  })();
  return loadMorePromise;
}

function maybeLoadMore(): void {
  if (!activeOptions?.onNeedMore || endReached || loadingMore) {
    return;
  }
  if (items.length - playIndex <= QUEUE_LOAD_AHEAD) {
    void loadMore();
  }
}

async function ensurePlayableItem(generation: number): Promise<QueueItem | null> {
  for (;;) {
    if (generation !== playGeneration || !hasSession.value) {
      return null;
    }
    if (playIndex < items.length) {
      break;
    }
    if (endReached || !activeOptions?.onNeedMore) {
      return null;
    }
    await loadMore();
  }
  return items[playIndex] ?? null;
}

function interruptCurrentSpeech(): void {
  currentAbort?.abort();
  currentAbort = null;
  const engineId = selectedEngine.value.id;
  if (engineId === SYSTEM_ENGINE_ID) {
    void stopSystemSpeech();
  } else {
    void frontendPlugins.stopTtsEngine(engineId).catch(() => {});
  }
}

function buildSpeakContext(
  text: string,
  signal: AbortSignal,
  preloaded?: unknown,
): TtsSpeakContext {
  const voice = availableVoices.value.find((item) => item.id === selectedVoiceId.value);
  const context: TtsSpeakContext = {
    text,
    voiceId:
      nonEmpty(selectedVoiceId.value) ??
      nonEmpty(activeOptions?.voiceId) ??
      nonEmpty(activeOptions?.voice),
    language: nonEmpty(activeOptions?.language) ?? nonEmpty(voice?.language) ?? "zh-CN",
    rate: playbackRate.value,
    pitch: pitch.value,
    volume: volume.value,
    signal,
  };
  if (preloaded !== undefined) {
    context.preloaded = preloaded;
  }
  return context;
}

function buildTtsPreloadKey(item: QueueItem): string {
  const voice = availableVoices.value.find((candidate) => candidate.id === selectedVoiceId.value);
  return JSON.stringify({
    engineId: selectedEngine.value.id,
    voiceId:
      nonEmpty(selectedVoiceId.value) ??
      nonEmpty(activeOptions?.voiceId) ??
      nonEmpty(activeOptions?.voice) ??
      "",
    language: nonEmpty(activeOptions?.language) ?? nonEmpty(voice?.language) ?? "zh-CN",
    rate: playbackRate.value,
    pitch: pitch.value,
    text: item.text,
  });
}

function abortTtsPreloadEntry(entry: TtsPreloadEntry): void {
  entry.controller.abort();
}

function clearTtsPreloadCache(): void {
  for (const entry of ttsPreloadEntries.values()) {
    abortTtsPreloadEntry(entry);
  }
  ttsPreloadEntries.clear();
}

function abortPendingTtsPreloads(): void {
  for (const [globalIdx, entry] of ttsPreloadEntries) {
    if (!entry.loaded) {
      abortTtsPreloadEntry(entry);
      ttsPreloadEntries.delete(globalIdx);
    }
  }
}

function pruneTtsPreloadCache(): void {
  const activeWindow = new Set(
    items.slice(playIndex, playIndex + TTS_PRELOAD_AHEAD + 1).map((item) => item.globalIdx),
  );
  for (const [globalIdx, entry] of ttsPreloadEntries) {
    if (!activeWindow.has(globalIdx)) {
      abortTtsPreloadEntry(entry);
      ttsPreloadEntries.delete(globalIdx);
    }
  }
}

function ensureTtsPreloadEntry(item: QueueItem, generation: number): TtsPreloadEntry | null {
  const engineId = selectedEngine.value.id;
  if (engineId === SYSTEM_ENGINE_ID || ttsPreloadUnsupportedEngineIds.has(engineId)) {
    return null;
  }

  const key = buildTtsPreloadKey(item);
  const existing = ttsPreloadEntries.get(item.globalIdx);
  if (existing?.key === key) {
    return existing;
  }
  if (existing) {
    abortTtsPreloadEntry(existing);
    ttsPreloadEntries.delete(item.globalIdx);
  }

  const controller = new AbortController();
  const context = buildSpeakContext(item.text, controller.signal);
  const entry: TtsPreloadEntry = {
    globalIdx: item.globalIdx,
    key,
    controller,
    loaded: false,
    promise: Promise.resolve(undefined),
  };

  entry.promise = frontendPlugins
    .preloadTtsEngine(engineId, context)
    .then((result) => {
      if (!result.supported) {
        ttsPreloadUnsupportedEngineIds.add(engineId);
        if (ttsPreloadEntries.get(item.globalIdx) === entry) {
          ttsPreloadEntries.delete(item.globalIdx);
        }
        return undefined;
      }
      if (controller.signal.aborted || generation !== playGeneration || !hasSession.value) {
        return undefined;
      }
      entry.loaded = true;
      entry.value = result.value;
      return result.value;
    })
    .catch(() => undefined);

  ttsPreloadEntries.set(item.globalIdx, entry);
  return entry;
}

async function getPreparedTtsValue(item: QueueItem, generation: number): Promise<TtsPreparedValue> {
  const entry = ensureTtsPreloadEntry(item, generation);
  if (!entry) {
    return { loaded: false };
  }

  await entry.promise;
  if (generation !== playGeneration || !hasSession.value || !entry.loaded) {
    return { loaded: false };
  }
  return { loaded: true, value: entry.value };
}

function scheduleTtsPreloadBuffer(generation: number): void {
  if (generation !== playGeneration || !hasSession.value || !isPlaying.value) {
    return;
  }

  maybeLoadMore();
  pruneTtsPreloadCache();
  for (let offset = 1; offset <= TTS_PRELOAD_AHEAD; offset++) {
    const item = items[playIndex + offset];
    if (item) {
      ensureTtsPreloadEntry(item, generation);
    }
  }
}

async function speakCurrentItem(
  item: QueueItem,
  signal: AbortSignal,
  generation: number,
): Promise<void> {
  currentSegmentText.value = item.text;

  if (selectedEngine.value.id === SYSTEM_ENGINE_ID) {
    const context = buildSpeakContext(item.text, signal);
    await speakWithSystemEngine(item.text, context);
    return;
  }

  const prepared = await getPreparedTtsValue(item, generation);
  if (prepared.loaded) {
    scheduleTtsPreloadBuffer(generation);
  }
  const context = buildSpeakContext(
    item.text,
    signal,
    prepared.loaded ? prepared.value : undefined,
  );
  await frontendPlugins.speakWithTtsEngine(selectedEngine.value.id, context);
}

function finishAll(): void {
  isPlaying.value = false;
  hasSession.value = false;
  currentGlobalIdx.value = -1;
  currentSegmentText.value = "";
  activeOptions?.onAllDone?.();
}

async function playLoop(generation: number): Promise<void> {
  for (;;) {
    if (generation !== playGeneration || !hasSession.value || !isPlaying.value) {
      return;
    }
    const item = await ensurePlayableItem(generation);
    if (!item) {
      if (generation === playGeneration && hasSession.value) {
        finishAll();
      }
      return;
    }

    currentGlobalIdx.value = item.globalIdx;
    playIndex = items.findIndex((candidate) => candidate.globalIdx === item.globalIdx);
    if (playIndex < 0) {
      playIndex = item.globalIdx;
    }
    syncProgressRefs();
    pruneTtsPreloadCache();
    activeOptions?.onSegmentStart?.(item.globalIdx);
    maybeLoadMore();

    const controller = new AbortController();
    currentAbort = controller;
    const loadingTimer = window.setTimeout(() => {
      if (generation === playGeneration && currentAbort === controller) {
        isLoading.value = false;
      }
    }, 450);

    try {
      isLoading.value = true;
      error.value = null;
      await speakCurrentItem(item, controller.signal, generation);
    } catch (err) {
      if (!controller.signal.aborted && generation === playGeneration) {
        error.value = err instanceof Error ? err.message : String(err);
        isPlaying.value = false;
        return;
      }
    } finally {
      window.clearTimeout(loadingTimer);
      if (currentAbort === controller) {
        currentAbort = null;
      }
      isLoading.value = false;
    }

    if (controller.signal.aborted || generation !== playGeneration || !isPlaying.value) {
      return;
    }
    playIndex += 1;
    syncProgressRefs();
    scheduleTtsPreloadBuffer(generation);
  }
}

function restartPlaybackIfNeeded(wasPlaying: boolean): void {
  if (!hasSession.value) {
    return;
  }
  if (!wasPlaying) {
    syncProgressRefs();
    const item = items[playIndex];
    if (item) {
      currentGlobalIdx.value = item.globalIdx;
      activeOptions?.onSegmentStart?.(item.globalIdx);
    }
    return;
  }
  isPlaying.value = true;
  const generation = ++playGeneration;
  void playLoop(generation);
}

async function refreshEngines(): Promise<void> {
  await frontendPlugins.ensureInitialized();
  ttsPreloadUnsupportedEngineIds.clear();
  if (!availableEngines.value.some((engine) => engine.id === selectedEngineId.value)) {
    selectedEngineId.value = SYSTEM_ENGINE_ID;
    selectedVoiceId.value = "";
    saveCurrentSettings();
  }
}

async function loadVoices(): Promise<void> {
  voicesLoading.value = true;
  try {
    await refreshEngines();
    if (selectedEngine.value.id === SYSTEM_ENGINE_ID) {
      await appCapabilities.loadCapabilities();
      if (!nativeTtsCapability.value.supported) {
        availableVoices.value = await waitForBrowserVoices();
      } else {
        try {
          const voices = await invokeWithTimeout<TtsVoice[]>(
            "tts_get_voices",
            { language: undefined },
            COMMAND_TIMEOUT_MS,
          );
          availableVoices.value = voices;
        } catch {
          availableVoices.value = await waitForBrowserVoices();
        }
      }
    } else {
      availableVoices.value = normalizePluginVoices(
        await frontendPlugins.getTtsEngineVoices(selectedEngine.value.id),
      );
    }

    if (
      selectedVoiceId.value &&
      !availableVoices.value.some((voice) => voice.id === selectedVoiceId.value)
    ) {
      selectedVoiceId.value = "";
      saveCurrentSettings();
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
    availableVoices.value = [];
  } finally {
    voicesLoading.value = false;
  }
}

function startReading(options: TtsStartOptions): void {
  stop();
  activeOptions = options;
  if (options.engineId) {
    selectedEngineId.value = options.engineId;
  }
  const nextVoiceId = nonEmpty(options.voiceId) ?? nonEmpty(options.voice);
  if (nextVoiceId) {
    selectedVoiceId.value = nextVoiceId;
  }
  if (typeof options.rate === "number") {
    playbackRate.value = clamp(options.rate, 0.5, 2);
  }
  if (typeof options.volume === "number") {
    volume.value = clamp(options.volume, 0, 1);
  }
  if (typeof options.pitch === "number") {
    pitch.value = clamp(options.pitch, 0.5, 2);
  }
  saveCurrentSettings();

  nextGlobalIdx = 0;
  playIndex = 0;
  endReached = false;
  items.length = 0;
  clearTtsPreloadCache();
  appendSegments(options.initialSegments);
  hasSession.value = true;
  isPlaying.value = true;
  error.value = null;
  void loadVoices();
  const generation = ++playGeneration;
  void playLoop(generation);
}

function play(): void {
  if (!hasSession.value || isPlaying.value) {
    return;
  }
  isPlaying.value = true;
  const generation = ++playGeneration;
  void playLoop(generation);
}

function pause(): void {
  if (!isPlaying.value) {
    return;
  }
  isPlaying.value = false;
  playGeneration += 1;
  interruptCurrentSpeech();
  abortPendingTtsPreloads();
}

function stop(): void {
  playGeneration += 1;
  interruptCurrentSpeech();
  clearTtsPreloadCache();
  isPlaying.value = false;
  isLoading.value = false;
  hasSession.value = false;
  error.value = null;
  currentGlobalIdx.value = -1;
  currentSegmentIndex.value = 0;
  totalSegmentsKnown.value = 0;
  totalFinalized.value = false;
  currentSegmentText.value = "";
  activeOptions = null;
  loadingMore = false;
  endReached = false;
  loadMorePromise = null;
  nextGlobalIdx = 0;
  playIndex = 0;
  items.length = 0;
}

function nextSegment(): void {
  if (!hasSession.value) {
    return;
  }
  const wasPlaying = isPlaying.value;
  isPlaying.value = false;
  playGeneration += 1;
  interruptCurrentSpeech();
  abortPendingTtsPreloads();
  playIndex = endReached ? Math.min(playIndex + 1, Math.max(items.length - 1, 0)) : playIndex + 1;
  pruneTtsPreloadCache();
  restartPlaybackIfNeeded(wasPlaying);
}

function prevSegment(): void {
  if (!hasSession.value) {
    return;
  }
  const wasPlaying = isPlaying.value;
  isPlaying.value = false;
  playGeneration += 1;
  interruptCurrentSpeech();
  abortPendingTtsPreloads();
  playIndex = Math.max(0, playIndex - 1);
  pruneTtsPreloadCache();
  restartPlaybackIfNeeded(wasPlaying);
}

function seekToIndex(index: number): void {
  if (!hasSession.value || items.length === 0) {
    return;
  }
  const wasPlaying = isPlaying.value;
  isPlaying.value = false;
  playGeneration += 1;
  interruptCurrentSpeech();
  abortPendingTtsPreloads();
  playIndex = Math.max(0, Math.min(Math.round(index), items.length - 1));
  pruneTtsPreloadCache();
  restartPlaybackIfNeeded(wasPlaying);
}

function setPlaybackRate(rate: number): void {
  playbackRate.value = clamp(rate, 0.5, 2);
  saveCurrentSettings();
  if (isPlaying.value) {
    const index = playIndex;
    pause();
    clearTtsPreloadCache();
    playIndex = index;
    play();
  } else {
    clearTtsPreloadCache();
  }
}

function setVolume(nextVolume: number): void {
  volume.value = clamp(nextVolume, 0, 1);
  saveCurrentSettings();
}

function setPitch(nextPitch: number): void {
  pitch.value = clamp(nextPitch, 0.5, 2);
  saveCurrentSettings();
  clearTtsPreloadCache();
}

async function setEngineId(engineId: string): Promise<void> {
  await refreshEngines();
  const nextEngineId = availableEngines.value.some((engine) => engine.id === engineId)
    ? engineId
    : SYSTEM_ENGINE_ID;
  if (selectedEngineId.value === nextEngineId) {
    return;
  }

  const wasPlaying = isPlaying.value;
  isPlaying.value = false;
  playGeneration += 1;
  interruptCurrentSpeech();
  clearTtsPreloadCache();
  selectedEngineId.value = nextEngineId;
  selectedVoiceId.value = "";
  saveCurrentSettings();
  await loadVoices();
  restartPlaybackIfNeeded(wasPlaying);
}

function setVoiceId(voiceId: string): void {
  selectedVoiceId.value = voiceId;
  clearTtsPreloadCache();
  saveCurrentSettings();
}

async function previewVoice(voiceId = selectedVoiceId.value): Promise<void> {
  if (!voiceId) {
    return;
  }
  if (selectedEngine.value.id === SYSTEM_ENGINE_ID) {
    await appCapabilities.loadCapabilities();
    if (!nativeTtsCapability.value.supported) {
      await speakWithBrowserSpeech(
        "这是一段系统朗读试听。",
        {
          voiceId,
          language: availableVoices.value.find((voice) => voice.id === voiceId)?.language,
          rate: playbackRate.value,
          volume: volume.value,
          pitch: pitch.value,
        },
        new AbortController().signal,
      );
      return;
    }
    try {
      await invokeWithTimeout(
        "tts_preview_voice",
        { voiceId, text: "这是一段系统朗读试听。" },
        COMMAND_TIMEOUT_MS,
      );
    } catch {
      await speakWithBrowserSpeech(
        "这是一段系统朗读试听。",
        {
          voiceId,
          language: availableVoices.value.find((voice) => voice.id === voiceId)?.language,
          rate: playbackRate.value,
          volume: volume.value,
          pitch: pitch.value,
        },
        new AbortController().signal,
      );
    }
    return;
  }
  await frontendPlugins.previewTtsEngineVoice(selectedEngine.value.id, voiceId);
}

export function useTts() {
  return {
    isPlaying: readonly(isPlaying),
    isLoading: readonly(isLoading),
    hasSession: readonly(hasSession),
    playbackRate: readonly(playbackRate),
    volume: readonly(volume),
    pitch: readonly(pitch),
    currentGlobalIdx: readonly(currentGlobalIdx),
    currentSegmentIndex: readonly(currentSegmentIndex),
    currentSegmentOrdinal,
    currentSegmentText: readonly(currentSegmentText),
    totalSegmentsKnown: readonly(totalSegmentsKnown),
    totalFinalized: readonly(totalFinalized),
    progressRatio,
    error: readonly(error),
    availableEngines,
    selectedEngine,
    selectedEngineId: readonly(selectedEngineId),
    availableVoices: readonly(availableVoices),
    selectedVoiceId: readonly(selectedVoiceId),
    voicesLoading: readonly(voicesLoading),

    startReading,
    play,
    pause,
    stop,
    nextSegment,
    prevSegment,
    seekToIndex,
    setPlaybackRate,
    setVolume,
    setPitch,
    setEngineId,
    setVoiceId,
    refreshEngines,
    loadVoices,
    previewVoice,
  };
}
