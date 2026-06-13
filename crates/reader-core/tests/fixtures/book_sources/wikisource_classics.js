// oxlint-disable no-unused-vars -- ReaderCore invokes JS source entry points by name.
// @name 中文维基文库经典小说
// @url https://zh.wikisource.org
// @homepage https://zh.wikisource.org/wiki/Wikisource:%E9%A6%96%E9%A1%B5
// @author Codex
// @description Public-domain classic Chinese novels from Chinese Wikisource. Text is fetched from Wikisource pages and should not be used to bypass paid, login-only, or preview-only content.
// @tags public-domain,wikisource,classic
// @version 2026.06.13
// @enabled true
// @minDelayMs 800

const BASE_URL = "https://zh.wikisource.org";
const REQUEST_HEADERS = {
  "User-Agent":
    "Legado-Tauri/2026.06.13 (https://github.com/FanhuaAwA/legado; reader-core source compatibility test)",
  Accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
};

const CATALOG = [
  {
    name: "三國演義",
    aliases: ["三国演义", "三國演義", "三国", "三國"],
    author: "羅貫中",
    title: "三國演義",
    intro: "明代章回小說，维基文库标注为公有领域作品。本书源仅抓取公开页面正文。",
    kind: "公有领域,古典小说,历史演义",
    status: "完本",
    chapterCount: 120,
  },
];

function wikiPageUrl(title) {
  return `${BASE_URL}/wiki/${encodeURIComponent(title).replace(/%2F/g, "/")}`;
}

async function fetchWiki(url) {
  return legado.http.get(url, REQUEST_HEADERS);
}

function normalizeText(input) {
  return String(input || "")
    .toLowerCase()
    .replace(/[國国]/g, "国")
    .replace(/[羅罗]/g, "罗")
    .replace(/[義义]/g, "义")
    .replace(/[學学]/g, "学")
    .replace(/[^\u3400-\u9fffa-z0-9]+/g, "");
}

function decodeHtml(input) {
  return String(input || "")
    .replace(/&nbsp;/g, " ")
    .replace(/&#160;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#039;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">");
}

function stripTags(input) {
  return decodeHtml(
    String(input || "")
      .replace(/<script[\s\S]*?<\/script>/gi, "")
      .replace(/<style[\s\S]*?<\/style>/gi, "")
      .replace(/<sup[\s\S]*?<\/sup>/gi, "")
      .replace(/<[^>]+>/g, ""),
  )
    .replace(/\[[^\]]{1,8}\]/g, "")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function htmlAttr(attrs, name) {
  const re = new RegExp(`${name}=["']([^"']+)["']`, "i");
  const match = re.exec(attrs || "");
  return match ? decodeHtml(match[1]) : "";
}

function absoluteUrl(href) {
  const value = decodeHtml(href || "");
  if (/^https?:\/\//i.test(value)) return value;
  if (value.startsWith("//")) return `https:${value}`;
  if (value.startsWith("/")) return `${BASE_URL}${value}`;
  return `${BASE_URL}/${value}`;
}

function catalogItemToBook(item) {
  const bookUrl = wikiPageUrl(item.title);
  return {
    name: item.name,
    author: item.author,
    bookUrl,
    intro: item.intro,
    kind: item.kind,
    status: item.status,
    chapterCount: item.chapterCount,
    latestChapter: "第一百二十回",
    latestChapterUrl: `${bookUrl}/${encodeURIComponent("第120回")}`,
  };
}

function catalogByUrl(bookUrl) {
  const normalizedUrl = decodeURIComponent(String(bookUrl || ""));
  return CATALOG.find((item) => normalizedUrl.includes(`/wiki/${item.title}`)) || CATALOG[0];
}

function isChapterLink(item, href, title, linkText) {
  if (!href) return false;
  const decodedHref = decodeURIComponent(href);
  const decodedTitle = decodeHtml(title);
  const text = stripTags(linkText);
  const hasNumberedTitle = new RegExp(`^${item.title}/第\\d{3}回$`).test(decodedTitle);
  const hasNumberedHref = decodedHref.includes(`/wiki/${item.title}/第`);
  return /^第.+回$/.test(text) && (hasNumberedTitle || hasNumberedHref);
}

async function search(keyword, page) {
  if (page && Number(page) > 1) return [];
  const key = normalizeText(keyword);
  return CATALOG.filter((item) => {
    if (!key) return true;
    return [item.name, item.author].concat(item.aliases).some((value) => {
      const haystack = normalizeText(value);
      return haystack.includes(key) || key.includes(haystack);
    });
  }).map(catalogItemToBook);
}

async function bookInfo(bookUrl) {
  const item = catalogByUrl(bookUrl);
  const html = await fetchWiki(wikiPageUrl(item.title));
  const introMatch = /<p>([\s\S]*?公有领域[\s\S]*?)<\/p>/i.exec(html);
  const intro = introMatch ? stripTags(introMatch[1]) : item.intro;
  const book = catalogItemToBook(item);
  return {
    ...book,
    bookUrl: wikiPageUrl(item.title),
    tocUrl: wikiPageUrl(item.title),
    intro: intro || item.intro,
  };
}

async function chapterList(tocUrl) {
  const item = catalogByUrl(tocUrl);
  const html = await fetchWiki(wikiPageUrl(item.title));
  const chapters = [];
  const seen = {};
  const itemRe = /<li[^>]*>\s*<a\b([^>]*)>([\s\S]*?)<\/a>\s*([\s\S]*?)<\/li>/gi;
  let match;

  while ((match = itemRe.exec(html))) {
    const attrs = match[1];
    const linkText = match[2];
    const tail = stripTags(match[3]).replace(/\s+/g, " ").trim();
    const href = htmlAttr(attrs, "href");
    const title = htmlAttr(attrs, "title");

    if (!isChapterLink(item, href, title, linkText)) continue;

    const url = absoluteUrl(href);
    if (seen[url]) continue;
    seen[url] = true;

    const prefix = stripTags(linkText);
    chapters.push({
      name: tail ? `${prefix} ${tail}` : prefix,
      url,
      group: item.name,
      vip: false,
      isVip: false,
    });
  }

  return chapters;
}

async function chapterContent(chapterUrl) {
  const html = await fetchWiki(chapterUrl);
  const contentMatch =
    /<div id="mw-content-text"[^>]*>\s*<div[^>]*class="[^"]*mw-parser-output[^"]*"[^>]*>([\s\S]*?)<noscript>/i.exec(
      html,
    ) || /<div class="mw-content-ltr mw-parser-output"[^>]*>([\s\S]*?)<noscript>/i.exec(html);

  if (!contentMatch) return "";

  let body = contentMatch[1]
    .replace(/<table[\s\S]*?<\/table>/gi, "\n")
    .replace(/<div[^>]*class="[^"]*printfooter[^"]*"[\s\S]*?<\/div>/gi, "")
    .replace(/<span[^>]*class="[^"]*mw-editsection[^"]*"[\s\S]*?<\/span>/gi, "")
    .replace(/<\/p>\s*<p[^>]*>/gi, "\n\n")
    .replace(/<br\s*\/?>/gi, "\n");

  body = stripTags(body)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n\n");

  return body;
}
