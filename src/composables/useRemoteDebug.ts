/**
 * useRemoteDebug — 按应用配置注入 Chii 远程调试 target 脚本。
 *
 * 该注入运行在前端页面内，因此同时覆盖 Tauri WebView 和浏览器服务模式。
 */

import { storeToRefs } from "pinia";
import { watch } from "vue";
import { useAppConfigStore } from "@/stores/appConfig";

const SCRIPT_ID = "legado-chii-target";

let installedSrc = "";

function removeScript() {
  const existing = document.getElementById(SCRIPT_ID);
  existing?.remove();
  installedSrc = "";
}

function normalizeHost(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }
  try {
    const parsed = new URL(trimmed.includes("://") ? trimmed : `http://${trimmed}`);
    return parsed.hostname;
  } catch {
    return (
      trimmed
        .replace(/^https?:\/\//, "")
        .split("/")[0]
        ?.split(":")[0] ?? ""
    );
  }
}

function buildTargetSrc(hostValue: string, portValue: number): string {
  const host = normalizeHost(hostValue);
  if (!host || !Number.isInteger(portValue) || portValue < 1 || portValue > 65535) {
    return "";
  }
  const wrappedHost = host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
  return `http://${wrappedHost}:${portValue}/target.js`;
}

function installScript(src: string) {
  if (installedSrc === src && document.getElementById(SCRIPT_ID)) {
    return;
  }
  removeScript();
  const script = document.createElement("script");
  script.id = SCRIPT_ID;
  script.src = src;
  script.async = true;
  script.onerror = () => {
    console.warn("[RemoteDebug] Chii target 脚本加载失败:", src);
  };
  document.head.appendChild(script);
  installedSrc = src;
}

export function useRemoteDebug() {
  const appConfigStore = useAppConfigStore();
  const { config } = storeToRefs(appConfigStore);

  watch(
    () =>
      [
        config.value.web_remote_debug_enabled,
        config.value.web_remote_debug_host,
        config.value.web_remote_debug_port,
      ] as const,
    ([enabled, host, port]) => {
      if (!enabled) {
        removeScript();
        return;
      }
      const src = buildTargetSrc(host, port);
      if (!src) {
        removeScript();
        return;
      }
      installScript(src);
    },
    { immediate: true },
  );
}
