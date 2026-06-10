import { computed, readonly, ref } from "vue";
import { invokeWithTimeout } from "./useInvoke";

export type CapabilityKey = "sync" | "tts" | "videoProxy";

export interface FeatureCapability {
  supported: boolean;
  reason: string;
  commands: string[];
}

export interface AppCapabilities {
  sync: FeatureCapability;
  tts: FeatureCapability;
  videoProxy: FeatureCapability;
}

function unsupported(reason: string, commands: string[]): FeatureCapability {
  return {
    supported: false,
    reason,
    commands,
  };
}

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
};

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
  const record = value && typeof value === "object" ? (value as Partial<AppCapabilities>) : {};
  return {
    sync: normalizeCapability(record.sync, fallbackCapabilities.sync),
    tts: normalizeCapability(record.tts, fallbackCapabilities.tts),
    videoProxy: normalizeCapability(record.videoProxy, fallbackCapabilities.videoProxy),
  };
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
