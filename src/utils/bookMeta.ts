import type { BookDetail, ChapterItem } from "@/stores";
import type { CoverImageInput } from "@/utils/coverImage";

function stringifySourceValue(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return value.toString();
  }
  if (typeof value === "symbol") {
    return value.description ? `Symbol(${value.description})` : "Symbol()";
  }
  if (typeof value === "function") {
    return value.name ? `[Function: ${value.name}]` : "[Function]";
  }
  if (value instanceof Date) {
    return Number.isNaN(value.getTime()) ? "" : value.toISOString();
  }
  try {
    const json = JSON.stringify(value);
    return typeof json === "string" ? json : "";
  } catch {
    return Object.prototype.toString.call(value);
  }
}

function previewSourceValue(value: unknown): string {
  return stringifySourceValue(value) || Object.prototype.toString.call(value);
}

export interface BookMetaLike {
  kind?: string;
  lastChapter?: string;
  latestChapter?: string;
  latestChapterUrl?: string;
  wordCount?: string;
  chapterCount?: number;
  updateTime?: string;
  status?: string;
}

/** 书源返回字段类型不符时的诊断信息 */
export interface BookSourceFieldError {
  field: string;
  /** 期望的类型描述 */
  expected: string;
  /** 实际类型 */
  actual: string;
  /** 原始值（用于日志/显示） */
  rawValue: unknown;
}

/** 将 unknown 强制转换为字符串，并在类型不符时记录警告 */
function coerceString(
  value: unknown,
  field: string,
  errors: BookSourceFieldError[],
): string | undefined {
  if (value === null || value === undefined) return undefined;
  if (typeof value === "string") return value;
  errors.push({
    field,
    expected: "string",
    actual: typeof value,
    rawValue: value,
  });
  return stringifySourceValue(value);
}

/** 将 unknown 强制转换为 number，并在类型不符时记录警告 */
function coerceNumber(
  value: unknown,
  field: string,
  errors: BookSourceFieldError[],
): number | undefined {
  if (value === null || value === undefined) return undefined;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const n = parseFloat(value);
    if (Number.isFinite(n)) return n;
  }
  errors.push({
    field,
    expected: "number",
    actual: typeof value,
    rawValue: value,
  });
  return undefined;
}

/**
 * bookInfo() 字段缺省时可沿用的来源（通常来自搜索结果 SearchBook）。
 * 与 Legado 语义一致：ruleBookInfo 为空或某字段未提取时，保留搜索结果已有字段，
 * 而不是把书籍判为「缺少必需字段」。
 */
export interface BookDetailFallback {
  name?: string;
  author?: string;
  coverUrl?: CoverImageInput;
  intro?: string;
  kind?: string;
  lastChapter?: string;
  latestChapter?: string;
  latestChapterUrl?: string;
  wordCount?: string;
  updateTime?: string;
  status?: string;
}

function fallbackString(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

/**
 * 校验并规范化书源 bookInfo() 的返回值。
 * - 必需字段（name）缺失或类型错误时抛出详细错误，帮助定位书源问题。
 * - 非必需字段类型错误时强制转换并记录到 fieldErrors，不中断流程。
 * - `fallback`（搜索结果）用于补全 bookInfo 未提供的字段：name 为空但搜索已知书名时
 *   沿用搜索书名（如七猫 ruleBookInfo={}），不再抛错。
 */
export function sanitizeBookDetail(
  raw: unknown,
  sourceFile: string,
  fallbackUrl: string,
  fallback?: BookDetailFallback,
): { data: BookDetail; fieldErrors: BookSourceFieldError[] } {
  if (raw === null || raw === undefined || typeof raw !== "object" || Array.isArray(raw)) {
    throw new Error(
      `bookInfo 返回了非对象数据 [${sourceFile}]: 实际类型=${Array.isArray(raw) ? "array" : typeof raw}`,
    );
  }

  const r = raw as Record<string, unknown>;
  const fieldErrors: BookSourceFieldError[] = [];

  // name：必需。bookInfo 未提供时沿用搜索结果书名（Legado 语义）。
  const fallbackName = fallbackString(fallback?.name);
  let name: string;
  if (typeof r.name === "string" && r.name.trim()) {
    name = r.name.trim();
  } else if (r.name !== null && r.name !== undefined && r.name !== "") {
    fieldErrors.push({
      field: "name",
      expected: "非空 string",
      actual: typeof r.name,
      rawValue: r.name,
    });
    name = stringifySourceValue(r.name).trim() || fallbackName || "[书名解析失败]";
  } else if (fallbackName) {
    name = fallbackName;
  } else {
    throw new Error(`bookInfo 缺少必需字段 name [${sourceFile}]: 书籍 URL=${fallbackUrl}`);
  }

  // author：建议字段，bookInfo 未提供时沿用搜索结果，仍为空则空字符串
  const author =
    coerceString(r.author, "author", fieldErrors) ?? fallbackString(fallback?.author) ?? "";

  // tocUrl：bookInfo 中必需，缺失时使用 fallbackUrl 并记录警告
  let tocUrl: string | undefined;
  if (typeof r.tocUrl === "string" && r.tocUrl.trim()) {
    tocUrl = r.tocUrl.trim();
  } else if (r.tocUrl !== null && r.tocUrl !== undefined) {
    fieldErrors.push({
      field: "tocUrl",
      expected: "string URL",
      actual: typeof r.tocUrl,
      rawValue: r.tocUrl,
    });
    tocUrl = fallbackUrl;
  }
  // undefined 时调用方将使用 fallbackUrl

  // 可选字符串字段：bookInfo 未提供时沿用搜索结果（Legado 语义）。
  const intro = coerceString(r.intro, "intro", fieldErrors) ?? fallbackString(fallback?.intro);
  const kind = coerceString(r.kind, "kind", fieldErrors) ?? fallbackString(fallback?.kind);
  const lastChapter =
    coerceString(r.lastChapter, "lastChapter", fieldErrors) ??
    fallbackString(fallback?.lastChapter);
  const latestChapter =
    coerceString(r.latestChapter, "latestChapter", fieldErrors) ??
    fallbackString(fallback?.latestChapter);
  const latestChapterUrl =
    coerceString(r.latestChapterUrl, "latestChapterUrl", fieldErrors) ??
    fallbackString(fallback?.latestChapterUrl);
  const wordCount =
    coerceString(r.wordCount, "wordCount", fieldErrors) ?? fallbackString(fallback?.wordCount);
  const updateTime =
    coerceString(r.updateTime, "updateTime", fieldErrors) ?? fallbackString(fallback?.updateTime);
  const status = coerceString(r.status, "status", fieldErrors) ?? fallbackString(fallback?.status);

  // coverUrl：透传（可能是 string 或 CoverImageInput 对象）；bookInfo 未提供时沿用搜索结果封面
  const coverUrl = (r.coverUrl as CoverImageInput | undefined) ?? fallback?.coverUrl;

  // chapterCount：可选数字
  const chapterCount = coerceNumber(r.chapterCount, "chapterCount", fieldErrors);

  const data: BookDetail = { name, author };
  if (coverUrl !== undefined) data.coverUrl = coverUrl;
  if (intro !== undefined) data.intro = intro;
  if (kind !== undefined) data.kind = kind;
  if (lastChapter !== undefined) data.lastChapter = lastChapter;
  if (latestChapter !== undefined) data.latestChapter = latestChapter;
  if (latestChapterUrl !== undefined) data.latestChapterUrl = latestChapterUrl;
  if (wordCount !== undefined) data.wordCount = wordCount;
  if (updateTime !== undefined) data.updateTime = updateTime;
  if (status !== undefined) data.status = status;
  if (tocUrl !== undefined) data.tocUrl = tocUrl;
  if (chapterCount !== undefined) data.chapterCount = chapterCount;

  if (fieldErrors.length > 0) {
    console.warn(
      `[BookSource] bookInfo 字段类型异常 [${sourceFile}]:`,
      fieldErrors
        .map(
          (e) =>
            `${e.field}(期望 ${e.expected}, 实际 ${e.actual}=${previewSourceValue(e.rawValue).slice(0, 80)})`,
        )
        .join("; "),
    );
  }

  return { data, fieldErrors };
}

/**
 * 校验并规范化书源 chapterList() 的返回值。
 * - 非数组时抛出，帮助定位书源问题。
 * - 条目字段类型错误时强制转换或跳过，不中断流程。
 */
export function sanitizeChapterList(
  raw: unknown,
  sourceFile: string,
): { data: ChapterItem[]; skipped: number; warnings: string[] } {
  if (!Array.isArray(raw)) {
    throw new Error(`chapterList 返回了非数组数据 [${sourceFile}]: 实际类型=${typeof raw}`);
  }

  const data: ChapterItem[] = [];
  const warnings: string[] = [];
  let skipped = 0;

  for (let i = 0; i < raw.length; i++) {
    const item = raw[i];
    if (item === null || item === undefined || typeof item !== "object" || Array.isArray(item)) {
      warnings.push(`第 ${i + 1} 条目非对象 (${Array.isArray(item) ? "array" : typeof item})`);
      skipped++;
      continue;
    }

    const r = item as Record<string, unknown>;

    // url：缺失则跳过该条目（无法加载章节内容）
    let url: string;
    if (typeof r.url === "string" && r.url) {
      url = r.url;
    } else if (r.url !== null && r.url !== undefined) {
      const coerced = stringifySourceValue(r.url).trim();
      if (!coerced) {
        warnings.push(`第 ${i + 1} 条 url 为空（原始类型 ${typeof r.url}），已跳过`);
        skipped++;
        continue;
      }
      warnings.push(`第 ${i + 1} 条 url 类型异常（期望 string, 实际 ${typeof r.url}），已强制转换`);
      url = coerced;
    } else {
      warnings.push(`第 ${i + 1} 条缺少 url，已跳过`);
      skipped++;
      continue;
    }

    // name：缺失或类型错误时用占位符
    let name: string;
    if (typeof r.name === "string") {
      name = r.name;
    } else if (r.name !== null && r.name !== undefined) {
      warnings.push(
        `第 ${i + 1} 条 name 类型异常（期望 string, 实际 ${typeof r.name}），已强制转换`,
      );
      name = stringifySourceValue(r.name);
    } else {
      name = `第 ${i + 1} 章`;
    }

    const ch: ChapterItem = { name, url };
    if (r.group !== null && r.group !== undefined) {
      ch.group = typeof r.group === "string" ? r.group : stringifySourceValue(r.group);
    }
    if (r.vip !== undefined) ch.vip = Boolean(r.vip);
    if (r.isVip !== undefined) ch.isVip = Boolean(r.isVip);
    if (r.price !== undefined) ch.price = r.price;
    if (r.currency !== null && r.currency !== undefined) {
      ch.currency = typeof r.currency === "string" ? r.currency : stringifySourceValue(r.currency);
    }

    data.push(ch);
  }

  if (warnings.length > 0) {
    console.warn(`[BookSource] chapterList 数据异常 [${sourceFile}]:`, warnings.join("; "));
  }

  return { data, skipped, warnings };
}

export interface BookMetaBadge {
  key: string;
  label: string;
  tone: "source" | "kind" | "status";
}

const TYPE_LABELS: Record<string, string> = {
  novel: "小说",
  comic: "漫画",
  video: "视频",
  music: "音乐",
  webpage: "网页",
};

function cleanText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function getLatestChapterText(book?: BookMetaLike | null): string {
  if (!book) {
    return "";
  }
  return cleanText(book.latestChapter) || cleanText(book.lastChapter);
}

export function getLatestChapterUrl(book?: BookMetaLike | null): string {
  return book ? cleanText(book.latestChapterUrl) : "";
}

export function getSourceTypeLabel(sourceType?: string | null): string {
  return TYPE_LABELS[cleanText(sourceType) || "novel"] ?? "";
}

export function getChapterCountText(book?: BookMetaLike | null): string {
  if (!book || typeof book.chapterCount !== "number" || !Number.isFinite(book.chapterCount)) {
    return "";
  }
  const count = Math.max(0, Math.floor(book.chapterCount));
  return count > 0 ? `共 ${count} 章` : "";
}

export function getBookMetaLine(book?: BookMetaLike | null): string[] {
  if (!book) {
    return [];
  }
  return [cleanText(book.wordCount), getChapterCountText(book), cleanText(book.updateTime)].filter(
    Boolean,
  );
}

export function getBookMetaBadges(book?: BookMetaLike | null, sourceType = ""): BookMetaBadge[] {
  const badges: BookMetaBadge[] = [];
  const sourceTypeKey = cleanText(sourceType);
  const typeLabel = sourceTypeKey ? getSourceTypeLabel(sourceTypeKey) : "";
  if (typeLabel) {
    badges.push({
      key: `source:${sourceType}`,
      label: typeLabel,
      tone: "source",
    });
  }
  const kind = cleanText(book?.kind);
  if (kind) {
    badges.push({ key: `kind:${kind}`, label: kind, tone: "kind" });
  }
  const status = cleanText(book?.status);
  if (status) {
    badges.push({ key: `status:${status}`, label: status, tone: "status" });
  }
  return badges;
}

export function getNormalizedLastChapter(book?: BookMetaLike | null): string | undefined {
  return getLatestChapterText(book) || undefined;
}
