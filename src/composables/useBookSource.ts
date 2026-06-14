import { invokeWithTimeout } from "./useInvoke";
import { useCapabilities } from "./useCapabilities";

// ── 类型定义（与 Rust BookSourceMeta 对应）────────────────────────────────

export interface BookSourceMeta {
  sourceKey: string;
  uuid: string;
  fileName: string;
  name: string;
  /** 主 URL（第一个 @url） */
  url: string;
  /** 全部 URL（含主 URL；多镜像时有多条） */
  urls: string[];
  /** 主页 URL（来自 @homepage / @homeurl，用于展示名称外链） */
  homepageUrl?: string;
  author?: string;
  logo?: string;
  /** 多行描述（多条 @description 以换行拼接） */
  description?: string;
  enabled: boolean;
  fileSize: number;
  modifiedAt: number;
  /** 该书源所在目录的绝对路径 */
  sourceDir: string;
  /** 书源类型："novel"（小说，默认）| "comic"（漫画）| "video"（视频）| "music"（音乐/有声书） */
  sourceType: string;
  /** 版本号（用于更新检测） */
  version: string;
  /** 自动更新 URL（可选，指向远程 .js 文件地址） */
  updateUrl?: string;
  /** 标签列表（@tags 逗号分隔，如 "笔趣阁,小说,免费小说"） */
  tags: string[];
  /** 单书源最小请求间隔（毫秒） */
  minDelayMs: number;
  /** @require 依赖 JS 的 URL 列表（按声明顺序），书源加载前会依序 eval */
  requireUrls: string[];
  /** 文本扫描检测到的顶层 explore 函数标志（无需启动 JS 引擎） */
  hasExplore?: boolean;
  /** 后端轻量扫描出的入口能力，用于避免前端再次逐源 eval */
  capabilities?: string[];
}

export interface NovelSourceRef {
  fileName: string;
  sourceDir?: string | null;
}

export interface ValidationResult {
  ok: boolean;
  errors: string[];
  warnings: string[];
}

export interface BookSourceFormatInfo {
  name: string;
  urls: string[];
  sourceType: string;
  entryFunctions: string[];
}

export interface BookSourceValidationResult extends ValidationResult {
  meta: BookSourceFormatInfo;
}

export interface LegacyJsonImportResult {
  imported: number;
  skipped: number;
  files: string[];
  errors: string[];
}

export interface BookSourceDeleteItem {
  fileName: string;
  sourceDir?: string | null;
}

export interface BookSourceDeleteError extends BookSourceDeleteItem {
  message: string;
}

export interface BookSourceBatchDeleteResult {
  deleted: BookSourceDeleteItem[];
  errors: BookSourceDeleteError[];
}

const BOOK_SOURCE_META_SCAN_LINES = 80;
const BOOK_SOURCE_ENTRY_FUNCTIONS = ["search", "bookInfo", "toc", "content", "explore"] as const;
const SUPPORTED_BOOK_SOURCE_TYPES = new Set([
  "novel",
  "comic",
  "video",
  "music",
  "audio",
  "webpage",
  "web",
  "小说",
  "漫画",
  "视频",
  "音乐",
  "听书",
  "有声",
  "网页",
]);
const capabilities = useCapabilities();

async function isBookSourceCapabilitySupported(key: "comicCache" | "repository"): Promise<boolean> {
  const state = await capabilities.loadCapabilities();
  return state[key].supported;
}

function normalizeMetaValue(value: string): string {
  const trimmed = value.trim();
  if (
    trimmed.length >= 2 &&
    ((trimmed.startsWith('"') && trimmed.endsWith('"')) ||
      (trimmed.startsWith("'") && trimmed.endsWith("'")))
  ) {
    return trimmed.slice(1, -1).trim();
  }
  return trimmed;
}

function normalizeMetaLine(line: string): string {
  return line
    .trimStart()
    .replace(/^\uFEFF/, "")
    .trimStart()
    .replace(/^\/\//, "")
    .trimStart()
    .replace(/^\*/, "")
    .trim();
}

function readMetaTag(line: string, key: string): string | null {
  const normalized = normalizeMetaLine(line);
  if (
    normalized.startsWith(key) &&
    (normalized.length === key.length || /\s/.test(normalized.charAt(key.length)))
  ) {
    return normalizeMetaValue(normalized.slice(key.length).trim());
  }

  const marker = `${key} `;
  const pos = normalized.indexOf(marker);
  if (pos < 0) {
    return null;
  }
  const rest = normalized.slice(pos + marker.length);
  const end = rest.search(/[@*/]/);
  return normalizeMetaValue(rest.slice(0, end >= 0 ? end : undefined).trim());
}

function readJsonStringField(line: string, key: string): string | null {
  const match = line.match(new RegExp(`"${key}"\\s*:\\s*"((?:\\\\.|[^"\\\\])*)"`));
  if (!match?.[1]) {
    return null;
  }
  try {
    return normalizeMetaValue(JSON.parse(`"${match[1]}"`) as string);
  } catch {
    return normalizeMetaValue(match[1]);
  }
}

function readBookSourceMetaValues(
  content: string,
  tagName: string,
  jsonFieldNames: string[] = [],
): string[] {
  const values: string[] = [];
  for (const line of content.split(/\r?\n/).slice(0, BOOK_SOURCE_META_SCAN_LINES)) {
    const tagValue = readMetaTag(line, tagName);
    if (tagValue) {
      values.push(tagValue);
    }
    for (const fieldName of jsonFieldNames) {
      const jsonValue = readJsonStringField(line, fieldName);
      if (jsonValue) {
        values.push(jsonValue);
      }
    }
  }
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

function isValidHttpUrl(value: string): boolean {
  try {
    const parsed = new URL(normalizeMetaValue(value));
    return (parsed.protocol === "http:" || parsed.protocol === "https:") && !!parsed.hostname;
  } catch {
    return false;
  }
}

function findBookSourceEntryFunctions(content: string): string[] {
  return BOOK_SOURCE_ENTRY_FUNCTIONS.filter((name) => {
    const pattern = new RegExp(
      `(?:async\\s+function\\s+${name}\\b|function\\s+${name}\\b|(?:const|let|var)\\s+${name}\\s*=)`,
    );
    return pattern.test(content);
  });
}

function validateJavaScriptSyntax(content: string): string | null {
  try {
    const source = content.replace(/^\uFEFF/, "").replace(/^#!.*(?:\r?\n|$)/, "");
    // oxlint-disable-next-line no-implied-eval -- book-source scripts are user-authored code; this only compiles them for syntax validation.
    const syntaxCheck = new Function(source);
    void syntaxCheck;
    return null;
  } catch (e: unknown) {
    return e instanceof Error ? e.message : String(e);
  }
}

export function formatValidationIssues(title: string, issues: string[], maxItems = 4): string {
  if (!issues.length) {
    return title;
  }
  const visible = issues.slice(0, maxItems).join("；");
  const more = issues.length > maxItems ? `；另有 ${issues.length - maxItems} 项` : "";
  return `${title}：${visible}${more}`;
}

export function validateBookSourceFileName(fileName: string): string[] {
  const name = fileName.trim();
  const errors: string[] = [];
  if (!name) {
    errors.push("文件名不能为空");
    return errors;
  }
  if (!name.toLowerCase().endsWith(".js")) {
    errors.push("文件名必须以 .js 结尾");
  }
  if (/[\\/:*?"<>|]/.test(name) || name === "." || name === ".." || name.includes("..")) {
    errors.push("文件名不能包含路径或特殊字符");
  }
  return errors;
}

export function validateBookSourceContent(
  content: string,
  options: { fileName?: string } = {},
): BookSourceValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];
  const trimmed = content.trim();
  const name = readBookSourceMetaValues(content, "@name", ["bookSourceName"])[0] ?? "";
  const urls = readBookSourceMetaValues(content, "@url", ["bookSourceUrl"]);
  const sourceType = readBookSourceMetaValues(content, "@type", ["bookSourceType"])[0] ?? "novel";
  const entryFunctions = findBookSourceEntryFunctions(content);

  if (!trimmed) {
    errors.push("书源内容为空");
  }

  const head = trimmed.slice(0, 200).toLowerCase();
  if (/^<(?:!doctype\s+html|html|head|body)\b/.test(head)) {
    errors.push("当前内容看起来是网页 HTML，请填写或导入书源 JS 文件地址");
  }
  if (/^[{[]/.test(trimmed)) {
    errors.push("当前内容看起来是 JSON，不是可直接运行的书源 JS 文件");
  }

  if (!name) {
    const hint = options.fileName ? `（${options.fileName}）` : "";
    errors.push(`缺少书源名称${hint}，请在文件头添加 // @name 名称`);
  }

  if (!urls.length) {
    errors.push("缺少源站地址，请在文件头添加 // @url https://example.com");
  }
  for (const url of urls) {
    if (!isValidHttpUrl(url)) {
      errors.push(`源站地址格式不正确：${url}`);
    }
  }

  const normalizedType = sourceType.trim().toLowerCase();
  if (normalizedType && !SUPPORTED_BOOK_SOURCE_TYPES.has(normalizedType)) {
    warnings.push(`书源类型「${sourceType}」不常见，建议使用 novel/comic/video/music/webpage`);
  }

  if (!entryFunctions.length) {
    errors.push("缺少可调用函数，请至少提供 search/bookInfo/toc/content/explore 之一");
  }

  if (trimmed) {
    const syntaxError = validateJavaScriptSyntax(content);
    if (syntaxError) {
      errors.push(`JavaScript 语法错误：${syntaxError}`);
    }
  }

  return {
    ok: errors.length === 0,
    errors,
    warnings,
    meta: {
      name,
      urls,
      sourceType,
      entryFunctions,
    },
  };
}

// ── API 封装 ─────────────────────────────────────────────────────────────

/** 获取书源目录绝对路径 */
export async function getBookSourceDir(): Promise<string> {
  return invokeWithTimeout<string>("booksource_get_dir", {}, 10000);
}

/** 获取所有书源目录（内置 + 外部） */
export async function getBookSourceDirs(): Promise<string[]> {
  return invokeWithTimeout<string[]>("booksource_get_dirs", {}, 10000);
}

/** 添加外部书源目录 */
export async function addBookSourceDir(dirPath: string): Promise<void> {
  return invokeWithTimeout<void>("booksource_add_dir", { dirPath }, 10000);
}

/** 移除外部书源目录 */
export async function removeBookSourceDir(dirPath: string): Promise<void> {
  return invokeWithTimeout<void>("booksource_remove_dir", { dirPath }, 10000);
}

/** 弹出系统目录选择对话框，返回选择的路径（取消返回空字符串） */
export async function pickBookSourceDir(): Promise<string> {
  return invokeWithTimeout<string>("booksource_pick_dir", {}, 60000);
}

/** 列举所有已安装书源（一次性全量返回，不建议在书源数量 > 500 时使用） */
export async function listBookSources(): Promise<BookSourceMeta[]> {
  return invokeWithTimeout<BookSourceMeta[]>("booksource_list", {}, 15000);
}

/**
 * 流式列举书源：立即返回，后台通过 `booksource:batch` 事件分批推送。
 * 调用方需在调用本函数前监听 `booksource:batch` 事件，并通过 `requestId` 过滤。
 */
export async function listBookSourcesStreaming(requestId: string, force = false): Promise<void> {
  return invokeWithTimeout<void>("booksource_list_streaming", { requestId, force }, 70000);
}

/** 读取单个书源 JS 内容 */
export async function readBookSource(fileName: string, sourceDir?: string): Promise<string> {
  return invokeWithTimeout<string>(
    "booksource_read",
    { fileName, sourceDir: sourceDir ?? null },
    10000,
  );
}

/** 保存书源 JS 文件（新建或覆盖），fileName 不含路径 */
export async function saveBookSource(
  fileName: string,
  content: string,
  sourceDir?: string,
): Promise<void> {
  const validationErrors = [
    ...validateBookSourceFileName(fileName),
    ...validateBookSourceContent(content, { fileName }).errors,
  ];
  if (validationErrors.length) {
    throw new Error(formatValidationIssues("书源格式不正确", validationErrors));
  }
  return invokeWithTimeout<void>(
    "booksource_save",
    { fileName, content, sourceDir: sourceDir ?? null },
    10000,
  );
}

/** 将开源阅读/Legado Android JSON 书源内容转换为 Tauri JS 书源并安装。 */
export async function importLegacyJsonText(
  content: string,
  smartExploreSubCategories = false,
): Promise<LegacyJsonImportResult> {
  return invokeWithTimeout<LegacyJsonImportResult>(
    "booksource_import_legacy_json_text",
    { content, smartExploreSubCategories },
    70000,
  );
}

/** 从远程 URL 下载开源阅读/Legado Android JSON 书源，转换为 Tauri JS 书源并安装。 */
export async function importLegacyJsonUrl(
  url: string,
  smartExploreSubCategories = false,
): Promise<LegacyJsonImportResult> {
  return invokeWithTimeout<LegacyJsonImportResult>(
    "booksource_import_legacy_json_url",
    { url, smartExploreSubCategories },
    70000,
  );
}

/** 删除书源文件 */
export async function deleteBookSource(fileName: string, sourceDir?: string): Promise<void> {
  return invokeWithTimeout<void>(
    "booksource_delete",
    { fileName, sourceDir: sourceDir ?? null },
    10000,
  );
}

/** 批量删除书源文件，后端会去重并只触发一次列表刷新事件 */
export async function deleteBookSources(
  items: BookSourceDeleteItem[],
): Promise<BookSourceBatchDeleteResult> {
  return invokeWithTimeout<BookSourceBatchDeleteResult>(
    "booksource_delete_batch",
    {
      items: items.map((item) => ({
        fileName: item.fileName,
        sourceDir: item.sourceDir ?? null,
      })),
    },
    60000,
  );
}

/** 切换书源启用/禁用（修改文件头部 @enabled 标记） */
export async function toggleBookSource(
  fileName: string,
  enabled: boolean,
  sourceDir?: string,
): Promise<void> {
  return invokeWithTimeout<void>(
    "booksource_toggle",
    { fileName, enabled, sourceDir: sourceDir ?? null },
    10000,
  );
}

/** 用 VS Code 打开指定书源文件 */
export async function openInVscode(fileName: string, sourceDir?: string): Promise<void> {
  return invokeWithTimeout<void>(
    "booksource_open_in_vscode",
    { fileName, sourceDir: sourceDir ?? null },
    10000,
  );
}

/**
 * 在 Android 系统默认编辑器中打开书源文件（通过 FileProvider + ACTION_EDIT Intent）。
 * 外部编辑器保存后，文件监听器会自动检测变更并刷新编辑器内容。
 */
export async function openInExternalEditor(fileName: string, sourceDir?: string): Promise<void> {
  const path = await invokeWithTimeout<string>(
    "booksource_resolve_path",
    { fileName, sourceDir: sourceDir ?? null },
    10000,
  );
  const bridge = (window as unknown as Record<string, unknown>)["LegadoAndroidInput"] as
    | { openFileInEditor(p: string): string }
    | undefined;
  if (!bridge?.openFileInEditor) {
    throw new Error("外部编辑器功能仅在 Android 上可用");
  }
  const error = bridge.openFileInEditor(path);
  if (error) {
    throw new Error(error);
  }
}

/**
 * 装载书源文件并通过 Boa JS 引擎执行 entryCode。
 * - entryCode 为空时：返回书源内定义的所有顶层函数列表。
 * - entryCode 非空时：在书源作用域内执行该代码，返回结果字符串。
 */
export async function evalBookSource(
  fileName: string,
  entryCode?: string,
  sourceDir?: string,
): Promise<string> {
  return invokeWithTimeout<string>(
    "booksource_eval",
    { fileName, entryCode: entryCode ?? null, sourceDir: sourceDir ?? null },
    20000,
  );
}

/** 小说搜索：调用指定书源的 search 入口。 */
export async function searchNovelSource(
  source: NovelSourceRef,
  keyword: string,
  page = 1,
): Promise<unknown> {
  return invokeWithTimeout(
    "booksource_search",
    {
      fileName: source.fileName,
      keyword,
      page,
      sourceDir: source.sourceDir ?? null,
    },
    35000,
  );
}

/** 小说详情：调用指定书源的 bookInfo 入口。 */
export async function getNovelBookInfo(source: NovelSourceRef, bookUrl: string): Promise<unknown> {
  return invokeWithTimeout(
    "booksource_book_info",
    {
      fileName: source.fileName,
      bookUrl,
      sourceDir: source.sourceDir ?? null,
    },
    35000,
  );
}

/** 小说目录：调用指定书源的 toc/chapterList 入口。 */
export async function getNovelChapterList(
  source: NovelSourceRef,
  bookUrl: string,
  taskId?: string,
): Promise<unknown> {
  return invokeWithTimeout(
    "booksource_chapter_list",
    {
      fileName: source.fileName,
      bookUrl,
      taskId: taskId ?? null,
      sourceDir: source.sourceDir ?? null,
    },
    125000,
  );
}

/** 小说正文：调用指定书源的 content/chapterContent 入口。 */
export async function getNovelChapterContent(
  source: NovelSourceRef,
  chapterUrl: string,
): Promise<unknown> {
  return invokeWithTimeout(
    "booksource_chapter_content",
    {
      fileName: source.fileName,
      chapterUrl,
      sourceDir: source.sourceDir ?? null,
      categoryParams: null,
    },
    35000,
  );
}

/** 直接执行任意 JS 代码（Boa 引擎），返回结果字符串（调试用途） */
export async function jsEval(code: string): Promise<string> {
  return invokeWithTimeout<string>("js_eval", { code }, 15000);
}

/**
 * 调用书源的 explore 函数（发现页）
 * @param fileName 书源文件名（含 .js 后缀）
 * @param category 分类名称（"GETALL" 获取分类列表，具体分类名获取书籍）
 * @param page     页码，从 1 开始
 */
export async function exploreBookSource(
  fileName: string,
  category: string,
  page = 1,
  sourceDir?: string,
): Promise<unknown> {
  return invokeWithTimeout(
    "booksource_explore",
    { fileName, page, category, sourceDir: sourceDir ?? null },
    35000,
  );
}

// ── 脚本配置持久化（对应 Rust script_config 命令） ────────────────────────

/**
 * 读取脚本配置值
 * @param scope  作用域标识（通常为书源/扩展文件名）
 * @param key    配置键名
 * @returns      字符串值，键不存在时返回空字符串
 */
export async function configRead(scope: string, key: string): Promise<string> {
  return invokeWithTimeout<string>("config_read", { scope, key }, 10000);
}

export type ScriptConfigJsonValue =
  | string
  | number
  | boolean
  | null
  | { [key: string]: ScriptConfigJsonValue }
  | ScriptConfigJsonValue[];

/**
 * 写入脚本配置值。
 * - 传入字符串时沿用旧字符串接口
 * - 传入对象/数组/数值/布尔/null 时走原生 JSON 接口，避免额外 JSON 编码
 */
export async function configWrite(
  scope: string,
  key: string,
  value: ScriptConfigJsonValue,
): Promise<void> {
  if (typeof value === "string") {
    return invokeWithTimeout<void>("config_write", { scope, key, value }, 10000);
  }
  return invokeWithTimeout<void>("config_write_json", { scope, key, value }, 10000);
}

/** 删除指定配置键 */
export async function configDeleteKey(scope: string, key: string): Promise<void> {
  return invokeWithTimeout<void>("config_delete_key", { scope, key }, 10000);
}

/** 读取某 scope 下的所有配置（返回 JSON 字符串） */
export async function configReadAll(scope: string): Promise<string> {
  return invokeWithTimeout<string>("config_read_all", { scope }, 10000);
}

/** 读取脚本配置的原生 JSON 值；键不存在时返回 null */
export async function configReadJson<T extends ScriptConfigJsonValue = ScriptConfigJsonValue>(
  scope: string,
  key: string,
): Promise<T | null> {
  return invokeWithTimeout<T | null>("config_read_json", { scope, key }, 10000);
}

/** 清除某 scope 下的所有配置 */
export async function configClear(scope: string): Promise<void> {
  return invokeWithTimeout<void>("config_clear", { scope }, 10000);
}

// ── 字节数组配置（对应 Rust config_read_bytes / config_write_bytes） ───────

/**
 * 读取字节数组配置值
 * @param scope 作用域标识
 * @param key   配置键名
 * @returns     Uint8Array，键不存在时返回空数组
 */
export async function configReadBytes(scope: string, key: string): Promise<Uint8Array> {
  const arr = await invokeWithTimeout<number[]>("config_read_bytes", { scope, key }, 10000);
  return new Uint8Array(arr);
}

/**
 * 写入字节数组配置值
 * @param scope 作用域标识
 * @param key   配置键名
 * @param value Uint8Array 数据（编码由调用方负责）
 */
export async function configWriteBytes(
  scope: string,
  key: string,
  value: Uint8Array,
): Promise<void> {
  return invokeWithTimeout<void>(
    "config_write_bytes",
    { scope, key, value: Array.from(value) },
    10000,
  );
}

// ── 漫画图片下载 & 缓存 ──────────────────────────────────────────────────

/**
 * 获取漫画章节的可渲染图片源列表，并在后台继续顺序缓存缺失页
 * @param fileName    书源文件名
 * @param chapterUrl  章节 URL（用于生成缓存目录）
 * @param imageUrls   图片 URL 数组
 * @param cacheEnabled 为 false 时仍走后端 proxy 补请求头，但不读写漫画图片缓存
 * @returns 可直接渲染的图片源数组：
 *          已缓存页返回本地路径，未缓存页返回后端 proxy URL
 */
export async function comicDownloadImages(
  fileName: string,
  bookUrl: string,
  bookName: string,
  chapterUrl: string,
  chapterIndex: number,
  imageUrls: string[],
  cacheEnabled?: boolean,
): Promise<string[]> {
  if (!(await isBookSourceCapabilitySupported("comicCache"))) {
    return imageUrls;
  }

  return invokeWithTimeout<string[]>(
    "comic_download_images",
    {
      fileName,
      bookUrl,
      bookName,
      chapterUrl,
      chapterIndex,
      imageUrls,
      cacheEnabled,
    },
    60000,
  );
}

/**
 * 获取章节已缓存的所有图片原始像素尺寸。
 * 返回数组长度 = 已记录页数，元素为 [width, height] 或 null（该页未缓存/未记录）。
 * 前端可据此精确计算 scrollTop，无需等待图片在视口内加载。
 */
export async function comicGetPageSizes(
  fileName: string,
  bookUrl: string,
  bookName: string,
  chapterIndex: number,
): Promise<Array<[number, number] | null>> {
  if (!(await isBookSourceCapabilitySupported("comicCache"))) {
    return [];
  }

  return invokeWithTimeout<Array<[number, number] | null>>(
    "comic_get_page_sizes",
    { fileName, bookUrl, bookName, chapterIndex },
    5000,
  );
}

/**
 * 获取已缓存的单页图片（base64 Data URL）
 * @param fileName    书源文件名
 * @param chapterUrl  章节 URL
 * @param pageIndex   页码索引（从 0 开始）
 */
export async function comicGetCachedPage(
  fileName: string,
  bookUrl: string,
  bookName: string,
  chapterIndex: number,
  pageIndex: number,
): Promise<string> {
  await capabilities.requireCapability("comicCache");

  return invokeWithTimeout<string>(
    "comic_get_cached_page",
    { fileName, bookUrl, bookName, chapterIndex, pageIndex },
    10000,
  );
}

/**
 * 清理单章漫画图片缓存（按 fileName + chapterUrl 定位）
 * @returns 释放的字节数
 */
export async function comicCacheClearChapter(
  fileName: string,
  bookUrl: string,
  bookName: string,
  chapterIndex: number,
): Promise<number> {
  if (!(await isBookSourceCapabilitySupported("comicCache"))) {
    return 0;
  }

  return invokeWithTimeout<number>(
    "comic_cache_clear_chapter",
    { fileName, bookUrl, bookName, chapterIndex },
    15000,
  );
}

/**
 * 清理漫画缓存
 * @param fileName  书源文件名（为 null 时清理全部）
 * @returns 释放的字节数
 */
export async function comicCacheClear(fileName?: string): Promise<number> {
  if (!(await isBookSourceCapabilitySupported("comicCache"))) {
    return 0;
  }

  return invokeWithTimeout<number>("comic_cache_clear", { fileName: fileName ?? null }, 30000);
}

/**
 * 获取漫画缓存总大小（字节）
 */
export async function comicCacheSize(): Promise<number> {
  if (!(await isBookSourceCapabilitySupported("comicCache"))) {
    return 0;
  }

  return invokeWithTimeout<number>("comic_cache_size", {}, 10000);
}

// ── 书源更新检测 ─────────────────────────────────────────────────────────

export interface UpdateCheckResult {
  fileName: string;
  uuid: string;
  hasUpdate: boolean;
  localVersion: string;
  remoteVersion: string;
}

/**
 * 检测单个书源是否有更新（需要书源设置了 @updateUrl）
 */
export async function checkBookSourceUpdate(fileName: string): Promise<UpdateCheckResult> {
  await capabilities.requireCapability("repository");

  return invokeWithTimeout<UpdateCheckResult>("booksource_check_update", { fileName }, 20000);
}

/**
 * 从 @updateUrl 拉取最新内容并覆盖本地文件
 */
export async function applyBookSourceUpdate(fileName: string): Promise<void> {
  await capabilities.requireCapability("repository");

  return invokeWithTimeout<void>("booksource_apply_update", { fileName }, 20000);
}

/** 将任意字符串转为合法 JS 文件名 */
export function toSafeFileName(name: string): string {
  return name.replace(/[\\/:*?"<>|]/g, "_").replace(/\s+/g, "_") + ".js";
}

/** 生成新书源 JS 文件的模板内容 */
export function newBookSourceTemplate(name = "新书源", url = "https://") {
  return `// @name        ${name}
// @version     1.0.0
// @author      作者名
// @url         ${url}
// @logo        default
// @enabled     true
// @tags        免费,小说
// @description 书源简介

const BASE_URL = '${url}'

// ── 搜索 ─────────────────────────────────────────────────────
// 返回 BookItem[]
async function search(key, page) {
  const resp = await legado.http.get(
    \`\${BASE_URL}/api/search?keyword=\${encodeURIComponent(key)}&page=\${page}\`
  )
  const json = JSON.parse(resp)
  return (json.data?.list ?? []).map(book => ({
    name:    book.name,
    author:  book.author,
    coverUrl: book.cover,
    intro:   book.intro,
    bookUrl: \`\${BASE_URL}/book/\${book.id}\`,
  }))
}

// ── 书籍详情 ──────────────────────────────────────────────────
// 返回 BookItem（含 tocUrl）
async function bookInfo(bookUrl) {
  const resp = await legado.http.get(bookUrl)
  const json = JSON.parse(resp)
  return {
    name:    json.data.name,
    author:  json.data.author,
    coverUrl: json.data.cover,
    intro:   json.data.intro,
    bookUrl,
    tocUrl:  bookUrl,
  }
}

// ── 章节目录 ──────────────────────────────────────────────────
// 返回 ChapterInfo[]；VIP 章节可额外返回 vip/price/currency
async function toc(tocUrl) {
  const resp = await legado.http.get(tocUrl)
  const json = JSON.parse(resp)
  return (json.data?.chapters ?? []).map(ch => ({
    name: ch.title,
    url:  \`\${BASE_URL}/chapter/\${ch.id}\`,
    vip:  !!ch.vip,
    price: ch.price,
    currency: ch.currency,
  }))
}

// ── 章节正文 ──────────────────────────────────────────────────
// 返回纯文本字符串
async function content(chapterUrl) {
  const resp = await legado.http.get(chapterUrl)
  const json = JSON.parse(resp)
  return json.data?.content ?? ''
}

// ── VIP 章节购买（可选） ──────────────────────────────────────
// 仅在用户确认购买 VIP 章节后调用；免费书源可删除此函数
// async function purchaseChapter(chapterUrl, chapter) {
//   const resp = await legado.http.post(
//     \`\${BASE_URL}/api/chapter/buy\`,
//     JSON.stringify({ chapterId: chapter?.id ?? chapterUrl }),
//     { 'Content-Type': 'application/json' }
//   )
//   const json = JSON.parse(resp)
//   return { ok: !!json.success, message: json.message }
// }

// ── 发现页（可选） ────────────────────────────────────────────
// 返回 ExploreItem[]，不需要时可删除此函数
// async function explore(page) { ... }
`;
}

export function newVideoSourceTemplate(name = "新视频源", url = "https://") {
  return `// @name        ${name}
// @version     1.0.0
// @author      作者名
// @type        video
// @url         ${url}
// @logo        default
// @enabled     true
// @tags        免费,视频
// @description 视频源简介

// ─────────────────────────────────────────────────────────────
//  视频书源 — content() 返回视频播放地址
//  支持格式：mp4 直链 / m3u8 (HLS) / JSON 对象 / 本地代理
//
//  content() 返回值说明：
//    1. 简单 URL 字符串:  "https://example.com/video.mp4"
//    2. JSON 字符串:
//       {
//         "url":       "https://example.com/index.m3u8",
//         "type":      "hls",              // 可选: "hls" | "dash" | "mp4" | "flv" | "proxy"
//         "proxyType": "mp4",              // type 为 "proxy" 时可选，提示真实流类型
//         "proxyConcurrency": 8,             // type 为 "proxy" 时可选，小于 100MB 的 Range 请求会按此并发拆分
//         "headers":   { "Referer": "..." },// 可选
//         "qualities": [                    // 可选: 多清晰度
//           { "label": "1080P", "url": "https://..." },
//           { "label": "720P",  "url": "https://..." }
//         ],
//         "subtitles": [                    // 可选: 字幕
//           { "label": "中文", "url": "https://.../zh.vtt", "lang": "zh" }
//         ]
//       }
// ─────────────────────────────────────────────────────────────
const BASE_URL = '${url}'
const SCOPE    = '${name}'

/**
 * 搜索影视
 * @param {string} key   搜索关键词
 * @param {number} page  页码（从 1 开始）
 * @returns {BookItem[]}
 */
async function search(key, page) {
  const resp = await legado.http.get(
    \`\${BASE_URL}/api/search?keyword=\${encodeURIComponent(key)}&page=\${page}\`
  )
  const json = JSON.parse(resp)
  return (json.data?.list ?? []).map(item => ({
    name:     item.name,
    author:   item.director ?? item.author ?? '',
    coverUrl: item.cover,
    intro:    item.intro,
    bookUrl:  \`\${BASE_URL}/video/\${item.id}\`,
    kind:     item.category,
  }))
}

/**
 * 影视详情
 * @param {string} bookUrl
 * @returns {BookItem}
 */
async function bookInfo(bookUrl) {
  const resp = await legado.http.get(bookUrl)
  const json = JSON.parse(resp)
  return {
    name:     json.data.name,
    author:   json.data.director ?? '',
    coverUrl: json.data.cover,
    intro:    json.data.intro,
    bookUrl,
    tocUrl:   bookUrl,
  }
}

/**
 * 剧集 / 选集列表
 * 电影返回单集即可，电视剧返回多集
 * @param {string} tocUrl
 * @returns {ChapterInfo[]}
 */
async function toc(tocUrl) {
  const resp = await legado.http.get(\`\${tocUrl}/episodes\`)
  const json = JSON.parse(resp)
  return (json.data?.episodes ?? []).map(ep => ({
    name: ep.title,
    url:  \`\${BASE_URL}/play/\${ep.id}\`,
  }))
}

/**
 * 获取播放地址
 * 返回视频 URL 字符串，或包含 url/type/headers/qualities 的 JSON 字符串
 * @param {string} chapterUrl
 * @returns {string}
 */
async function content(chapterUrl) {
  const resp = await legado.http.get(chapterUrl)
  const json = JSON.parse(resp)
  // 直接返回播放地址
  return json.data?.playUrl ?? ''
}

/**
 * 发现页（可选）
 * @param {number} page  页码（从 1 开始）
 * @returns {ExploreItem[]}
 */
async function explore(page) {
  const resp = await legado.http.get(\`\${BASE_URL}/api/discover?page=\${page}\`)
  const json = JSON.parse(resp)
  return (json.data?.list ?? []).map(item => ({
    name:     item.name,
    author:   item.director ?? '',
    bookUrl:  \`\${BASE_URL}/video/\${item.id}\`,
    coverUrl: item.cover,
    intro:    item.intro,
    type:     item.category,
  }))
}
`;
}

// ── 书源仓库 ─────────────────────────────────────────────────────────────

/** 仓库中单个书源的元数据（对应 Rust RepoSourceInfo） */
export interface RepoSourceInfo {
  uuid?: string;
  name: string;
  version: string;
  author: string;
  url: string;
  logo: string;
  description: string;
  tags: string[];
  enabled: boolean;
  fileName: string;
  downloadUrl: string;
  fileSize: number;
  updatedAt: string;
}

/** 仓库顶层清单（对应 Rust RepoManifest） */
export interface RepoManifest {
  name: string;
  version: string;
  url?: string;
  updatedAt: string;
  sources: RepoSourceInfo[];
}

export function validateRepositoryUrl(url: string): ValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];
  const value = url.trim();

  if (!value) {
    errors.push("仓库 URL 不能为空");
    return { ok: false, errors, warnings };
  }

  try {
    const parsed = new URL(value);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      errors.push("仓库 URL 仅支持 http 或 https");
    }
    if (!parsed.hostname) {
      errors.push("仓库 URL 缺少域名");
    }
    if (!parsed.pathname.toLowerCase().endsWith(".json")) {
      warnings.push("仓库地址通常应指向 JSON 清单，若是接口地址请确保返回仓库 JSON");
    }
  } catch {
    errors.push("仓库 URL 格式不正确，请输入完整的 http(s) 地址");
  }

  return { ok: errors.length === 0, errors, warnings };
}

function resolveRepositoryDownloadUrl(downloadUrl: string, repositoryUrl: string): string | null {
  try {
    return new URL(downloadUrl, repositoryUrl).href;
  } catch {
    return null;
  }
}

export function validateRepositoryManifest(
  manifest: RepoManifest,
  repositoryUrl: string,
): ValidationResult {
  const errors: string[] = [];
  const warnings: string[] = [];

  if (!manifest.name?.trim()) {
    errors.push("仓库清单缺少 name");
  }
  if (!manifest.version?.trim()) {
    errors.push("仓库清单缺少 version");
  }
  if (!manifest.updatedAt?.trim()) {
    errors.push("仓库清单缺少 updatedAt");
  }
  if (manifest.url && !isValidHttpUrl(manifest.url)) {
    warnings.push("仓库清单 url 不是有效的 http(s) 地址");
  }
  if (!Array.isArray(manifest.sources)) {
    errors.push("仓库清单缺少 sources 数组");
    return { ok: false, errors, warnings };
  }
  if (!manifest.sources.length) {
    warnings.push("仓库清单中没有书源条目");
  }

  const sourceErrors: string[] = [];
  manifest.sources.forEach((source, index) => {
    const label = `sources[${index + 1}]`;
    if (!source.name?.trim()) {
      sourceErrors.push(`${label} 缺少 name`);
    }
    if (!source.fileName?.trim()) {
      sourceErrors.push(`${label} 缺少 fileName`);
    } else if (!source.fileName.toLowerCase().endsWith(".js")) {
      sourceErrors.push(`${label}.fileName 必须以 .js 结尾`);
    }
    if (!source.downloadUrl?.trim()) {
      sourceErrors.push(`${label} 缺少 downloadUrl`);
    } else {
      const resolved = resolveRepositoryDownloadUrl(source.downloadUrl, repositoryUrl);
      if (!resolved || !isValidHttpUrl(resolved)) {
        sourceErrors.push(`${label}.downloadUrl 不是有效的 http(s) 地址`);
      }
    }
    if (source.url && !isValidHttpUrl(source.url)) {
      warnings.push(`${label}.url 不是有效的 http(s) 地址`);
    }
  });

  errors.push(...sourceErrors.slice(0, 8));
  if (sourceErrors.length > 8) {
    errors.push(`还有 ${sourceErrors.length - 8} 个书源条目格式错误`);
  }

  return { ok: errors.length === 0, errors, warnings };
}

function stringifyErrorField(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  try {
    return JSON.stringify(value) ?? Object.prototype.toString.call(value);
  } catch {
    return Object.prototype.toString.call(value);
  }
}

function clipErrorText(value: string, limit = 360): string {
  if (value.length <= limit) {
    return value;
  }
  return `${value.slice(0, limit)}...`;
}

export function formatBookSourceError(error: unknown, fallback = "操作失败"): string {
  if (error instanceof Error) {
    return clipErrorText(error.message || fallback);
  }
  if (typeof error === "string") {
    return clipErrorText(error || fallback);
  }
  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    const message = stringifyErrorField(record.message).trim();
    const detail = stringifyErrorField(record.detail).trim();
    const code = stringifyErrorField(record.code).trim();
    const nested = stringifyErrorField(record.error).trim();
    const parts: string[] = [];

    if (message) {
      parts.push(message);
    }
    if (detail && detail !== message) {
      parts.push(detail);
    }
    if (!message && nested) {
      parts.push(nested);
    }
    if (code) {
      parts.push(`错误码: ${code}`);
    }
    if (parts.length) {
      return clipErrorText(parts.join("；"));
    }
  }

  const raw = stringifyErrorField(error);
  return clipErrorText(raw && raw !== "undefined" ? raw : fallback);
}

export function formatRepositoryError(error: unknown): string {
  const raw = formatBookSourceError(error, "仓库请求失败");
  if (!raw || raw === "undefined") {
    return "仓库请求失败";
  }
  if (/JSON|json|missing field|invalid type|expected|EOF|解析失败/.test(raw)) {
    return "仓库格式不正确：请确认地址返回的是仓库 JSON 清单，且包含 name、version、updatedAt、sources 字段";
  }
  if (/HTML|doctype|网页/.test(raw)) {
    return "仓库地址返回了网页内容，请填写仓库 JSON 地址";
  }
  return raw;
}

export function getBookSourceIdentity(source: { uuid?: string | null; name: string }): string {
  return source.uuid?.trim() ?? source.name.trim();
}

export function hasExplicitBookSourceUuid(source: { uuid?: string | null }): boolean {
  return !!source.uuid?.trim();
}

export interface RepoSourceSyncResult {
  fileName: string;
  uuid: string;
  isConsistent: boolean;
  localVersion: string;
  remoteVersion: string;
}

export interface RemoteBookSourcePreview {
  downloadUrl: string;
  meta: BookSourceMeta;
  hasExplicitUuid: boolean;
}

/** 拉取远程仓库 JSON */
export async function fetchRepository(url: string): Promise<RepoManifest> {
  await capabilities.requireCapability("repository");

  return invokeWithTimeout<RepoManifest>("repository_fetch", { url }, 35000);
}

/** 从仓库下载书源 .js 文件并安装到本地 */
export async function installFromRepository(
  downloadUrl: string,
  fileName: string,
  expectedUuid?: string,
): Promise<void> {
  await capabilities.requireCapability("repository");

  return invokeWithTimeout<void>(
    "repository_install",
    { downloadUrl, fileName, expectedUuid: expectedUuid ?? null },
    35000,
  );
}

/** 下载远程书源并解析元数据，用于安装前确认 */
export async function previewRemoteBookSource(
  downloadUrl: string,
  expectedUuid?: string,
): Promise<RemoteBookSourcePreview> {
  await capabilities.requireCapability("repository");

  return invokeWithTimeout<RemoteBookSourcePreview>(
    "repository_preview_source",
    { downloadUrl, expectedUuid: expectedUuid ?? null },
    35000,
  );
}

/** 比较在线仓库书源与本地同 UUID 书源是否一致（忽略 @enabled / @uuid 行） */
export async function checkRepositorySourceSync(
  fileName: string,
  downloadUrl: string,
  expectedUuid?: string,
): Promise<RepoSourceSyncResult> {
  await capabilities.requireCapability("repository");

  return invokeWithTimeout<RepoSourceSyncResult>(
    "repository_check_source_sync",
    { fileName, downloadUrl, expectedUuid: expectedUuid ?? null },
    35000,
  );
}

// ── 书源内置测试 ─────────────────────────────────────────────────────────

/** 单个测试步骤的结果 */
export interface TestStepResult {
  step: string;
  passed: boolean;
  message: string;
  durationMs: number;
}

/** 全量测试结果 */
export interface TestRunResult {
  fileName: string;
  steps: TestStepResult[];
  allPassed: boolean;
}

/** 执行书源内置测试（依次运行 search / bookInfo / chapterList / chapterContent / explore）
 *
 * @param timeoutSecs 单书源超时秒数，默认 150 秒。同时传给 Rust 侧控制引擎超时。
 */
export async function runBookSourceTests(
  fileName: string,
  timeoutSecs = 150,
): Promise<TestRunResult> {
  return invokeWithTimeout<TestRunResult>(
    "booksource_run_tests",
    { fileName, timeoutSecs },
    timeoutSecs * 1000 + 5000, // JS 层多留 5s 缓冲
  );
}
