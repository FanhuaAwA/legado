export interface ReaderParagraph {
  text: string;
  index: number;
  charOffset: number;
}

export interface SplitReaderParagraphsOptions {
  preserveInlineHtml?: boolean;
}

interface LegacyImageOptions {
  style?: string;
  type?: string;
  js?: string;
  click?: string;
  [key: string]: unknown;
}

interface LegacyImageSource {
  src: string;
  options: LegacyImageOptions;
}

const LEGACY_INLINE_RE = /<(?:img|span)\b[^>]*\breader-legacy-[^>]*>(?:<\/span>)?/giu;
const INLINE_IMAGE_RE = /<img\b[^>]*>/giu;
const INLINE_PLACEHOLDER_PREFIX = "\uE000LEGADO_INLINE_";
const INLINE_PLACEHOLDER_SUFFIX = "\uE001";

export function splitReaderParagraphs(
  content: string,
  options: SplitReaderParagraphsOptions = {},
): ReaderParagraph[] {
  const normalized = normalizeReaderContent(content, !!options.preserveInlineHtml);
  const paragraphs: ReaderParagraph[] = [];
  const parts = normalized.split(/\r?\n+/u);
  let searchFrom = 0;

  for (const part of parts) {
    const text = part.trim();
    if (!isMeaningfulParagraph(text, !!options.preserveInlineHtml)) {
      searchFrom += part.length + 1;
      continue;
    }

    const pos = normalized.indexOf(text, searchFrom);
    const charOffset = pos >= 0 ? pos : searchFrom;
    paragraphs.push({
      text,
      index: paragraphs.length,
      charOffset,
    });
    searchFrom = charOffset + text.length;
  }

  return paragraphs;
}

function normalizeReaderContent(content: string, preserveInlineHtml: boolean): string {
  let text = String(content ?? "").replace(/\r\n?/gu, "\n");
  if (!/<\/?p\b|<br\b|<img\b|<comment\b/iu.test(text)) {
    return text;
  }

  text = text.replace(/<comment[\s\S]*?\/>/giu, "");
  text = normalizeInlineImages(text, preserveInlineHtml);
  text = text.replace(/<br\s*\/?>/giu, "\n");

  if (/<\/p\s*>/iu.test(text)) {
    return text
      .split(/<\/p\s*>/iu)
      .map((part) => cleanParagraphFragment(part, preserveInlineHtml))
      .filter((part) => isMeaningfulParagraph(part, preserveInlineHtml))
      .join("\n");
  }

  return text
    .replace(/<p\b[^>]*>/giu, "\n")
    .split(/\n+/u)
    .map((part) => cleanParagraphFragment(part, preserveInlineHtml))
    .filter((part) => isMeaningfulParagraph(part, preserveInlineHtml))
    .join("\n");
}

function cleanParagraphFragment(fragment: string, preserveInlineHtml: boolean): string {
  const withoutParagraphTags = fragment
    .replace(/^\s*<p\b[^>]*>/iu, "")
    .replace(/<p\b[^>]*>/giu, "\n");
  return preserveInlineHtml
    ? sanitizeParagraphHtml(withoutParagraphTags)
    : htmlFragmentToPlainText(withoutParagraphTags);
}

function normalizeInlineImages(fragment: string, preserveInlineHtml: boolean): string {
  return fragment.replace(INLINE_IMAGE_RE, (tag) => {
    const rawSrc = extractImageSrc(tag);
    if (!rawSrc) {
      return "";
    }

    const legacy = parseLegacyImageSource(rawSrc);
    if (!legacy) {
      if (!preserveInlineHtml || !isAllowedImageSource(rawSrc)) {
        return "";
      }
      return `<img class="reader-content-inline-image" src="${escapeAttr(rawSrc)}" alt="">`;
    }

    if (!preserveInlineHtml) {
      return "";
    }

    if (isAllowedImageSource(legacy.src)) {
      const label = legacyActionLabel(legacy.options);
      return [
        `<img class="reader-legacy-inline-image" src="${escapeAttr(legacy.src)}"`,
        `alt="${escapeAttr(label)}" title="${escapeAttr(label)}"`,
        legacyDataAttrs(legacy.options),
        ">",
      ]
        .filter(Boolean)
        .join(" ");
    }

    const label = legacyActionLabel(legacy.options);
    return [
      `<span class="reader-legacy-comment-action" role="button" tabindex="0"`,
      `title="${escapeAttr(label)}"`,
      legacyDataAttrs(legacy.options),
      `>${escapeHtml(label)}</span>`,
    ]
      .filter(Boolean)
      .join(" ");
  });
}

function extractImageSrc(tag: string): string {
  const simple = /\bsrc\s*=\s*(["'])([^"']*)\1/iu.exec(tag);
  if (simple?.[2] && !simple[2].includes(",{")) {
    return decodeHtmlEntities(simple[2].trim());
  }

  const start = /\bsrc\s*=\s*(["'])/iu.exec(tag);
  if (start) {
    const quote = start[1];
    const valueStart = start.index + start[0].length;
    const valueEnd = tag.lastIndexOf(quote);
    if (valueEnd > valueStart) {
      return decodeHtmlEntities(tag.slice(valueStart, valueEnd).trim());
    }
  }

  const unquoted = /\bsrc\s*=\s*([^\s>]+)/iu.exec(tag);
  return decodeHtmlEntities(unquoted?.[1]?.trim() ?? "");
}

function parseLegacyImageSource(src: string): LegacyImageSource | null {
  const optionStart = src.lastIndexOf(",{");
  if (optionStart < 0) {
    return null;
  }

  const optionsText = src.slice(optionStart + 1).trim();
  if (!optionsText.endsWith("}")) {
    return null;
  }

  try {
    const options = JSON.parse(optionsText) as LegacyImageOptions;
    return {
      src: src.slice(0, optionStart).trim(),
      options,
    };
  } catch {
    return null;
  }
}

function legacyDataAttrs(options: LegacyImageOptions): string {
  const attrs: string[] = [];
  if (typeof options.type === "string" && options.type) {
    attrs.push(`data-legado-type="${escapeAttr(options.type)}"`);
  }
  if (typeof options.style === "string" && options.style) {
    attrs.push(`data-legado-style="${escapeAttr(options.style)}"`);
  }
  if (typeof options.js === "string" && options.js) {
    attrs.push(`data-legado-js="${escapeAttr(options.js)}"`);
  }
  if (typeof options.click === "string" && options.click) {
    attrs.push(`data-legado-click="${escapeAttr(options.click)}"`);
  }
  return attrs.join(" ");
}

function legacyActionLabel(options: LegacyImageOptions): string {
  const payload = `${typeof options.js === "string" ? options.js : ""} ${
    typeof options.click === "string" ? options.click : ""
  }`;
  const count = /(?:getDP|getSP|getZP)\([^,]+,\s*(\d+)/iu.exec(payload)?.[1];
  const suffix = count ? ` ${count}` : "";
  if (/getSP/iu.test(payload)) {
    return `神评${suffix}`;
  }
  if (/getZP/iu.test(payload)) {
    return `章评${suffix}`;
  }
  if (options.type === "qm") {
    return `段评${suffix}`;
  }
  return `段评${suffix}`;
}

function sanitizeParagraphHtml(fragment: string): string {
  const controls: string[] = [];
  const masked = fragment.replace(LEGACY_INLINE_RE, (tag) => {
    const index = controls.push(tag) - 1;
    return `${INLINE_PLACEHOLDER_PREFIX}${index}${INLINE_PLACEHOLDER_SUFFIX}`;
  });
  const text = decodeHtmlEntities(masked.replace(/<[^>]*>/gu, "")).replace(/\u00a0/gu, " ");
  return text
    .replace(
      new RegExp(`${INLINE_PLACEHOLDER_PREFIX}(\\d+)${INLINE_PLACEHOLDER_SUFFIX}`, "gu"),
      (_, index: string) => controls[Number(index)] ?? "",
    )
    .trim();
}

function htmlFragmentToPlainText(fragment: string): string {
  const withoutControls = fragment.replace(LEGACY_INLINE_RE, "");
  return decodeHtmlEntities(withoutControls.replace(/<[^>]*>/gu, ""))
    .replace(/\u00a0/gu, " ")
    .trim();
}

function isMeaningfulParagraph(text: string, preserveInlineHtml: boolean): boolean {
  if (htmlFragmentToPlainText(text).trim()) {
    return true;
  }
  LEGACY_INLINE_RE.lastIndex = 0;
  return preserveInlineHtml && LEGACY_INLINE_RE.test(text);
}

function isAllowedImageSource(src: string): boolean {
  return /^(?:https?:\/\/|data:image\/)/iu.test(src) && !/^https?:\/\/$/iu.test(src);
}

function decodeHtmlEntities(value: string): string {
  if (typeof document !== "undefined") {
    const textarea = document.createElement("textarea");
    textarea.innerHTML = value;
    return textarea.value;
  }
  return value
    .replace(/&nbsp;/giu, " ")
    .replace(/&quot;/giu, '"')
    .replace(/&#39;/giu, "'")
    .replace(/&lt;/giu, "<")
    .replace(/&gt;/giu, ">")
    .replace(/&amp;/giu, "&");
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/gu, "&amp;")
    .replace(/</gu, "&lt;")
    .replace(/>/gu, "&gt;")
    .replace(/"/gu, "&quot;");
}

function escapeAttr(value: string): string {
  return escapeHtml(value).replace(/'/gu, "&#39;");
}
