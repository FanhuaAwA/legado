<script setup lang="ts">
import { Download, ExternalLink, PackageCheck } from "lucide-vue-next";
import { useMessage } from "naive-ui";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import AppDialog from "@/components/base/AppDialog.vue";
import {
  downloadAppUpdate,
  installDownloadedAppUpdate,
  type AppUpdateDownloadProgress,
  type AppUpdateDownloadResult,
} from "@/composables/useAppUpdateDownload";
import { isTauri, platform, initPlatformFromRust } from "@/composables/useEnv";
import { eventListenSync } from "@/composables/useEventBus";
import { usePreferencesStore } from "@/stores";
import {
  checkAppUpdate,
  formatAppUpdateAssetSize,
  getAppUpdateChannelLabel,
  type AppUpdateCheckResult,
} from "@/utils/appUpdate";
import { log } from "@/utils/logger";
import tauriConfig from "../../src-tauri/tauri.conf.json";

const message = useMessage();
const prefStore = usePreferencesStore();

const show = ref(false);
const checking = ref(false);
const updateResult = ref<AppUpdateCheckResult | null>(null);
const downloadState = ref<"idle" | "downloading" | "downloaded" | "error">("idle");
const downloadProgress = ref<AppUpdateDownloadProgress | null>(null);
const downloadResult = ref<AppUpdateDownloadResult | null>(null);
const downloadError = ref("");
const installing = ref(false);
let startupTimer: ReturnType<typeof setTimeout> | null = null;

const updateTitle = computed(() => {
  const result = updateResult.value;
  if (!result) {
    return "发现更新";
  }
  return `发现 ${getAppUpdateChannelLabel(result.channel)}版更新`;
});

const assetSizeLabel = computed(() => {
  const size = updateResult.value?.asset?.size ?? 0;
  return formatAppUpdateAssetSize(size);
});

const downloadPercent = computed(() => {
  const percent = downloadProgress.value?.percent;
  if (typeof percent === "number" && Number.isFinite(percent)) {
    return Math.max(0, Math.min(100, Math.round(percent)));
  }

  const total = downloadProgress.value?.totalBytes ?? updateResult.value?.asset?.size ?? 0;
  const downloaded = downloadProgress.value?.downloadedBytes ?? 0;
  if (total > 0) {
    return Math.max(0, Math.min(100, Math.round((downloaded / total) * 100)));
  }
  return downloadState.value === "downloaded" ? 100 : 0;
});

const downloadStatusText = computed(() => {
  if (downloadState.value === "downloaded" && downloadResult.value) {
    return `已下载到应用数据目录，大小 ${formatAppUpdateAssetSize(downloadResult.value.size)}`;
  }
  if (downloadState.value === "error") {
    return downloadError.value || "下载失败";
  }
  const progress = downloadProgress.value;
  if (!progress) {
    return "准备下载";
  }
  const downloaded = formatAppUpdateAssetSize(progress.downloadedBytes);
  const total = progress.totalBytes ? formatAppUpdateAssetSize(progress.totalBytes) : "未知大小";
  const mode = progress.mode === "multipart" ? " · 多连接" : "";
  return `${downloaded} / ${total}${mode}`;
});

const canDownloadInApp = computed(() => {
  const currentPlatform = platform.value;
  return Boolean(
    isTauri &&
    updateResult.value?.asset &&
    (currentPlatform === "Windows" || currentPlatform === "Android"),
  );
});

const unsupportedPlatformText = computed(() => {
  if (!updateResult.value?.asset || canDownloadInApp.value) {
    return "";
  }
  return "当前平台暂未接入应用内下载和安装，请前往发布页查看更新说明。";
});

const installButtonLabel = computed(() =>
  platform.value === "Android" ? "安装 APK" : "安装并重启",
);

const primaryExternalUrl = computed(
  () => updateResult.value?.asset?.url || updateResult.value?.releaseUrl || "",
);

function createRequestId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `app-update-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function openExternalUrl(url: string) {
  if (!url) {
    return;
  }
  try {
    if (isTauri) {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
      return;
    }
  } catch (error) {
    log.warn("AppUpdate", "打开外部链接失败，回退到 window.open", { error });
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

const MIN_AUTO_CHECK_INTERVAL_MS = 60 * 60 * 1000; // 1 小时

async function runStartupCheck() {
  if (checking.value) {
    return;
  }

  try {
    await prefStore.ready;
    if (prefStore.appUpdate.autoCheckOnStartup === false) {
      return;
    }
    if (!isTauri) {
      return;
    }

    const lastAt = prefStore.appUpdate.lastAutoCheckAt ?? 0;
    if (Date.now() - lastAt < MIN_AUTO_CHECK_INTERVAL_MS) {
      return;
    }

    checking.value = true;
    prefStore.patchAppUpdate({ lastAutoCheckAt: Date.now() });
    await initPlatformFromRust().catch(() => undefined);
    const result = await checkAppUpdate({
      channel: prefStore.appUpdate.channel,
      currentVersion: tauriConfig.version,
      platform: platform.value,
    });

    if (!result.hasUpdate) {
      return;
    }

    showUpdateResult(result);
  } catch (error) {
    console.warn("[AppUpdate] 启动更新检查失败:", error);
  } finally {
    checking.value = false;
  }
}

function showUpdateResult(result: AppUpdateCheckResult) {
  updateResult.value = result;
  downloadState.value = "idle";
  downloadProgress.value = null;
  downloadResult.value = null;
  downloadError.value = "";
  show.value = true;
}

function handleManualUpdateEvent(event: Event) {
  const result = (event as CustomEvent<AppUpdateCheckResult>).detail;
  if (!result) {
    return;
  }
  showUpdateResult(result);
}

async function startDownload() {
  const asset = updateResult.value?.asset;
  if (!asset || !canDownloadInApp.value) {
    await openExternalUrl(primaryExternalUrl.value);
    return;
  }

  const requestId = createRequestId();
  downloadState.value = "downloading";
  downloadError.value = "";
  downloadResult.value = null;
  downloadProgress.value = {
    requestId,
    status: "started",
    mode: "unknown",
    fileName: asset.name,
    downloadedBytes: 0,
    totalBytes: asset.size || null,
    percent: 0,
  };

  try {
    downloadResult.value = await downloadAppUpdate({
      requestId,
      url: asset.url,
      fileName: asset.name,
      digest: asset.digest || undefined,
    });
    downloadState.value = "downloaded";
    downloadProgress.value = {
      requestId,
      status: "done",
      mode: downloadProgress.value?.mode || "unknown",
      fileName: asset.name,
      downloadedBytes: downloadResult.value.size,
      totalBytes: downloadResult.value.size,
      percent: 100,
      localPath: downloadResult.value.localPath,
    };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    downloadState.value = "error";
    downloadError.value = reason;
    message.error(`更新下载失败：${reason}`);
  }
}

async function openInstaller() {
  const path = downloadResult.value?.localPath;
  if (!path) {
    return;
  }

  installing.value = true;
  try {
    if (platform.value === "Android") {
      const result = window.LegadoAndroidInput?.installApk?.(path);
      if (result) {
        message.warning(result);
      }
      return;
    }
    await installDownloadedAppUpdate(path);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    message.error(`安装更新失败：${reason}`);
  } finally {
    installing.value = false;
  }
}

let unlistenProgress = () => {};
if (isTauri) {
  unlistenProgress = eventListenSync<AppUpdateDownloadProgress>(
    "app-update:download-progress",
    (event) => {
      const payload = event.payload;
      if (!payload || payload.requestId !== downloadProgress.value?.requestId) {
        return;
      }
      downloadProgress.value = payload;
      if (payload.status === "error") {
        downloadState.value = "error";
        downloadError.value = payload.error || "下载失败";
      }
    },
  );
}

onMounted(() => {
  window.addEventListener("legado:show-app-update", handleManualUpdateEvent);
  startupTimer = setTimeout(() => {
    void runStartupCheck();
  }, 1200);
});

onBeforeUnmount(() => {
  window.removeEventListener("legado:show-app-update", handleManualUpdateEvent);
  unlistenProgress();
  if (startupTimer) {
    clearTimeout(startupTimer);
  }
});
</script>

<template>
  <AppDialog
    :show="show"
    :title="updateTitle"
    width="520px"
    :mask-closable="downloadState !== 'downloading'"
    :closable="downloadState !== 'downloading'"
    :close-on-esc="downloadState !== 'downloading'"
    @update:show="(value) => (show = value)"
  >
    <div v-if="updateResult" class="app-update-dialog">
      <div class="app-update-dialog__versions">
        <div>
          <span class="app-update-dialog__label">当前版本</span>
          <strong>{{ updateResult.currentVersion }}</strong>
        </div>
        <div>
          <span class="app-update-dialog__label">最新版本</span>
          <strong>{{ updateResult.latestDisplayVersion || updateResult.latestVersion }}</strong>
        </div>
      </div>

      <div class="app-update-dialog__meta">
        <span>{{ updateResult.platformLabel }}</span>
        <span>{{ updateResult.releaseTag }}</span>
        <span v-if="updateResult.asset">{{ assetSizeLabel }}</span>
      </div>

      <n-alert v-if="!updateResult.asset" type="warning" :show-icon="false">
        {{ updateResult.unavailableReason || "当前平台暂未找到可下载的安装包" }}
      </n-alert>

      <div v-else class="app-update-dialog__asset">
        <span class="app-update-dialog__label">安装包</span>
        <span class="app-update-dialog__asset-name">{{ updateResult.asset.name }}</span>
      </div>

      <div v-if="downloadState !== 'idle'" class="app-update-dialog__download">
        <n-progress
          type="line"
          :percentage="downloadPercent"
          :processing="downloadState === 'downloading'"
          :status="downloadState === 'error' ? 'error' : 'success'"
        />
        <div class="app-update-dialog__download-text">
          {{ downloadStatusText }}
        </div>
      </div>

      <n-alert v-if="unsupportedPlatformText" type="info" :show-icon="false">
        {{ unsupportedPlatformText }}
      </n-alert>
    </div>

    <template #action>
      <n-space justify="end">
        <n-button :disabled="downloadState === 'downloading'" @click="show = false">稍后</n-button>
        <n-button secondary @click="openExternalUrl(updateResult?.releaseUrl || '')">
          <template #icon><ExternalLink :size="16" /></template>
          发布页
        </n-button>
        <n-button
          v-if="downloadState === 'downloaded' && canDownloadInApp"
          type="primary"
          :loading="installing"
          @click="openInstaller"
        >
          <template #icon><PackageCheck :size="16" /></template>
          {{ installButtonLabel }}
        </n-button>
        <n-button
          v-else-if="canDownloadInApp"
          type="primary"
          :loading="downloadState === 'downloading'"
          @click="startDownload"
        >
          <template #icon><Download :size="16" /></template>
          下载更新
        </n-button>
      </n-space>
    </template>
  </AppDialog>
</template>

<style scoped>
.app-update-dialog {
  display: grid;
  gap: 14px;
}

.app-update-dialog__versions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.app-update-dialog__versions > div,
.app-update-dialog__asset,
.app-update-dialog__download {
  border: 1px solid var(--color-border, rgba(148, 163, 184, 0.28));
  border-radius: 8px;
  padding: 10px 12px;
  min-width: 0;
}

.app-update-dialog__versions strong,
.app-update-dialog__asset-name {
  display: block;
  margin-top: 4px;
  font-size: 0.95rem;
  word-break: break-word;
}

.app-update-dialog__label {
  color: var(--color-text-muted, #71717a);
  font-size: 0.78rem;
}

.app-update-dialog__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  color: var(--color-text-muted, #71717a);
  font-size: 0.82rem;
}

.app-update-dialog__meta span {
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.14);
  padding: 2px 8px;
}

.app-update-dialog__download {
  display: grid;
  gap: 8px;
}

.app-update-dialog__download-text {
  color: var(--color-text-muted, #71717a);
  font-size: 0.82rem;
}

@media (max-width: 560px) {
  .app-update-dialog__versions {
    grid-template-columns: 1fr;
  }
}
</style>
