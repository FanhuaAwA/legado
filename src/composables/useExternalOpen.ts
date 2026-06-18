import { isTauri } from "@/composables/useEnv";

export interface OpenExternalUrlOptions {
  fallbackToWindow?: boolean;
}

function openWithWindow(url: string): boolean {
  if (typeof window === "undefined" || typeof window.open !== "function") {
    return false;
  }
  try {
    if (typeof document !== "undefined" && typeof document.createElement === "function") {
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.target = "_blank";
      anchor.rel = "noopener noreferrer";
      anchor.style.display = "none";
      document.body?.appendChild(anchor);
      anchor.click();
      anchor.remove();
      return true;
    }
    window.open(url, "_blank", "noopener,noreferrer");
    return true;
  } catch {
    return false;
  }
}

export async function openExternalUrl(
  rawUrl: string | null | undefined,
  options: OpenExternalUrlOptions = {},
): Promise<boolean> {
  const url = rawUrl?.trim();
  if (!url) {
    return false;
  }

  const fallbackToWindow = options.fallbackToWindow ?? true;
  if (isTauri) {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
      return true;
    } catch {
      return fallbackToWindow ? openWithWindow(url) : false;
    }
  }

  return openWithWindow(url);
}
