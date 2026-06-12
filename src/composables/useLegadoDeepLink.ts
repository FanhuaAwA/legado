// useLegadoDeepLink — 解析 Legado 深链接并注册原生/应用内链接监听。
import { isTauri, isHarmonyNative } from "./useEnv";
import { eventListen } from "./useEventBus";
import { log } from "@/utils/logger";

const LEGADO_SCHEME = "legado:";
const YUEDU_SCHEME = "yuedu:";
const DEEP_LINK_DEDUPE_MS = 1000;

// ── 深链接类型定义 ────────────────────────────────────────────────────────────
export type LegadoDeepLinkPayload =
  | { type: "booksource"; url: string }
  | { type: "booksourceSubscription"; url: string }
  | { type: "repo"; url: string; name?: string }
  | { type: "plugin"; url: string };

export type LegadoInstallTarget = LegadoDeepLinkPayload | { type: "unknown" };

/** 将原始 payload 字符串规范化为 https?:// URL，至多解码两次 */
function normalizeHttpUrl(payload: string): string {
  let p = payload;
  for (let i = 0; i < 2; i += 1) {
    if (!/%[0-9a-f]{2}/i.test(p)) {
      break;
    }
    try {
      const decoded = decodeURIComponent(p);
      if (decoded === p) {
        break;
      }
      p = decoded;
    } catch {
      break;
    }
  }
  if (p.startsWith("//")) {
    p = `http:${p}`;
  } else if (!/^https?:\/\//i.test(p)) {
    p = `http://${p.replace(/^\/+/, "")}`;
  }
  const url = new URL(p);
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("仅支持 http 或 https 地址");
  }
  return url.href;
}

/**
 * 解析深链接，返回类型化的 payload。
 *
 * 支持格式：
 *  - 书源（向下兼容）：`https://...`  /  `legado://?url=...`
 *  - 阅读书源：`yuedu://booksource/importonline?src=...`
 *  - 阅读订阅：`yuedu://rsssource/importonline?src=...`
 *  - 仓库：`legado://repo?url=...&name=<默认名称>`
 *  - 插件：`legado://plugin?url=...`
 */
export function parseLegadoDeepLink(rawUrl: string): LegadoDeepLinkPayload {
  const input = rawUrl.trim();
  if (!input) {
    throw new Error("链接为空");
  }

  // 纯 https?:// → 默认当书源处理（向下兼容）
  if (/^https?:\/\//i.test(input)) {
    return { type: "booksource", url: normalizeHttpUrl(input) };
  }

  const lowerInput = input.toLowerCase();
  if (!lowerInput.startsWith(LEGADO_SCHEME) && !lowerInput.startsWith(YUEDU_SCHEME)) {
    throw new Error("不是 legado 深链接");
  }

  // 解析为标准 URL（legado://host?params 或 legado://?params）
  let parsed: URL | null = null;
  try {
    parsed = new URL(input);
  } catch {
    // fallback handled below
  }

  const host = parsed?.hostname ?? ""; // e.g. "repo" / "plugin" / ""
  const params = parsed?.searchParams;

  // ── 开源阅读 yuedu:// 链接 ────────────────────────────────────────────────
  if (lowerInput.startsWith(YUEDU_SCHEME)) {
    const rawSrc = params?.get("src") ?? params?.get("url") ?? "";
    if (host === "booksource") {
      if (!rawSrc) {
        throw new Error("阅读书源链接缺少 src 参数");
      }
      return { type: "booksource", url: normalizeHttpUrl(rawSrc) };
    }
    if (host === "rsssource") {
      if (!rawSrc) {
        throw new Error("阅读订阅链接缺少 src 参数");
      }
      return { type: "booksourceSubscription", url: normalizeHttpUrl(rawSrc) };
    }
    throw new Error("不支持的 yuedu 链接类型");
  }

  // ── 仓库 ────────────────────────────────────────────────────────────────────
  if (host === "repo") {
    const rawRepoUrl = params?.get("url") ?? "";
    if (!rawRepoUrl) {
      throw new Error("仓库链接缺少 url 参数");
    }
    const name = params?.get("name")?.trim() ?? undefined;
    return { type: "repo", url: normalizeHttpUrl(rawRepoUrl), name };
  }

  // ── 插件 ────────────────────────────────────────────────────────────────────
  if (host === "plugin") {
    const rawPluginUrl = params?.get("url") ?? "";
    if (!rawPluginUrl) {
      throw new Error("插件链接缺少 url 参数");
    }
    return { type: "plugin", url: normalizeHttpUrl(rawPluginUrl) };
  }

  // ── 书源（legado://?url=... 或 legado://...）─────────────────────────────
  let payload = params?.get("url") ?? "";
  if (!payload) {
    // 兼容 legado://https://... 形式
    payload = input.slice(`${LEGADO_SCHEME}//`.length);
  }
  if (!payload) {
    throw new Error("书源链接缺少 url 参数");
  }
  return { type: "booksource", url: normalizeHttpUrl(payload) };
}

function getHttpPathName(rawUrl: string): string {
  try {
    const url = new URL(normalizeHttpUrl(rawUrl));
    return decodeURIComponent(url.pathname).toLowerCase();
  } catch {
    return "";
  }
}

function looksLikeRepositoryUrl(rawUrl: string): boolean {
  const path = getHttpPathName(rawUrl);
  const fileName = path.split("/").filter(Boolean).pop() ?? "";
  return fileName === "repository.json";
}

function looksLikePluginUrl(rawUrl: string): boolean {
  const path = getHttpPathName(rawUrl);
  const fileName = path.split("/").filter(Boolean).pop() ?? "";
  return fileName.endsWith(".user.js");
}

/**
 * 安装书源入口的兜底分类。
 *
 * 正常深链接会先由 parseLegadoDeepLink 分流；这里用于 Java/WebView 等特殊
 * 环境把其它安装链接误送到书源安装弹窗时，按明确 URL 规则转发到对应入口。
 */
export function classifyLegadoInstallTarget(rawUrl: string): LegadoInstallTarget {
  const input = rawUrl.trim();
  if (!input) {
    return { type: "unknown" };
  }

  try {
    const parsed = parseLegadoDeepLink(input);
    if (
      input.toLowerCase().startsWith(LEGADO_SCHEME) ||
      input.toLowerCase().startsWith(YUEDU_SCHEME)
    ) {
      if (parsed.type === "booksource" && looksLikeRepositoryUrl(parsed.url)) {
        return { type: "repo", url: parsed.url };
      }
      if (parsed.type === "booksource" && looksLikePluginUrl(parsed.url)) {
        return { type: "plugin", url: parsed.url };
      }
      return parsed;
    }
  } catch {
    // Fallback rules below handle plain HTTP(S) links only.
  }

  if (!/^https?:\/\//i.test(input)) {
    return { type: "unknown" };
  }
  let url: string;
  try {
    url = normalizeHttpUrl(input);
  } catch {
    return { type: "unknown" };
  }
  if (looksLikeRepositoryUrl(url)) {
    return { type: "repo", url };
  }
  if (looksLikePluginUrl(url)) {
    return { type: "plugin", url };
  }
  return { type: "booksource", url };
}

/** @deprecated 使用 parseLegadoDeepLink 代替 */
export function parseLegadoBookSourceUrl(rawUrl: string): string {
  const result = parseLegadoDeepLink(rawUrl);
  if (result.type !== "booksource") {
    throw new Error("不是 legado 书源链接");
  }
  return result.url;
}

type DeepLinkHandler = (urls: string[]) => void;

interface DeepLinkEventPayloadObject {
  urls?: string[];
  url?: string;
}

type NativeDeepLinkPayload = DeepLinkEventPayloadObject | string[] | string | null | undefined;

function readDeepLinkUrls(payload: NativeDeepLinkPayload): string[] {
  if (typeof payload === "string") {
    return payload ? [payload] : [];
  }
  if (Array.isArray(payload)) {
    return payload.filter((url): url is string => typeof url === "string" && url.trim().length > 0);
  }
  if (payload && typeof payload === "object") {
    if (Array.isArray(payload.urls)) {
      return payload.urls.filter(
        (url): url is string => typeof url === "string" && url.trim().length > 0,
      );
    }
    if (typeof payload.url === "string" && payload.url.trim()) {
      return [payload.url];
    }
  }
  return [];
}

export async function installLegadoDeepLinkListener(handler: DeepLinkHandler): Promise<() => void> {
  const unlisteners: Array<() => void> = [];
  const recentLinks = new Map<string, number>();

  const deliver = (urls: string[]) => {
    const now = Date.now();
    const unique = urls.filter((url) => {
      const normalized = url.trim();
      if (!normalized) {
        return false;
      }
      const lastSeenAt = recentLinks.get(normalized) ?? 0;
      recentLinks.set(normalized, now);
      return now - lastSeenAt > DEEP_LINK_DEDUPE_MS;
    });
    if (unique.length) {
      handler(unique);
    }
  };

  if (isTauri) {
    try {
      const { getCurrent, onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
      const current = await getCurrent();
      if (current?.length) {
        deliver(current);
      }
      const unlisten = await onOpenUrl((urls) => deliver(urls));
      unlisteners.push(unlisten);
    } catch (e) {
      log.warn("DeepLink", "Tauri deep-link 初始化失败", e);
    }
  }

  if (isTauri || isHarmonyNative) {
    const unlisten = await eventListen<NativeDeepLinkPayload>("deep-link://new-url", (event) => {
      deliver(readDeepLinkUrls(event.payload));
    });
    unlisteners.push(unlisten);
  }

  if (!isTauri && !isHarmonyNative && typeof window !== "undefined") {
    const url = new URL(window.location.href);
    const fromQuery = url.searchParams.get("legado") ?? url.searchParams.get("url");
    const fromHash = url.hash.startsWith("#legado=") ? url.hash.slice("#legado=".length) : "";
    const current = fromQuery ?? fromHash;
    if (current) {
      deliver([current]);
    }
  }

  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
