/**
 * 分级结构化 Logger — 通过 Tauri `frontend_log` command 接入 Rust tracing。
 *
 * 级别：
 *   error → tracing::error
 *   warn  → tracing::warn
 *   info  → tracing::info
 *   debug → tracing::debug
 *
 * 用法：
 *   import { log } from "@/utils/logger";
 *   log.info("booksource.list", "found 5 sources", { extra: "data" });
 *   log.warn("booksource.search", "timeout after 35s", { fileName });
 *   log.error("prefetch", err);
 */

type LogLevel = "error" | "warn" | "info" | "debug" | "success";

async function sendToRust(
  level: LogLevel,
  zone: string,
  message: string,
  data?: unknown,
): Promise<void> {
  const body =
    data !== undefined
      ? `[${zone}] ${message} | ${typeof data === "string" ? data : JSON.stringify(data)}`
      : `[${zone}] ${message}`;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("frontend_log", { level: level === "success" ? "info" : level, message: body });
  } catch {
    // 回退到 console（Tauri 不可用时）
    const fn = console[level === "success" ? "log" : level] ?? console.log;
    fn(`[${zone}] ${message}`, data ?? "");
  }
}

export const log = {
  error(zone: string, messageOrError: string | Error, data?: unknown) {
    const msg = messageOrError instanceof Error ? messageOrError.message : messageOrError;
    const extra =
      messageOrError instanceof Error
        ? { stack: messageOrError.stack, ...((data as object) ?? {}) }
        : data;
    sendToRust("error", zone, msg, extra);
  },

  warn(zone: string, message: string, data?: unknown) {
    sendToRust("warn", zone, message, data);
  },

  info(zone: string, message: string, data?: unknown) {
    sendToRust("info", zone, message, data);
  },

  success(zone: string, message: string, data?: unknown) {
    sendToRust("success", zone, message, data);
  },

  debug(zone: string, message: string, data?: unknown) {
    sendToRust("debug", zone, message, data);
  },
};
