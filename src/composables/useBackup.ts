import { invokeWithTimeout } from "./useInvoke";

export type BackupCategoryId =
  | "app_settings"
  | "reader_settings"
  | "bookshelf"
  | "bookshelf_cache"
  | "booksources"
  | "extensions"
  | "script_config"
  | "sync_state"
  | "user_fonts"
  | "other_frontend";

export interface BackupCategoryStat {
  id: BackupCategoryId;
  label: string;
  description: string;
  itemCount: number;
  byteSize: number;
}

export interface BackupInspectReport {
  categories: BackupCategoryStat[];
  totalBytes: number;
}

export interface BackupManifest {
  format: string;
  version: number;
  createdAt: number;
  appVersion: string;
  categories: BackupCategoryStat[];
}

export interface BackupCreateResult {
  outputPath: string;
  byteSize: number;
  categories: BackupCategoryStat[];
}

export interface BackupCreateDataResult {
  fileName: string;
  mime: string;
  base64: string;
  byteSize: number;
  categories: BackupCategoryStat[];
}

export interface BackupPeekReport {
  manifest: BackupManifest;
  unknownCategories: string[];
}

export interface BackupRestoreResult {
  restored: BackupCategoryStat[];
  skipped: string[];
}

const ALL_CATEGORIES: BackupCategoryId[] = [
  "app_settings",
  "reader_settings",
  "bookshelf",
  "bookshelf_cache",
  "booksources",
  "extensions",
  "script_config",
  "sync_state",
  "user_fonts",
  "other_frontend",
];

export function listAllBackupCategories(): BackupCategoryId[] {
  return [...ALL_CATEGORIES];
}

export async function inspectBackup(): Promise<BackupInspectReport> {
  return await invokeWithTimeout<BackupInspectReport>("backup_inspect", {}, 60_000);
}

export async function createBackup(
  outputPath: string,
  categories: BackupCategoryId[],
): Promise<BackupCreateResult> {
  return await invokeWithTimeout<BackupCreateResult>(
    "backup_create",
    { outputPath, categories },
    10 * 60_000,
  );
}

export async function createBackupData(
  defaultName: string,
  categories: BackupCategoryId[],
): Promise<BackupCreateDataResult> {
  return await invokeWithTimeout<BackupCreateDataResult>(
    "backup_create_data",
    { defaultName, categories },
    10 * 60_000,
  );
}

export async function peekBackup(jsonPath: string): Promise<BackupPeekReport> {
  return await invokeWithTimeout<BackupPeekReport>("backup_peek", { jsonPath }, 60_000);
}

export async function peekBackupData(base64: string): Promise<BackupPeekReport> {
  return await invokeWithTimeout<BackupPeekReport>("backup_peek_data", { base64 }, 60_000);
}

export async function restoreBackup(
  jsonPath: string,
  categories: BackupCategoryId[],
): Promise<BackupRestoreResult> {
  return await invokeWithTimeout<BackupRestoreResult>(
    "backup_restore",
    { jsonPath, categories },
    10 * 60_000,
  );
}

export async function restoreBackupData(
  base64: string,
  categories: BackupCategoryId[],
): Promise<BackupRestoreResult> {
  return await invokeWithTimeout<BackupRestoreResult>(
    "backup_restore_data",
    { base64, categories },
    10 * 60_000,
  );
}
