import { computed, readonly, ref } from "vue";
import { invokeWithTimeout } from "./useInvoke";

export type CapabilityKey =
  | "sync"
  | "tts"
  | "videoProxy"
  | "browserProbe"
  | "comicCache"
  | "coverCache"
  | "repository"
  | "unlock"
  | "aiProxy"
  | "pluginHttp"
  | "exploreCache";

export interface FeatureCapability {
  supported: boolean;
  reason: string;
  commands: string[];
}

export type AppCapabilities = Record<CapabilityKey, FeatureCapability>;

function unsupported(reason: string, commands: string[]): FeatureCapability {
  return {
    supported: false,
    reason,
    commands,
  };
}

function supported(reason: string, commands: string[]): FeatureCapability {
  return {
    supported: true,
    reason,
    commands,
  };
}

// 离线兜底表：capabilities_get 不可达时使用。键集合必须与后端
// src-tauri/src/commands/system.rs 的 CAPABILITY_SPECS 保持一致。
const fallbackCapabilities: AppCapabilities = {
  sync: unsupported("Sync backend is not implemented in this build.", [
    "sync_baidu_start_auth",
    "sync_baidu_poll_token",
    "sync_baidu_token_status",
    "sync_baidu_revoke_auth",
    "sync_set_credentials",
    "sync_get_credentials",
    "sync_clear_credentials",
    "sync_get_status",
    "sync_now",
    "sync_test_connection",
    "sync_list_conflicts",
    "sync_resolve_conflict",
    "sync_notify_lifecycle",
    "sync_client_state_set",
    "sync_report_reader_session",
    "sync_v2_sync_reading_progress",
  ]),
  tts: unsupported(
    "Native TTS backend is not implemented in this build; browser speech remains available.",
    [
      "tts_get_voices",
      "tts_is_initialized",
      "tts_is_speaking",
      "tts_speak",
      "tts_stop",
      "tts_preview_voice",
    ],
  ),
  videoProxy: unsupported("Local video proxy is not implemented in this build.", [
    "start_video_proxy",
    "stop_video_proxy",
  ]),
  browserProbe: unsupported(
    "Headless browser probe is not implemented in this build; sources requiring WebView verification cannot run it.",
    [
      "browser_probe_create",
      "browser_probe_close",
      "browser_probe_close_all",
      "browser_probe_hide",
      "browser_probe_show",
      "browser_probe_navigate",
      "browser_probe_eval",
      "browser_probe_run",
      "browser_probe_get_cookies",
      "browser_probe_set_cookie",
      "browser_probe_clear_data",
      "browser_probe_set_user_agent",
    ],
  ),
  comicCache: unsupported(
    "Comic page cache is not implemented in this build; pages load directly from the network.",
    [
      "comic_cache_clear",
      "comic_cache_clear_chapter",
      "comic_cache_size",
      "comic_download_images",
      "comic_get_cached_page",
      "comic_get_page_sizes",
    ],
  ),
  coverCache: unsupported(
    "Cover disk cache is not implemented in this build; covers load directly from the network.",
    ["cover_cache_clear", "cover_cache_size", "cover_resolve_cache"],
  ),
  repository: supported(
    "Source repository browsing and JS-source auto-update via @updateUrl are supported.",
    [
      "repository_fetch",
      "repository_install",
      "repository_preview_source",
      "repository_check_source_sync",
      "booksource_check_update",
      "booksource_apply_update",
    ],
  ),
  unlock: unsupported("Secure-mode unlock challenges are not implemented in this build.", [
    "issue_full_mode_challenge",
    "verify_full_mode_challenge",
    "issue_scoped_unlock_challenge",
    "verify_scoped_unlock_challenge",
  ]),
  aiProxy: unsupported(
    "AI HTTP proxy is not implemented in this build; AI features use direct connections.",
    ["ai_http_proxy_url"],
  ),
  pluginHttp: unsupported("Frontend plugin HTTP bridge is not implemented in this build.", [
    "frontend_plugin_http_request",
  ]),
  exploreCache: unsupported(
    "Explore result cache is not implemented in this build; nothing to clear.",
    ["explore_clear_cache"],
  ),
};

const capabilityKeys = Object.keys(fallbackCapabilities) as CapabilityKey[];

const capabilities = ref<AppCapabilities>(fallbackCapabilities);
const loaded = ref(false);
const loading = ref(false);
let loadPromise: Promise<AppCapabilities> | null = null;

function normalizeCapability(value: unknown, fallback: FeatureCapability): FeatureCapability {
  if (!value || typeof value !== "object") {
    return fallback;
  }
  const record = value as Partial<FeatureCapability>;
  return {
    supported: record.supported === true,
    reason:
      typeof record.reason === "string" && record.reason.trim() ? record.reason : fallback.reason,
    commands: Array.isArray(record.commands)
      ? record.commands.filter((item): item is string => typeof item === "string")
      : fallback.commands,
  };
}

function normalizeCapabilities(value: unknown): AppCapabilities {
  const record =
    value && typeof value === "object" ? (value as Partial<Record<CapabilityKey, unknown>>) : {};
  const result = {} as AppCapabilities;
  for (const key of capabilityKeys) {
    result[key] = normalizeCapability(record[key], fallbackCapabilities[key]);
  }
  return result;
}

async function loadCapabilities(force = false): Promise<AppCapabilities> {
  if (loaded.value && !force) {
    return capabilities.value;
  }
  if (loadPromise) {
    return loadPromise;
  }

  loading.value = true;
  loadPromise = invokeWithTimeout<unknown>("capabilities_get", undefined, 5000)
    .then((value) => {
      capabilities.value = normalizeCapabilities(value);
      return capabilities.value;
    })
    .catch(() => {
      capabilities.value = fallbackCapabilities;
      return capabilities.value;
    })
    .finally(() => {
      loaded.value = true;
      loading.value = false;
      loadPromise = null;
    });
  return loadPromise;
}

export function useCapabilities() {
  function getCapability(key: CapabilityKey) {
    return computed(() => capabilities.value[key]);
  }

  function isSupported(key: CapabilityKey) {
    return computed(() => capabilities.value[key].supported);
  }

  function unsupportedReason(key: CapabilityKey) {
    return computed(() => capabilities.value[key].reason);
  }

  async function requireCapability(key: CapabilityKey): Promise<void> {
    const state = await loadCapabilities();
    const capability = state[key];
    if (!capability.supported) {
      throw new Error(capability.reason);
    }
  }

  return {
    capabilities: readonly(capabilities),
    loaded: readonly(loaded),
    loading: readonly(loading),
    loadCapabilities,
    getCapability,
    isSupported,
    unsupportedReason,
    requireCapability,
  };
}
