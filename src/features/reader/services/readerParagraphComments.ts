import type { ReaderParagraph } from "@/components/reader/utils/paragraphs";

export const PARAGRAPH_COMMENT_COUNTS_FN = "chapterParagraphCommentCounts";
export const PARAGRAPH_COMMENT_DETAILS_FN = "chapterParagraphComments";
export const PARAGRAPH_COMMENT_LIKE_FN = "likeParagraphComment";
export const PARAGRAPH_COMMENT_REPLY_FN = "replyParagraphComment";

export interface ParagraphCommentRange {
  key: string;
  start: number;
  end: number;
}

export interface ParagraphCommentRangeCount extends ParagraphCommentRange {
  count: number;
}

export interface ParagraphCommentSummary extends ParagraphCommentRangeCount {
  ranges: ParagraphCommentRangeCount[];
}

export interface ParagraphCommentClickPayload extends ParagraphCommentSummary {
  paragraphIndex: number;
}

export interface ParagraphCommentTarget extends ParagraphCommentClickPayload {
  fileName: string;
  sourceDir?: string;
  chapterIndex: number;
  chapterName: string;
  chapterUrl: string;
}

export interface ParagraphCommentTag {
  label: string;
  type?: string;
  color?: string;
}

export interface ParagraphCommentDetail {
  id: string;
  nickname: string;
  avatarUrl?: string;
  content: string;
  createdAt?: string;
  likeCount: number;
  liked: boolean;
  replyCount: number;
  tags: ParagraphCommentTag[];
}

export interface ParagraphCommentDetailPage {
  comments: ParagraphCommentDetail[];
  total: number;
  cursor?: string;
  hasMore: boolean;
}

export interface ParagraphCommentActionCapabilities {
  counts: boolean;
  details: boolean;
  like: boolean;
  reply: boolean;
}

export interface ParagraphCommentContext {
  chapterIndex: number;
  chapterName: string;
  chapterUrl: string;
  sourceType: string;
  paragraphCount: number;
  paragraphs: string[];
  contentHash: string;
}

export interface ParagraphCommentDetailQuery {
  cursor?: string;
  page?: number;
  pageSize?: number;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function toFiniteNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function toPositiveCount(value: unknown): number {
  if (Array.isArray(value)) {
    return value.length;
  }
  const direct = toFiniteNumber(value);
  if (direct !== null) {
    return Math.max(0, Math.floor(direct));
  }

  const record = asRecord(value);
  if (!record) {
    return 0;
  }

  for (const key of ["count", "commentCount", "commentsCount", "total", "size", "length"]) {
    const count = toFiniteNumber(record[key]);
    if (count !== null) {
      return Math.max(0, Math.floor(count));
    }
  }

  const comments = record.comments ?? record.items ?? record.list ?? record.data;
  return Array.isArray(comments) ? comments.length : 0;
}

export function parseParagraphCommentRangeKey(key: string): ParagraphCommentRange | null {
  const normalizedKey = key.trim();
  const single = /^(\d+)$/.exec(normalizedKey);
  if (single) {
    const index = Number(single[1]);
    if (!Number.isSafeInteger(index)) {
      return null;
    }
    return { key: `${index}+${index}`, start: index, end: index };
  }

  const match = /^(\d+)\+(\d+)$/.exec(normalizedKey);
  if (!match) {
    return null;
  }
  const start = Number(match[1]);
  const end = Number(match[2]);
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start > end) {
    return null;
  }
  return { key: `${start}+${end}`, start, end };
}

function normalizeRangeCount(
  key: string,
  value: unknown,
  paragraphCount: number,
): ParagraphCommentRangeCount | null {
  const parsed = parseParagraphCommentRangeKey(key);
  if (!parsed || parsed.end >= paragraphCount) {
    return null;
  }
  const count = toPositiveCount(value);
  if (count <= 0) {
    return null;
  }
  return { ...parsed, count };
}

function collectRangeCounts(raw: unknown, paragraphCount: number): ParagraphCommentRangeCount[] {
  const result: ParagraphCommentRangeCount[] = [];

  if (Array.isArray(raw)) {
    for (const item of raw) {
      const record = asRecord(item);
      if (!record) {
        continue;
      }
      const key =
        typeof record.key === "string"
          ? record.key
          : typeof record.rangeKey === "string"
            ? record.rangeKey
            : undefined;
      const startValue = toFiniteNumber(record.start ?? record.startLine ?? record.from);
      const endValue = toFiniteNumber(record.end ?? record.endLine ?? record.to);
      const singleIndex = toFiniteNumber(
        record.index ?? record.paragraphIndex ?? record.paragraph ?? record.line,
      );
      const start = startValue ?? endValue ?? singleIndex;
      const end = endValue ?? startValue ?? singleIndex;
      const resolvedKey =
        key ??
        (start !== null && end !== null ? `${Math.floor(start)}+${Math.floor(end)}` : undefined);
      if (!resolvedKey) {
        continue;
      }
      const normalized = normalizeRangeCount(resolvedKey, record.count ?? record, paragraphCount);
      if (normalized) {
        result.push(normalized);
      }
    }
    return result;
  }

  const envelope = asRecord(raw);
  const source = asRecord(envelope?.counts) ?? asRecord(envelope?.data) ?? envelope;
  if (!source) {
    return result;
  }

  for (const [key, value] of Object.entries(source)) {
    const normalized = normalizeRangeCount(key, value, paragraphCount);
    if (normalized) {
      result.push(normalized);
    }
  }

  return result;
}

export function normalizeParagraphCommentSummaries(
  raw: unknown,
  paragraphCount: number,
): ParagraphCommentSummary[] {
  if (paragraphCount <= 0) {
    return [];
  }

  const byKey = new Map<string, ParagraphCommentRangeCount>();
  for (const item of collectRangeCounts(raw, paragraphCount)) {
    const existing = byKey.get(item.key);
    byKey.set(item.key, existing ? { ...item, count: existing.count + item.count } : item);
  }

  const byEnd = new Map<number, ParagraphCommentRangeCount[]>();
  for (const item of byKey.values()) {
    const list = byEnd.get(item.end) ?? [];
    list.push(item);
    byEnd.set(item.end, list);
  }

  return [...byEnd.entries()]
    .toSorted(([a], [b]) => a - b)
    .map(([, ranges]) => {
      const sortedRanges = ranges.toSorted(
        (a, b) => b.start - a.start || a.key.localeCompare(b.key),
      );
      const exact = sortedRanges.find((item) => item.start === item.end);
      const primary = exact ?? sortedRanges[0];
      const count = sortedRanges.reduce((sum, item) => sum + item.count, 0);
      return {
        key: primary.key,
        start: Math.min(...sortedRanges.map((item) => item.start)),
        end: primary.end,
        count,
        ranges: sortedRanges,
      };
    });
}

export function createParagraphCommentSummaryMap(
  summaries: readonly ParagraphCommentSummary[],
): Map<number, ParagraphCommentSummary> {
  return new Map(summaries.map((summary) => [summary.end, summary]));
}

export function formatParagraphCommentCount(count: number): string {
  return count > 99 ? "99+" : String(Math.max(0, count));
}

export function getParagraphCommentCapabilities(
  capabilities?: Set<string> | null,
): ParagraphCommentActionCapabilities {
  return {
    counts: !!capabilities?.has(PARAGRAPH_COMMENT_COUNTS_FN),
    details: !!capabilities?.has(PARAGRAPH_COMMENT_DETAILS_FN),
    like: !!capabilities?.has(PARAGRAPH_COMMENT_LIKE_FN),
    reply: !!capabilities?.has(PARAGRAPH_COMMENT_REPLY_FN),
  };
}

export function hashReaderContent(content: string): string {
  let hash = 2166136261;
  for (let index = 0; index < content.length; index += 1) {
    hash ^= content.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

export function buildParagraphCommentContext(
  chapterIndex: number,
  chapterName: string,
  chapterUrl: string,
  sourceType: string,
  paragraphs: readonly ReaderParagraph[],
  contentHash: string,
): ParagraphCommentContext {
  return {
    chapterIndex,
    chapterName,
    chapterUrl,
    sourceType,
    paragraphCount: paragraphs.length,
    paragraphs: paragraphs.map((paragraph) => paragraph.text),
    contentHash,
  };
}

function firstString(record: Record<string, unknown>, keys: string[]): string | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return undefined;
}

function normalizeTags(raw: unknown): ParagraphCommentTag[] {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw
    .map((item) => {
      if (typeof item === "string") {
        const label = item.trim();
        return label ? { label } : null;
      }
      const record = asRecord(item);
      if (!record) {
        return null;
      }
      const label = firstString(record, ["label", "name", "title", "text"]);
      if (!label) {
        return null;
      }
      return {
        label,
        type: firstString(record, ["type", "kind"]),
        color: firstString(record, ["color"]),
      };
    })
    .filter((item): item is ParagraphCommentTag => item !== null);
}

export function normalizeParagraphCommentDetailPage(raw: unknown): ParagraphCommentDetailPage {
  const record = asRecord(raw);
  const listRaw = Array.isArray(raw)
    ? raw
    : Array.isArray(record?.comments)
      ? record.comments
      : Array.isArray(record?.items)
        ? record.items
        : Array.isArray(record?.list)
          ? record.list
          : Array.isArray(record?.data)
            ? record.data
            : [];

  const comments = listRaw
    .map((item, index): ParagraphCommentDetail | null => {
      const itemRecord = asRecord(item);
      if (!itemRecord) {
        return null;
      }
      const id = firstString(itemRecord, ["id", "commentId", "cid"]) ?? String(index);
      const nickname =
        firstString(itemRecord, [
          "nickname",
          "nickName",
          "userName",
          "username",
          "author",
          "name",
        ]) ?? "匿名书友";
      const content = firstString(itemRecord, ["content", "text", "comment", "message"]) ?? "";
      return {
        id,
        nickname,
        avatarUrl: firstString(itemRecord, ["avatarUrl", "avatar", "avatarSrc", "userAvatar"]),
        content,
        createdAt: firstString(itemRecord, ["createdAt", "time", "date", "commentTime"]),
        likeCount: toPositiveCount(itemRecord.likeCount ?? itemRecord.likes ?? itemRecord.like),
        liked: itemRecord.liked === true || itemRecord.isLiked === true,
        replyCount: toPositiveCount(itemRecord.replyCount ?? itemRecord.repliesCount),
        tags: normalizeTags(itemRecord.tags ?? itemRecord.badges),
      };
    })
    .filter((item): item is ParagraphCommentDetail => item !== null);

  const total = toFiniteNumber(record?.total ?? record?.count) ?? comments.length;
  const cursor = firstString(record ?? {}, ["cursor", "nextCursor", "next"]);
  const hasMore = record?.hasMore === true || record?.more === true || !!cursor;

  return {
    comments,
    total: Math.max(comments.length, Math.floor(total)),
    cursor,
    hasMore,
  };
}
