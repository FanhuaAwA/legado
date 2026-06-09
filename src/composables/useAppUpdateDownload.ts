import { invokeWithTimeout } from "./useInvoke";

const DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1000;

export interface AppUpdateDownloadRequest {
  requestId: string;
  url: string;
  fileName: string;
  digest?: string;
  downloadOptions?: AppUpdateDownloadOptions;
}

export interface AppUpdateDownloadOptions {
  timeoutSecs?: number;
  maxConnections?: number;
  minPartSize?: number;
  maxRetries?: number;
  retryDelayMs?: number;
  progressIntervalMs?: number;
  useRange?: boolean;
  cleanupParts?: boolean;
  cleanupTempOnError?: boolean;
}

export interface AppUpdateDownloadResult {
  requestId: string;
  localPath: string;
  fileName: string;
  size: number;
  digest: string;
}

export type AppUpdateDownloadStatus = "started" | "progress" | "done" | "error";

export interface AppUpdateDownloadProgress {
  requestId: string;
  status: AppUpdateDownloadStatus;
  mode: "single" | "multipart" | "unknown";
  fileName: string;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number | null;
  localPath?: string;
  error?: string;
}

export async function downloadAppUpdate(
  request: AppUpdateDownloadRequest,
): Promise<AppUpdateDownloadResult> {
  return invokeWithTimeout<AppUpdateDownloadResult>(
    "app_update_download",
    { request },
    DOWNLOAD_TIMEOUT_MS,
  );
}

export async function installDownloadedAppUpdate(path: string): Promise<void> {
  await invokeWithTimeout<void>("app_update_install_downloaded_file", { path }, 30_000);
}
