/** * SectionAbout — 设置页“关于”面板，展示应用版本、运行环境、桥接能力与 WebView 诊断信息。 */

<script setup lang="ts">
import { Download, ExternalLink, RefreshCw } from "lucide-vue-next";
import { useMessage } from "naive-ui";
import { storeToRefs } from "pinia";
import { computed, onMounted, ref } from "vue";
import {
  hasNativeTransport,
  isHarmonyNative,
  isTauri,
  isMobile,
  envLabel,
  platform,
} from "@/composables/useEnv";
import { getCustomWsUrl, getTransportType, isTransportAvailable } from "@/composables/useTransport";
import { usePreferencesStore } from "@/stores/preferences";
import {
  checkAppUpdate,
  formatAppUpdateAssetSize,
  getAppUpdateChannelLabel,
  getAppUpdateReleasePageUrl,
  isAppUpdateChannel,
  resolveAppUpdatePlatform,
  type AppUpdateChannel,
  type AppUpdateCheckResult,
} from "@/utils/appUpdate";
import packageJson from "../../../package.json";
import tauriConfig from "../../../src-tauri/tauri.conf.json";
import FullModeUnlockDialog from "./FullModeUnlockDialog.vue";
import SettingItem from "./SettingItem.vue";
import SettingSection from "./SettingSection.vue";

const message = useMessage();
const prefStore = usePreferencesStore();
const { devTools, appUpdate } = storeToRefs(prefStore);

type TransportMode = "tauri" | "harmony" | "websocket" | "none";
type TagType = "default" | "info" | "success" | "warning" | "error";

const transportMode = ref<TransportMode>(getTransportType());
const transportReady = ref(hasNativeTransport);

const rawUserAgent = ref("读取中");
const updateChecking = ref(false);
const updateResult = ref<AppUpdateCheckResult | null>(null);
const updateError = ref("");
const showFullModeUnlockDialog = ref(false);

const updateChannelOptions = [
  { label: "正式", value: "stable" },
  { label: "开发", value: "development" },
] satisfies { label: string; value: AppUpdateChannel }[];

const contributors = [
  "Mg",
  "丽拓朝夕",
  "喵公子",
  "聚.散",
  "杯呗is me",
  "I'm yours",
  "凉子",
  "目目",
];

const runtimeModeLabel = computed(() => {
  switch (transportMode.value) {
    case "tauri":
      return "Tauri IPC";
    case "harmony":
      return "Harmony 桥接";
    case "websocket":
      return "WebSocket";
    default:
      return "未连接";
  }
});

const runtimeModeDesc = computed(() => {
  switch (transportMode.value) {
    case "tauri":
      return "当前工作在桌面原生壳内，前端通过 Tauri IPC 直连 Rust 命令。";
    case "harmony":
      return "当前工作在 Harmony 原生壳内，前端通过原生桥接访问宿主能力。";
    case "websocket":
      return "当前工作在 Web / WS 模式，前端通过 WebSocket 与后端服务通信。";
    default:
      return "当前未检测到可用宿主传输，仅能进行静态界面预览。";
  }
});

const runtimeTagType = computed(() => {
  if (!transportReady.value) {
    return "warning";
  }
  return transportMode.value === "websocket" ? "info" : "success";
});

const wsEndpoint = computed(() => {
  if (transportMode.value !== "websocket") {
    return "未使用";
  }
  return getCustomWsUrl() || "同源 WebSocket 自动探测";
});

const detectWebViewVersion = (ua: string) => {
  const harmonyWebView = ua.match(/ArkWeb\/([^\s]+)/i);
  if (harmonyWebView) {
    return `ArkWeb ${harmonyWebView[1]}`;
  }

  const edge = ua.match(/Edg\/([^\s]+)/i);
  if (edge) {
    return `Edge WebView2 ${edge[1]}`;
  }

  const webview = ua.match(/Version\/([^\s]+).*Chrome\/([^\s]+).*Mobile Safari/i);
  if (webview) {
    return `Android WebView / Chromium ${webview[2]}`;
  }

  const chrome = ua.match(/(?:Chrome|Chromium|CriOS)\/([^\s]+)/i);
  if (chrome) {
    return `Chromium ${chrome[1]}`;
  }

  const safari = ua.match(/Version\/([^\s]+).*Safari\/([^\s]+)/i);
  if (safari) {
    return `WebKit / Safari ${safari[1]}`;
  }

  return "未能从 UA 识别";
};

const webViewVersion = computed(() => detectWebViewVersion(rawUserAgent.value));

const environmentCards = computed(() => [
  {
    label: "运行环境",
    value: envLabel,
    desc: runtimeModeDesc.value,
  },
  {
    label: "通信模式",
    value: runtimeModeLabel.value,
    desc: transportReady.value ? "当前传输层已可用。" : "当前传输层不可用。",
  },
  {
    label: "操作系统",
    value: platform.value || "未知",
    desc: isMobile.value ? "当前为移动端布局。" : "当前为桌面端布局。",
  },
  {
    label: "WebView / 内核",
    value: webViewVersion.value,
    desc: "由当前页面原始 UA 解析，完整字段见下方诊断信息。",
  },
]);

const uaRows = computed(() => [
  {
    label: "系统原始 UA",
    value: rawUserAgent.value,
    desc: "本软件当前运行环境上报的原始 User-Agent，可用于查看系统 WebView 或浏览器内核版本。",
  },
]);

const updatePlatformInfo = computed(() => resolveAppUpdatePlatform(platform.value));

const updateReleasePageUrl = computed(() => getAppUpdateReleasePageUrl(appUpdate.value.channel));

const updateChannelLabel = computed(() => getAppUpdateChannelLabel(appUpdate.value.channel));

const updateResultChannelLabel = computed(() =>
  getAppUpdateChannelLabel(updateResult.value?.channel ?? appUpdate.value.channel),
);

const updateStatusLabel = computed(() => {
  if (updateChecking.value) {
    return "检查中";
  }
  if (updateError.value) {
    return "检查失败";
  }
  if (!updateResult.value) {
    return "尚未检查";
  }
  if (updateResult.value.hasUpdate) {
    return "发现新版本";
  }
  if (updateResult.value.unavailableReason) {
    return "无本平台包";
  }
  return "已是最新";
});

const updateStatusTagType = computed<TagType>(() => {
  if (updateError.value) {
    return "error";
  }
  if (updateChecking.value) {
    return "info";
  }
  if (updateResult.value?.hasUpdate) {
    return "success";
  }
  if (updateResult.value?.unavailableReason) {
    return "warning";
  }
  return "default";
});

const updateSummaryTitle = computed(() => {
  if (updateError.value) {
    return updateError.value;
  }
  if (!updateResult.value) {
    return `等待检查 ${updateChannelLabel.value} 渠道`;
  }
  if (updateResult.value.hasUpdate) {
    return `${updateResult.value.latestDisplayVersion || updateResult.value.latestVersion} 可用`;
  }
  if (updateResult.value.unavailableReason) {
    return "未找到当前平台安装包";
  }
  return `当前核心版本 ${updateResult.value.currentVersion}`;
});

const updateSummaryDesc = computed(() => {
  if (updateError.value) {
    return "请稍后重试，或打开对应发布页手动查看。";
  }
  if (!updateResult.value) {
    return `当前平台识别为 ${updatePlatformInfo.value.label}，检查时会匹配该平台最近可用的发布产物。`;
  }
  if (updateResult.value.unavailableReason) {
    return updateResult.value.unavailableReason;
  }
  if (updateResult.value.hasUpdate) {
    return `当前版本 ${updateResult.value.currentVersion}，${updateResultChannelLabel.value} 渠道最新核心版本为 ${updateResult.value.latestVersion}。`;
  }
  return `当前核心版本不低于 ${updateResultChannelLabel.value} 渠道最新可用版本。`;
});

const updatePublishedAt = computed(() => formatReleaseTime(updateResult.value?.releasePublishedAt));

const updateAssetSize = computed(() =>
  updateResult.value?.asset ? formatAppUpdateAssetSize(updateResult.value.asset.size) : "",
);

const updateCanDownload = computed(() => {
  const resultPlatform = updateResult.value?.platform;
  return resultPlatform === "windows" || resultPlatform === "android";
});

function formatReleaseTime(value?: string) {
  if (!value) {
    return "未知";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function handleUpdateChannel(value: string) {
  if (!isAppUpdateChannel(value)) {
    return;
  }
  prefStore.patchAppUpdate({ channel: value });
  updateResult.value = null;
  updateError.value = "";
}

async function handleCheckUpdate() {
  updateChecking.value = true;
  updateError.value = "";
  try {
    const result = await checkAppUpdate({
      channel: appUpdate.value.channel,
      currentVersion: tauriConfig.version,
      platform: platform.value,
    });
    updateResult.value = result;
    if (result.hasUpdate) {
      message.success("发现可用更新");
    } else if (result.unavailableReason) {
      message.warning(result.unavailableReason);
    } else {
      message.info("当前已是最新核心版本");
    }
  } catch (error: unknown) {
    const text = error instanceof Error ? error.message : String(error);
    updateError.value = text;
    message.error(text);
  } finally {
    updateChecking.value = false;
  }
}

async function openUrl(url: string) {
  if (!url) {
    return;
  }
  try {
    const { openUrl: tauriOpenUrl } = await import("@tauri-apps/plugin-opener");
    await tauriOpenUrl(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

async function openUpdateReleasePage() {
  await openUrl(updateResult.value?.releaseUrl || updateReleasePageUrl.value);
}

async function openUpdateDownload() {
  if (!updateResult.value?.asset || !updateCanDownload.value) {
    return;
  }
  window.dispatchEvent(
    new CustomEvent("legado:show-app-update", {
      detail: updateResult.value,
    }),
  );
}

function collectUserAgentInfo() {
  if (typeof navigator === "undefined") {
    rawUserAgent.value = "当前环境没有 navigator";
    return;
  }

  rawUserAgent.value = navigator.userAgent || "空";
}

onMounted(async () => {
  collectUserAgentInfo();
  transportReady.value = await isTransportAvailable();
  transportMode.value = getTransportType();
});
</script>

<template>
  <SettingSection title="关于" section-id="section-about">
    <div class="about-hero">
      <div class="about-hero__copy">
        <div class="about-hero__eyebrow">开源阅读</div>
        <h3 class="about-hero__title">跨端阅读应用运行概览</h3>
        <p class="about-hero__desc">
          当前页聚合应用版本、运行模式、桥接能力和软件贡献者，便于快速确认当前环境到底跑在
          Tauri、Harmony 还是 Web / WS 模式。
        </p>
      </div>
      <div class="about-hero__chips">
        <n-tag size="small" :bordered="false">前端 {{ packageJson.version }}</n-tag>
        <n-tag size="small" :bordered="false" type="info">桌面壳 {{ tauriConfig.version }}</n-tag>
        <n-tag size="small" :bordered="false" :type="runtimeTagType as any">
          {{ runtimeModeLabel }}
        </n-tag>
      </div>
    </div>

    <div class="about-runtime-grid">
      <div v-for="item in environmentCards" :key="item.label" class="about-card">
        <span class="about-label">{{ item.label }}</span>
        <strong class="about-card__value">{{ item.value }}</strong>
        <p class="about-card__desc">{{ item.desc }}</p>
      </div>
    </div>

    <div class="about-panel about-panel--update">
      <div class="about-panel__head">
        <h4 class="about-panel__title">版本更新</h4>
        <n-tag size="small" :type="updateStatusTagType as any" :bordered="false">
          {{ updateStatusLabel }}
        </n-tag>
      </div>

      <SettingItem
        label="检测渠道"
        :desc="`当前平台：${updatePlatformInfo.label}`"
        :vertical="true"
      >
        <div class="about-update-actions">
          <n-radio-group
            :value="appUpdate.channel"
            size="small"
            @update:value="handleUpdateChannel"
          >
            <n-radio-button
              v-for="option in updateChannelOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </n-radio-button>
          </n-radio-group>
          <n-button
            size="small"
            type="primary"
            :loading="updateChecking"
            @click="handleCheckUpdate"
          >
            <template #icon>
              <n-icon><RefreshCw /></n-icon>
            </template>
            检查
          </n-button>
          <n-button size="small" quaternary @click="openUpdateReleasePage">
            <template #icon>
              <n-icon><ExternalLink /></n-icon>
            </template>
            发布页
          </n-button>
        </div>
      </SettingItem>

      <div class="about-update-summary">
        <div class="about-update-summary__head">
          <strong>{{ updateSummaryTitle }}</strong>
          <span>{{ updateChannelLabel }}渠道</span>
        </div>
        <p>{{ updateSummaryDesc }}</p>
      </div>

      <div v-if="updateResult" class="about-update-meta">
        <div>
          <span>最新版本</span>
          <strong>{{ updateResult.latestDisplayVersion || updateResult.latestVersion }}</strong>
        </div>
        <div>
          <span>发布时间</span>
          <strong>{{ updatePublishedAt }}</strong>
        </div>
        <div>
          <span>发布标签</span>
          <strong>{{ updateResult.releaseTag || "未知" }}</strong>
        </div>
        <div>
          <span>匹配平台</span>
          <strong>{{ updateResult.platformLabel }}</strong>
        </div>
      </div>

      <div v-if="updateResult?.asset" class="about-update-asset">
        <div class="about-update-asset__info">
          <span>安装包</span>
          <strong>{{ updateResult.asset.name }}</strong>
          <small>{{ updateAssetSize }}</small>
          <small v-if="updateResult.asset.digest" class="about-update-asset__digest">
            {{ updateResult.asset.digest }}
          </small>
        </div>
        <n-button
          v-if="updateCanDownload"
          size="small"
          type="primary"
          secondary
          @click="openUpdateDownload"
        >
          <template #icon>
            <n-icon><Download /></n-icon>
          </template>
          下载并安装
        </n-button>
        <n-tag v-else size="small" :bordered="false" type="info">仅提示</n-tag>
      </div>
    </div>

    <div class="about-panel about-panel--ua">
      <div class="about-panel__head">
        <h4 class="about-panel__title">系统原始 UA</h4>
        <span class="about-panel__hint">当前软件运行环境</span>
      </div>
      <div class="about-ua-list">
        <div v-for="item in uaRows" :key="item.label" class="about-ua-row">
          <div class="about-ua-row__meta">
            <span class="about-badge-row__label">{{ item.label }}</span>
            <p>{{ item.desc }}</p>
          </div>
          <pre class="about-ua-row__value">{{ item.value }}</pre>
        </div>
      </div>
    </div>

    <div class="about-panels">
      <div class="about-panel">
        <div class="about-panel__head">
          <h4 class="about-panel__title">桥接能力</h4>
          <span class="about-panel__hint">当前宿主能力检测</span>
        </div>
        <div class="about-badges">
          <div class="about-badge-row">
            <span class="about-badge-row__label">Tauri 原生壳</span>
            <n-tag size="small" :type="isTauri ? 'success' : 'default'" :bordered="false">
              {{ isTauri ? "可用" : "未启用" }}
            </n-tag>
          </div>
          <div class="about-badge-row">
            <span class="about-badge-row__label">Harmony 原生桥接</span>
            <n-tag size="small" :type="isHarmonyNative ? 'success' : 'default'" :bordered="false">
              {{ isHarmonyNative ? "可用" : "未启用" }}
            </n-tag>
          </div>
          <div class="about-badge-row">
            <span class="about-badge-row__label">原生传输</span>
            <n-tag
              size="small"
              :type="hasNativeTransport ? 'success' : 'default'"
              :bordered="false"
            >
              {{ hasNativeTransport ? "直连宿主" : "浏览器环境" }}
            </n-tag>
          </div>
          <div class="about-badge-row">
            <span class="about-badge-row__label">WebSocket 入口</span>
            <n-tag
              size="small"
              :type="transportMode === 'websocket' ? 'info' : 'default'"
              :bordered="false"
            >
              {{ wsEndpoint }}
            </n-tag>
          </div>
          <div class="about-badge-row">
            <span class="about-badge-row__label">完全体模式</span>
            <div class="about-badge-row__actions">
              <n-tag
                size="small"
                :type="devTools.fullModeEnabled ? 'success' : 'default'"
                :bordered="false"
              >
                {{ devTools.fullModeEnabled ? "已激活" : "未激活" }}
              </n-tag>
              <n-button size="tiny" quaternary @click="showFullModeUnlockDialog = true">
                {{ devTools.fullModeEnabled ? "管理" : "解锁" }}
              </n-button>
            </div>
          </div>
        </div>
      </div>

      <div class="about-panel">
        <div class="about-panel__head">
          <h4 class="about-panel__title">软件贡献者</h4>
          <span class="about-panel__hint">排名不分先后</span>
        </div>
        <div class="about-contributors">
          <span v-for="name in contributors" :key="name" class="about-contributor">
            {{ name }}
          </span>
        </div>
      </div>
    </div>
  </SettingSection>
  <FullModeUnlockDialog v-model:show="showFullModeUnlockDialog" />
</template>

<style scoped>
.about-hero {
  display: flex;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-1) 0 var(--space-2);
}

.about-hero__copy {
  max-width: 540px;
}

.about-hero__eyebrow {
  font-size: var(--fs-12);
  font-weight: var(--fw-bold);
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-accent);
}

.about-hero__title {
  margin-top: 4px;
  font-size: 1.05rem;
  color: var(--color-text);
}

.about-hero__desc {
  margin-top: 6px;
  font-size: var(--fs-13);
  line-height: 1.7;
  color: var(--color-text-soft);
}

.about-hero__chips {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  justify-content: flex-end;
  gap: var(--space-2);
  min-width: 180px;
}

.about-runtime-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
  margin-top: var(--space-2);
}

.about-card,
.about-panel {
  border: 1px solid var(--color-border);
  border-radius: 14px;
  background: var(--color-surface);
}

.about-card {
  padding: 14px;
}

.about-card__value {
  display: block;
  margin-top: var(--space-2);
  font-size: 1rem;
  color: var(--color-text);
}

.about-card__desc {
  margin-top: var(--space-2);
  font-size: var(--fs-12);
  line-height: 1.65;
  color: var(--color-text-soft);
}

.about-panels {
  display: grid;
  grid-template-columns: 1.35fr 1fr;
  gap: var(--space-3);
  margin-top: var(--space-3);
}

.about-panel {
  padding: 14px;
}

.about-panel--update {
  margin-top: var(--space-3);
}

.about-panel--ua {
  margin-top: var(--space-3);
}

.about-panel__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
}

.about-panel__title {
  font-size: 0.9rem;
  color: var(--color-text);
}

.about-panel__hint {
  font-size: var(--fs-12);
  color: var(--color-text-muted);
}

.about-badges {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 14px;
}

.about-badge-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding-bottom: 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--color-border) 70%, transparent);
}

.about-badge-row:last-child {
  padding-bottom: 0;
  border-bottom: none;
}

.about-badge-row__label {
  font-size: var(--fs-13);
  color: var(--color-text-soft);
}

.about-badge-row__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.about-update-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  flex-wrap: wrap;
  width: 100%;
}

.about-update-summary {
  margin-top: var(--space-2);
  padding: 12px;
  border: 1px solid color-mix(in srgb, var(--color-border) 70%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--color-border) 18%, transparent);
}

.about-update-summary__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.about-update-summary__head strong {
  min-width: 0;
  color: var(--color-text);
  font-size: var(--fs-14);
  overflow-wrap: anywhere;
}

.about-update-summary__head span {
  flex: 0 0 auto;
  font-size: var(--fs-12);
  color: var(--color-text-muted);
}

.about-update-summary p {
  margin-top: 6px;
  font-size: var(--fs-12);
  line-height: 1.65;
  color: var(--color-text-soft);
}

.about-update-meta {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.about-update-meta > div {
  min-width: 0;
  padding: 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--color-border) 16%, transparent);
}

.about-update-meta span,
.about-update-asset__info span,
.about-update-asset__info small {
  display: block;
  font-size: var(--fs-12);
  color: var(--color-text-muted);
}

.about-update-meta strong,
.about-update-asset__info strong {
  display: block;
  margin-top: 4px;
  min-width: 0;
  color: var(--color-text);
  font-size: var(--fs-13);
  overflow-wrap: anywhere;
}

.about-update-asset {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  margin-top: var(--space-2);
  padding: 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--color-accent) 8%, var(--color-surface));
}

.about-update-asset__info {
  min-width: 0;
}

.about-update-asset__info small {
  margin-top: 4px;
}

.about-update-asset__digest {
  overflow-wrap: anywhere;
}

.about-ua-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 14px;
}

.about-ua-row {
  display: grid;
  grid-template-columns: minmax(140px, 0.28fr) minmax(0, 1fr);
  gap: var(--space-3);
  padding-bottom: 12px;
  border-bottom: 1px solid color-mix(in srgb, var(--color-border) 70%, transparent);
}

.about-ua-row:last-child {
  padding-bottom: 0;
  border-bottom: none;
}

.about-ua-row__meta p {
  margin-top: 4px;
  font-size: var(--fs-12);
  line-height: 1.55;
  color: var(--color-text-muted);
}

.about-ua-row__value {
  min-width: 0;
  margin: 0;
  padding: 9px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--color-border) 22%, transparent);
  color: var(--color-text);
  font-family:
    ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New",
    monospace;
  font-size: var(--fs-12);
  line-height: 1.55;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.about-contributors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: 14px;
}

.about-contributor {
  padding: 6px 10px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--color-accent) 10%, var(--color-surface));
  color: var(--color-text);
  font-size: var(--fs-13);
  font-weight: var(--fw-medium);
}

.about-label {
  font-size: var(--fs-12);
  color: var(--color-text-muted);
}

@media (max-width: 720px) {
  .about-hero,
  .about-panels {
    grid-template-columns: 1fr;
    display: grid;
  }

  .about-hero__chips {
    justify-content: flex-start;
    min-width: 0;
  }

  .about-runtime-grid {
    grid-template-columns: 1fr;
  }

  .about-ua-row {
    grid-template-columns: 1fr;
  }

  .about-panel__head,
  .about-badge-row {
    flex-direction: column;
    align-items: flex-start;
  }

  .about-update-actions,
  .about-update-asset {
    align-items: flex-start;
    justify-content: flex-start;
  }

  .about-update-summary__head,
  .about-update-asset {
    flex-direction: column;
  }

  .about-update-meta {
    grid-template-columns: 1fr;
  }
}
</style>
