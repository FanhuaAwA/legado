<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { eventEmit } from "@/composables/useEventBus";
import {
  classifyLegadoInstallTarget,
  installLegadoDeepLinkListener,
} from "@/composables/useLegadoDeepLink";
import { useNavigationStore } from "@/stores";
import BookSourceInstallDialog from "./BookSourceInstallDialog.vue";

const navigationStore = useNavigationStore();

const queue: string[] = [];
const show = ref(false);
const currentDownloadUrl = ref("");
const currentRawLink = ref("");
const currentParseError = ref("");

let unlisten: (() => void) | null = null;
let unlistenInApp: (() => void) | null = null;

function handleInAppInstall(e: Event) {
  const url = (e as CustomEvent<{ url: string }>).detail?.url;
  if (url) {
    enqueueLinks([url]);
  }
}

function dispatchRepoEvent(url: string, name?: string) {
  navigationStore.navigateToOnlineRepo(url, name);
}

function dispatchPluginEvent(url: string) {
  navigationStore.navigateToPluginInstall(url);
}

function enqueueLinks(urls: string[]) {
  for (const raw of urls) {
    if (!raw?.trim()) {
      continue;
    }
    const payload = classifyLegadoInstallTarget(raw);
    if (payload.type === "unknown") {
      // 无法识别类型时当书源处理，错误留给 openNext 展示
      queue.push(raw);
      continue;
    }
    if (payload.type === "repo") {
      dispatchRepoEvent(payload.url, payload.name);
    } else if (payload.type === "plugin") {
      dispatchPluginEvent(payload.url);
    } else {
      queue.push(raw);
    }
  }
  void openNext();
}

function openNext() {
  if (show.value) {
    return;
  }
  const next = queue.shift();
  if (!next) {
    return;
  }

  currentRawLink.value = next;
  currentParseError.value = "";
  currentDownloadUrl.value = "";
  const result = classifyLegadoInstallTarget(next);
  if (result.type === "booksource") {
    currentDownloadUrl.value = result.url;
  } else if (result.type === "repo") {
    dispatchRepoEvent(result.url, result.name);
    void openNext();
    return;
  } else if (result.type === "plugin") {
    dispatchPluginEvent(result.url);
    void openNext();
    return;
  } else {
    currentParseError.value = "不是可识别的 Legado 安装链接";
  }
  show.value = true;
}

function onUpdateShow(visible: boolean) {
  show.value = visible;
  if (!visible) {
    void openNext();
  }
}

async function onInstalled() {
  await eventEmit("app:view-reload", {
    view: "booksource",
    reason: "deep-link-install",
  });
}

onMounted(async () => {
  unlisten = await installLegadoDeepLinkListener(enqueueLinks);
  // 接收来自 iframe bridge 的应用内安装请求（用 CustomEvent 而非 Tauri 事件，避免 Rust 不回发的问题）
  window.addEventListener("app:install-source", handleInAppInstall);
  unlistenInApp = () => window.removeEventListener("app:install-source", handleInAppInstall);
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
  unlistenInApp?.();
  unlistenInApp = null;
});
</script>

<template>
  <BookSourceInstallDialog
    :show="show"
    :download-url="currentDownloadUrl"
    :raw-link="currentRawLink"
    :parse-error="currentParseError"
    @update:show="onUpdateShow"
    @installed="onInstalled"
  />
</template>
