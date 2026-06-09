/**
 * useAiAgent — AI 自动写书源 Agent
 *
 * 架构：
 *   前端 → Vercel AI SDK streamText → 可切换的 HTTP 通道（浏览器直连 / Rust 后端代理）→ 多步工具调用循环
 *   工具调用 → 现有 Tauri 命令（listBookSources / readBookSource / saveBookSource /
 *              evalBookSource / booksource_search / booksource_book_info 等）
 *
 * 配置存储：Rust 暴露的前端命名空间存储
 * 支持任意 OpenAI 兼容端点（OpenAI / 智谱 / DeepSeek / Ollama / 本地 LLM 等）
 *
 * 注意：AI SDK v6 使用 inputSchema（而非 parameters），且流式部分属性已更名：
 *   text-delta  → part.text      (非 textDelta)
 *   tool-call   → part.input     (非 args)
 *   tool-result → part.output    (非 result)
 *   start-step  → 替代 onStepStart 回调
 *
 * 会话集成：与 useAiSessions.ts 配合实现持久化多轮对话：
 *   - runAiAgent 接受 options.sessionId，运行结束后自动持久化会话状态
 *   - options.continueConversation=true 时，从会话中加载历史消息，实现多轮对话
 *   - 每次 save_source 工具调用后自动触发 addDraftSnapshot 创建版本快照
 */

import { createOpenAI } from "@ai-sdk/openai";
import { streamText, tool, stepCountIs, type ModelMessage } from "ai";
import { reactive } from "vue";
import { z } from "zod";
import { useAiSessionsStore } from "@/stores";
import {
  listBookSources,
  readBookSource,
  evalBookSource,
  type BookSourceMeta,
} from "./useBookSource";
import {
  ensureFrontendNamespaceLoaded,
  getFrontendStorageItem,
  legacyLocalStorageGet,
  legacyLocalStorageRemove,
  setFrontendStorageItem,
} from "./useFrontendStorage";
import { invokeWithTimeout } from "./useInvoke";

// ── 配置类型 ──────────────────────────────────────────────────────────────

export interface AiConfig {
  /** API 基地址，例如 https://api.openai.com/v1 */
  apiUrl: string;
  /** API 密钥 */
  apiKey: string;
  /** 模型名称，例如 gpt-4o / deepseek-chat */
  model: string;
  /** 最大工具调用轮次（stepCountIs 参数），防止死循环，默认 30 */
  maxSteps: number;
  /**
   * 请求模式:
   * - `chat`    → /v1/chat/completions（兼容大多数第三方接口：MiniMax、DeepSeek、Ollama 等）
   * - `responses` → /v1/responses（OpenAI 原生 Responses API，仅适用于官方 OpenAI）
   * 默认 chat
   */
  apiMode: "chat" | "responses";
  /**
   * 模型 HTTP 请求通道:
   * - `frontend` → 浏览器 fetch 直连，支持流式响应与实时思考显示，但受 CORS / 系统代理影响
   * - `backend`  → Rust 本地流式代理，复用应用 HTTP 配置，并支持实时流式显示
   */
  requestTransport: "frontend" | "backend";
  /** 采样温度，可选，留空表示使用模型默认値 */
  temperature?: number;
  /** 单次调用最大输出 Token，可选，留空表示不限制 */
  maxTokens?: number;
}

const CONFIG_NAMESPACE = "ai.agent";
const CONFIG_STORAGE_KEY = "config";

function defaultAiConfig(): AiConfig {
  return {
    apiUrl: "https://api.openai.com/v1",
    apiKey: "",
    model: "gpt-4o",
    maxSteps: 30,
    apiMode: "chat",
    requestTransport: "backend",
  };
}

function parseAiConfig(raw: string | null): AiConfig {
  if (!raw) {
    return defaultAiConfig();
  }
  try {
    const parsed = JSON.parse(raw) as Partial<AiConfig>;
    return {
      apiUrl: parsed.apiUrl ?? "https://api.openai.com/v1",
      apiKey: parsed.apiKey ?? "",
      model: parsed.model ?? "gpt-4o",
      maxSteps: parsed.maxSteps ?? 30,
      apiMode: parsed.apiMode ?? "chat",
      requestTransport: parsed.requestTransport ?? "backend",
      temperature: parsed.temperature,
      maxTokens: parsed.maxTokens,
    };
  } catch {
    return defaultAiConfig();
  }
}

export function loadAiConfig(): AiConfig {
  return parseAiConfig(getFrontendStorageItem(CONFIG_NAMESPACE, CONFIG_STORAGE_KEY));
}

export function saveAiConfig(cfg: AiConfig): void {
  setFrontendStorageItem(CONFIG_NAMESPACE, CONFIG_STORAGE_KEY, JSON.stringify(cfg));
}

export async function ensureAiConfigLoaded(): Promise<AiConfig> {
  await ensureFrontendNamespaceLoaded(CONFIG_NAMESPACE, () => {
    const legacy = legacyLocalStorageGet("legado_ai_agent_config");
    if (!legacy) {
      return null;
    }
    legacyLocalStorageRemove("legado_ai_agent_config");
    return { [CONFIG_STORAGE_KEY]: legacy };
  });
  return loadAiConfig();
}

// ── 活动日志类型 ───────────────────────────────────────────────────────────

export type ActivityType =
  | "thinking" // AI 正在推理（流式文本）
  | "tool_call" // AI 发起工具调用（含参数，完成后显示返回值）
  | "message" // AI 最终回复
  | "error" // 错误
  | "info"; // 系统信息

export interface AgentActivity {
  id: number;
  type: ActivityType;
  /** 主要文字内容（思考文本 / 错误信息 / 系统通知） */
  content: string;
  /** 工具名称（tool_call 类型时有效） */
  toolName?: string;
  /** 工具调用参数（JSON 字符串，tool_call 时有效） */
  args?: string;
  /** 工具执行结果（JSON/文本，tool_call 完成后填充） */
  result?: string;
  timestamp: number;
}

// ── 测试结果类型 ──────────────────────────────────────────────────────────

export type TestStatus = "pending" | "ok" | "error";

export interface TestResult {
  name: string;
  status: TestStatus;
  output: string;
}

// ── 系统提示词 ────────────────────────────────────────────────────────────

function buildSystemPrompt(): string {
  return [
    "你是 **Legado Tauri 书源制作助手**，负责把用户给出的目标网站、Android JSON 旧书源、HTML/API 响应、抓包线索、报错日志或半成品，做成可保存、可测试、可维护的 Legado Tauri 书源 JS。",
    "",
    "## 当前内置技能",
    "",
    "你可以调用这些工具完成真实闭环，而不是只给建议：",
    "",
    "- list_sources：列出已安装书源，用于参考现有实现。",
    "- read_source：读取已有书源完整代码，用于修改或借鉴同站型写法。",
    "- save_source：把完整书源 JS 保存到 AI 草稿目录；每次改代码后必须调用。",
    "- eval_in_source：在书源运行环境中执行探测代码，可用 legado.http、legado.dom、fetch、hash、crypto、browser API。",
    "- test_explore：测试 explore(page, category)，也可用空分类或 GETALL 获取分类。",
    "- test_search：测试 search(keyword, page)。",
    "- test_book_info：测试 bookInfo(bookUrl)。",
    "- test_chapter_list：测试 chapterList(tocUrl)。",
    "- test_chapter_content：测试 chapterContent(chapterUrl)。",
    "- test_purchase_chapter：仅在用户明确需要 VIP/付费购买时测试 purchaseChapter。",
    "- run_builtin_tests：运行书源内置 TEST('__list__') / TEST(type) 全量测试；只有书源实现 TEST 后使用。",
    "",
    "## 实体词和意图词",
    "",
    "用户可能不会精确说出函数名。遇到下列实体词、别名和场景词，都按书源制作任务理解，并主动映射到正确模块：",
    "",
    "- 书源实体：书源、阅读源、源、站点源、Legado 源、Tauri 书源、Android JSON 书源、旧书源、规则源、书源 JS、草稿、正式书源。",
    "- 内容类型：小说、网文、短篇、漫画、图片漫画、动漫、视频、影视、剧集、音乐、音频、听书、有声书。",
    "- 核心模块：搜索、search、发现、explore、分类、详情、bookInfo、目录、章节列表、chapterList、正文、内容、chapterContent、购买章节、purchaseChapter、图片处理、processImage、测试、TEST。",
    "- 数据字段：name、author、bookUrl、tocUrl、coverUrl、intro、kind、lastChapter、latestChapter、latestChapterUrl、wordCount、chapterCount、updateTime、status、group、vip、price、currency。",
    "- 页面结构：首页、分类页、搜索页、详情页、目录页、阅读页、播放页、下一页、分页、倒序、分卷、线路、多线路。",
    "- 网络/API：GET、POST、JSON、API、接口、GraphQL、headers、User-Agent、Referer、Origin、X-Requested-With、Cookie、token、session、登录态、会员态。",
    "- 编码/加密：GBK、GB2312、GB18030、Big5、Shift_JIS、Base64、MD5、SHA1、SHA256、HMAC、AES、DES、CBC、ECB、IV、sign、nonce、timestamp、Wasm、混淆、解密、签名。",
    "- 反爬/动态：Cloudflare、CF 盾、Turnstile、WAF、验证码、安全验证、Just a moment、Checking your browser、动态渲染、SPA、浏览器探测、嗅探、onRequest、m3u8、mp4、mp3、WebView。",
    "- 漫画资源：图片 URL、懒加载、data-src、data-original、防盗链、Referer、切片、条带打乱、scramble、processImage、二维码登录。",
    "- 调试验收：空数组、正文为空、图片 403、目录为空、乱码、顺序反了、CLI、booksource-test、booksource-eval、all 测试。",
    "",
    "## 运行环境约束",
    "",
    "书源 JS 运行在 Boa JS 引擎中，不是浏览器页面脚本。写法要保守，但可以使用 async function 和 await 调用宿主异步 API。",
    "",
    "必须遵守：",
    "",
    "- 优先使用 var、普通 function、async/await、字符串拼接；避免箭头函数、class、复杂解构、展开运算符、过新的语法糖。",
    "- 普通书源函数中不要直接使用 document、window、XMLHttpRequest；只有 legado.browser.eval/run 里的页面脚本可以使用浏览器 DOM。",
    "- 网络请求优先用 legado.http.*；fetch 可用但不要替代必须携带宿主能力的场景。",
    "- HTML 解析用 legado.dom.*，解析后长流程里尽量 try/finally 调 legado.dom.free(doc)。",
    "- 哈希、加密、charset 编码等如果返回 Promise，必须 await。",
    "- 不要硬编码用户私密 Cookie、token、账号、密码。",
    "",
    "## 书源文件骨架",
    "",
    "```js",
    "// @name        书源名称",
    "// @uuid        source-unique-id",
    "// @version     1.0.0",
    "// @author      AI",
    "// @url         https://主站域名",
    "// @type        novel          // novel | comic | video | music",
    "// @enabled     true",
    "// @tags        标签1,标签2",
    "// @description 简短描述",
    "",
    "var BASE = 'https://主站域名';",
    "",
    "async function search(keyword, page) {}",
    "async function explore(page, category) {}",
    "async function bookInfo(bookUrl) {}",
    "async function chapterList(tocUrl) {}",
    "async function purchaseChapter(chapterUrl, chapter) {} // 可选",
    "async function chapterContent(chapterUrl) {}",
    "function processImage(base64Data, pageIndex, imageUrl) {} // 漫画可选",
    "async function TEST(type) {} // 推荐复杂书源实现",
    "```",
    "",
    "## 可用宿主 API 速查",
    "",
    "HTTP：await legado.http.get(url, headers)、post(url, body, headers)、postBinary(url, base64Body, headers)、batchGet(urls, headers, concurrency)、request(options)，以及 fetch / Headers / Request / Response / URLSearchParams / FormData。",
    "DOM：legado.dom.parse/free/select/selectAll/text/html/attr/selectText/selectAttr/selectAllTexts/selectAllAttrs/selectByText/remove。",
    "编码：btoa/atob、encodeURIComponent/decodeURIComponent、await legado.urlEncodeCharset(str, charset)、await legado.htmlEncode/htmlDecode、legado.base64ByteSlice。",
    "哈希加密：await legado.md5/sha1/sha256/hmacSha256、aesEncrypt/aesDecrypt/aesDecryptB64Iv、desEncrypt/desDecrypt。",
    "浏览器探测：legado.browser.acquire/create/navigate/eval/run/html/text/url/cookies/getCookie/setCookie/show/hide/close/mute/unmute/onRequest/offRequest；也可用 legado.browser2.acquire 对象风格。",
    "图片处理：legado.image.decode/create/width/height/crop/paste/copyRegion/encode/free/qrCodeDataUrl。",
    "配置和日志：legado.log、toast、config.read/write/readBytes/writeBytes、ui.emit、runtime.getMachineUid/getMachineUUID/has、wasm.load/loadBase64/invoke/invokeJson。",
    "",
    "## 数据结构契约",
    "",
    "BookItem: { name, author, bookUrl, tocUrl, coverUrl, intro, kind, lastChapter, latestChapter, latestChapterUrl, wordCount, chapterCount, updateTime, status }。",
    "ChapterInfo: { name, url, group, vip, price, currency }，必须正序，第一章在前。",
    "chapterContent 返回：novel 为纯文本；comic 为 JSON.stringify 图片 URL 数组或资源对象数组；video/music 为播放地址或带 headers 的 JSON 字符串。",
    "列表页只提取当前页面或接口直接提供的数据，不要为了补齐字数、章节数、状态、更新时间逐本请求详情页。",
    "",
    "## 工作流程",
    "",
    "严格按单模块闭环推进：探测当前模块 -> 实现当前模块 -> save_source -> 测试当前模块 -> 根据日志修复 -> 再进入下一个模块。",
    "默认顺序：explore -> bookInfo -> chapterList -> chapterContent -> search -> run_builtin_tests 或最终链路复测。无发现页时可以从 search 开始，但仍要说明原因。",
    "探测必须在书源运行环境中完成：优先 eval_in_source 请求真实 URL、解析真实 HTML/API；不要只凭首页或站点名字猜选择器。",
    "",
    "## 场景处理策略",
    "",
    "传统小说 HTML：优先 OGP meta 和稳定容器，目录正序，正文移除广告/脚本/导航后返回纯文本。",
    "纯 API 站：先写通 apiGet/apiPost、签名和解密，再映射 search/bookInfo/chapterList/chapterContent。",
    "漫画：chapterContent 返回图片数组 JSON；处理 data-src、data-original、懒加载、去重、防盗链 Referer；切片打乱时实现 processImage。",
    "视频/音频：目录保留 group 线路；苹果 CMS 解析 $$$、#、$；播放地址藏在播放器中时用 onRequest 嗅探 m3u8/mp4/mp3。",
    "CF/WAF/验证码：先普通 HTTP 检测 Just a moment、Checking your browser、Turnstile 等标记；需要时用 browser acquire 完成验证并同步 cookies。",
    "登录和会员：需要用户操作时显式提示，使用 browser 登录或 config 存储用户配置；不要写死私密凭据。",
    "Android JSON 转换：把 searchUrl/exploreUrl/ruleSearch/ruleBookInfo/ruleToc/ruleContent 转成真实 JS 函数；能写成站点专用代码时不要保留庞大解释器。",
    "",
    "## 验收要求",
    "",
    "- 每个核心函数入口打 legado.log，记录 URL、长度、数量等关键调试信息，但不打印 Cookie/token/密码。",
    "- save_source 保存的是完整文件内容，不要省略、不要只保存函数片段。",
    "- search/explore 返回 BookItem[]；bookInfo 返回 BookItem 且含 tocUrl；chapterList 返回正序 ChapterInfo[]；chapterContent 返回类型必须符合 @type。",
    "- 能跑工具就必须跑对应测试。失败时根据工具输出修复，不要伪装成功。",
    "- 最终回复简短说明保存的文件名、已实现模块、测试结果、仍需用户人工完成的 CF/登录/验证码/会员事项。",
    "",
    "现在等待用户描述目标网站或给出旧书源/日志，然后开始制作。",
  ].join("\n");
}

// ── 工具定义（AI SDK v6：inputSchema 替代 parameters）────────────────────

/**
 * @param sessionId 当前会话 ID，save_source 时用于创建版本快照
 */
function buildTools(sessionId?: string) {
  return {
    list_sources: tool({
      description: "列出所有已安装书源，返回书源名称、文件名、URL。用于了解已有书源结构以作参考。",
      inputSchema: z.object({}),
      execute: async (_input: Record<string, never>) => {
        const sources: BookSourceMeta[] = await listBookSources();
        return sources.map((s) => ({
          fileName: s.fileName,
          name: s.name,
          url: s.url,
          tags: s.tags,
          sourceType: s.sourceType,
        }));
      },
    }),

    read_source: tool({
      description: "读取指定书源的完整 JS 代码，用于参考已有书源的 Boa 兼容实现方式。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名（含 .js 后缀）"),
      }),
      execute: async ({ fileName }: { fileName: string }) => {
        return await readBookSource(fileName);
      },
    }),

    save_source: tool({
      description:
        '保存（新建或覆盖）书源 JS 文件。每次修改完代码后必须调用此工具保存，后续测试才能看到最新代码。fileName 只含文件名（如 "笔趣阁.js"），不含路径。',
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名（含 .js 后缀）"),
        content: z.string().describe("完整的书源 JS 代码（保守 Boa 兼容写法）"),
      }),
      execute: async ({ fileName, content }: { fileName: string; content: string }) => {
        // 保存到草稿目录，不出现在已安装书源列表中
        await invokeWithTimeout("booksource_save_draft", { fileName, content }, 10_000);
        // 自动创建草稿版本快照
        if (sessionId) {
          useAiSessionsStore().addDraftSnapshot(sessionId, fileName, content, [
            ...state.testResults,
          ]);
        }
        return { success: true, fileName, savedBytes: content.length };
      },
    }),

    eval_in_source: tool({
      description:
        "在指定书源文件作用域中执行任意 JS 代码，返回执行结果字符串。" +
        "可调用 legado.http.get/post、fetch 等 API 探测目标网站结构。" +
        '示例代码：var html = await legado.http.get("https://example.com"); return html.slice(0,3000);',
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名（含 .js 后缀）。也可用任意已存在的书源文件名。"),
        code: z.string().describe("要执行的 JS 代码（可用 async/await，可用 legado.* API）"),
      }),
      execute: async ({ fileName, code }: { fileName: string; code: string }) => {
        return await evalBookSource(fileName, code);
      },
    }),

    test_explore: tool({
      description:
        "调用已保存书源的 explore(page, category) 函数，返回分类列表或发现页结果。空 category 或 GETALL 用于获取分类。必须先 save_source 保存最新代码。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名（含 .js 后缀）"),
        category: z.string().optional().describe("分类名；留空或传 GETALL 时通常返回分类列表"),
        page: z.number().optional().describe("页码（默认 1）"),
        noCache: z.boolean().optional().describe("是否跳过发现页缓存"),
      }),
      execute: async ({
        fileName,
        category = "",
        page = 1,
        noCache,
      }: {
        fileName: string;
        category?: string;
        page?: number;
        noCache?: boolean;
      }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_explore",
          { fileName, category, page, noCache },
          35000,
        );
      },
    }),

    test_search: tool({
      description:
        "调用已保存书源的 search(keyword, page) 函数，返回搜索结果。必须先 save_source 保存最新代码。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名（含 .js 后缀）"),
        keyword: z.string().describe("搜索关键词"),
        page: z.number().optional().describe("页码（默认 1）"),
      }),
      execute: async ({
        fileName,
        keyword,
        page = 1,
      }: {
        fileName: string;
        keyword: string;
        page?: number;
      }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_search",
          { fileName, keyword, page },
          35000,
        );
      },
    }),

    test_book_info: tool({
      description:
        "调用书源的 bookInfo(bookUrl) 函数，返回书籍详情（name/author/coverUrl/intro/tocUrl 等）。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名"),
        bookUrl: z.string().describe("书籍详情页 URL（来自搜索结果）"),
      }),
      execute: async ({ fileName, bookUrl }: { fileName: string; bookUrl: string }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_book_info",
          { fileName, bookUrl },
          35000,
        );
      },
    }),

    test_chapter_list: tool({
      description: "调用书源的 chapterList(tocUrl) 函数，返回章节目录数组（正序）。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名"),
        bookUrl: z.string().describe("书籍/目录入口 URL"),
      }),
      execute: async ({ fileName, bookUrl }: { fileName: string; bookUrl: string }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_chapter_list",
          { fileName, bookUrl, taskId: null },
          60000,
        );
      },
    }),

    test_chapter_content: tool({
      description:
        "调用书源的 chapterContent(chapterUrl) 函数，返回章节正文（小说→文本，漫画→图片 URL 数组 JSON，视频→播放地址）。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名"),
        chapterUrl: z.string().describe("章节 URL（来自章节列表）"),
      }),
      execute: async ({ fileName, chapterUrl }: { fileName: string; chapterUrl: string }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_chapter_content",
          { fileName, chapterUrl },
          35000,
        );
      },
    }),

    test_purchase_chapter: tool({
      description:
        "调用书源的 purchaseChapter(chapterUrl, chapter) 函数，用于测试 VIP 章节购买流程。只有用户明确要求时使用。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名"),
        chapterUrl: z.string().describe("VIP 章节 URL"),
        chapterName: z.string().optional().describe("章节名"),
      }),
      execute: async ({
        fileName,
        chapterUrl,
        chapterName,
      }: {
        fileName: string;
        chapterUrl: string;
        chapterName?: string;
      }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_purchase_chapter",
          {
            fileName,
            chapterUrl,
            chapter: {
              name: chapterName ?? chapterUrl,
              url: chapterUrl,
              vip: true,
            },
          },
          35000,
        );
      },
    }),

    run_builtin_tests: tool({
      description:
        "运行书源内置 TEST(type) 测试。只有书源已经实现 TEST 函数时使用；可全量运行，也可用 stepFilter 只运行 search/bookInfo/chapterList/chapterContent/explore。",
      inputSchema: z.object({
        fileName: z.string().describe("书源文件名（含 .js 后缀）"),
        timeoutSecs: z.number().optional().describe("总超时秒数，默认 150"),
        stepFilter: z
          .enum(["search", "bookInfo", "chapterList", "chapterContent", "explore"])
          .optional()
          .describe("只运行指定 TEST 步骤；留空为全量"),
      }),
      execute: async ({
        fileName,
        timeoutSecs = 150,
        stepFilter,
      }: {
        fileName: string;
        timeoutSecs?: number;
        stepFilter?: string;
      }) => {
        return await invokeWithTimeout<unknown>(
          "booksource_run_tests",
          { fileName, timeoutSecs, stepFilter },
          timeoutSecs * 1000 + 5000,
        );
      },
    }),
  };
}

// ── 全局单例状态 ──────────────────────────────────────────────────────────

let _activityCounter = 0;

const state = reactive({
  activities: [] as AgentActivity[],
  isRunning: false,
  currentFileName: "",
  currentSourceCode: "",
  testResults: [] as TestResult[],
  /** 当前正在流式输出的思考条目 id（-1 表示无） */
  activeThinkingId: -1,
});

let _abortController: AbortController | null = null;

// ── 内部工具函数 ──────────────────────────────────────────────────────────

function addActivity(
  type: ActivityType,
  content: string,
  opts?: { toolName?: string; args?: string; result?: string },
): number {
  const id = ++_activityCounter;
  state.activities.push({
    id,
    type,
    content,
    toolName: opts?.toolName,
    args: opts?.args,
    result: opts?.result,
    timestamp: Date.now(),
  });
  // 防止列表过长（保留最近 300 条）
  if (state.activities.length > 300) {
    state.activities.splice(0, state.activities.length - 300);
  }
  return id;
}

function updateActivity(id: number, patch: Partial<AgentActivity>) {
  const idx = state.activities.findIndex((a) => a.id === id);
  if (idx !== -1) {
    state.activities[idx] = { ...state.activities[idx], ...patch };
  }
}

function upsertTestResult(name: string, status: TestStatus, output: string) {
  const idx = state.testResults.findIndex((r) => r.name === name);
  const entry: TestResult = { name, status, output };
  if (idx !== -1) {
    state.testResults[idx] = entry;
  } else {
    state.testResults.push(entry);
  }
}

function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

type JsonRecord = Record<string, unknown>;
type NativeFetch = typeof fetch;

const LEGADO_REASONING_PROVIDER = "legadoReasoning";
let backendHttpProxyUrlPromise: Promise<string> | null = null;

function getBackendHttpProxyUrl(): Promise<string> {
  backendHttpProxyUrlPromise ??= invokeWithTimeout<string>("ai_http_proxy_url", {}, 5_000).catch(
    (error) => {
      backendHttpProxyUrlPromise = null;
      throw error;
    },
  );
  return backendHttpProxyUrlPromise;
}

function abortError(): Error {
  return new Error("AI 请求已取消");
}

function isAbortSignal(value: unknown): value is AbortSignal {
  return typeof AbortSignal !== "undefined" && value instanceof AbortSignal;
}

function isAborted(signal: AbortSignal, init: Parameters<NativeFetch>[1]): boolean {
  return signal.aborted || (isAbortSignal(init?.signal) && init.signal.aborted);
}

function waitForAbort(signal: AbortSignal, init: Parameters<NativeFetch>[1]): Promise<never> {
  return new Promise((_, reject) => {
    const initSignal = isAbortSignal(init?.signal) ? init.signal : undefined;
    const cleanup = () => {
      signal.removeEventListener("abort", onAbort);
      initSignal?.removeEventListener("abort", onAbort);
    };
    const onAbort = () => {
      cleanup();
      reject(abortError());
    };

    if (signal.aborted || initSignal?.aborted) {
      onAbort();
      return;
    }

    signal.addEventListener("abort", onAbort, { once: true });
    initSignal?.addEventListener("abort", onAbort, { once: true });
  });
}

function requestUrl(input: Parameters<NativeFetch>[0]): string {
  if (typeof input === "string") {
    return input;
  }
  if (input instanceof URL) {
    return input.href;
  }
  if (typeof Request !== "undefined" && input instanceof Request) {
    return input.url;
  }
  return String(input);
}

function requestMethod(
  input: Parameters<NativeFetch>[0],
  init: Parameters<NativeFetch>[1],
): string {
  return (
    init?.method ??
    (typeof Request !== "undefined" && input instanceof Request ? input.method : "GET")
  );
}

function requestHeaders(
  input: Parameters<NativeFetch>[0],
  init: Parameters<NativeFetch>[1],
): Array<[string, string]> {
  const headers = new Headers(
    typeof Request !== "undefined" && input instanceof Request ? input.headers : undefined,
  );
  new Headers(init?.headers).forEach((value, key) => {
    headers.set(key, value);
  });
  return [...headers.entries()];
}

async function requestBody(
  input: Parameters<NativeFetch>[0],
  init: Parameters<NativeFetch>[1],
): Promise<string | null> {
  const body = init?.body;
  if (body === null) {
    return null;
  }
  if (typeof body === "string") {
    return body;
  }
  if (body !== undefined) {
    return await new Response(body).text();
  }
  if (typeof Request !== "undefined" && input instanceof Request) {
    const method = input.method.toUpperCase();
    if (method !== "GET" && method !== "HEAD") {
      return await input.clone().text();
    }
  }
  return null;
}

function createBackendHttpFetch(signal: AbortSignal): NativeFetch {
  return async (input, init) => {
    if (isAborted(signal, init)) {
      throw abortError();
    }

    const url = requestUrl(input);
    const method = requestMethod(input, init);
    const body = await requestBody(input, init);
    const headers = requestHeaders(input, init);
    const proxyUrl = await Promise.race([getBackendHttpProxyUrl(), waitForAbort(signal, init)]);

    if (isAborted(signal, init)) {
      throw abortError();
    }

    return fetch(`${proxyUrl}?url=${encodeURIComponent(url)}`, {
      method,
      headers,
      body,
      signal: isAbortSignal(init?.signal) ? init.signal : signal,
    });
  };
}

function createFrontendHttpFetch(signal: AbortSignal): NativeFetch {
  return async (input, init) => {
    if (isAborted(signal, init)) {
      throw abortError();
    }
    return fetch(input, {
      ...init,
      signal: isAbortSignal(init?.signal) ? init.signal : signal,
    });
  };
}

function isRecord(value: unknown): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function stringProp(value: unknown, key: string): string | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  const prop = value[key];
  return typeof prop === "string" && prop.length > 0 ? prop : undefined;
}

function readRawReasoningContent(value: unknown): string {
  return (
    stringProp(value, "reasoning_content") ??
    stringProp(value, "reasoningContent") ??
    stringProp(value, "reasoning") ??
    ""
  );
}

function readStoredReasoningContent(part: JsonRecord): string | undefined {
  const providerOptions = isRecord(part.providerOptions) ? part.providerOptions : undefined;
  const stored = providerOptions?.[LEGADO_REASONING_PROVIDER];
  if (isRecord(stored)) {
    return stringProp(stored, "reasoningContent");
  }
  const providerMetadata = isRecord(part.providerMetadata) ? part.providerMetadata : undefined;
  const storedMetadata = providerMetadata?.[LEGADO_REASONING_PROVIDER];
  if (isRecord(storedMetadata)) {
    return stringProp(storedMetadata, "reasoningContent");
  }
  return undefined;
}

function withStoredReasoningContent(part: JsonRecord, reasoningContent: string): JsonRecord {
  const providerOptions = isRecord(part.providerOptions) ? part.providerOptions : {};
  const existing = isRecord(providerOptions[LEGADO_REASONING_PROVIDER])
    ? providerOptions[LEGADO_REASONING_PROVIDER]
    : {};
  return {
    ...part,
    providerOptions: {
      ...providerOptions,
      [LEGADO_REASONING_PROVIDER]: {
        ...existing,
        reasoningContent,
      },
    },
  };
}

function createChatReasoningContentBridge(nativeFetch: NativeFetch) {
  const reasoningByToolCallId = new Map<string, string>();
  const reasoningByText = new Map<string, string>();
  const pendingCaptures = new Set<Promise<void>>();

  function isChatCompletionsRequest(input: Parameters<NativeFetch>[0]): boolean {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : typeof Request !== "undefined" && input instanceof Request
            ? input.url
            : "";
    return url.includes("/chat/completions");
  }

  async function waitForPendingCaptures(): Promise<void> {
    if (pendingCaptures.size === 0) {
      return;
    }
    await Promise.allSettled([...pendingCaptures]);
  }

  function rememberCapture(capture: {
    reasoningContent: string;
    text: string;
    toolCallIds: Set<string>;
  }): void {
    if (!capture.reasoningContent.trim()) {
      return;
    }
    for (const id of capture.toolCallIds) {
      reasoningByToolCallId.set(id, capture.reasoningContent);
    }
    if (capture.text.trim()) {
      reasoningByText.set(capture.text, capture.reasoningContent);
    }
  }

  function collectToolCallIds(value: unknown, target: Set<string>): void {
    if (!Array.isArray(value)) {
      return;
    }
    for (const item of value) {
      const id = stringProp(item, "id");
      if (id) {
        target.add(id);
      }
    }
  }

  function collectChoice(
    choice: unknown,
    capture: {
      reasoningContent: string;
      text: string;
      toolCallIds: Set<string>;
    },
  ): void {
    if (!isRecord(choice)) {
      return;
    }
    const delta = isRecord(choice.delta) ? choice.delta : undefined;
    const message = isRecord(choice.message) ? choice.message : undefined;
    const reasoningDelta = readRawReasoningContent(delta) || readRawReasoningContent(message);
    if (reasoningDelta) {
      capture.reasoningContent += reasoningDelta;
    }

    const contentDelta = stringProp(delta, "content") ?? stringProp(message, "content");
    if (contentDelta) {
      capture.text += contentDelta;
    }

    collectToolCallIds(delta?.tool_calls, capture.toolCallIds);
    collectToolCallIds(message?.tool_calls, capture.toolCallIds);
  }

  function captureJsonPayload(
    payload: unknown,
    capture: {
      reasoningContent: string;
      text: string;
      toolCallIds: Set<string>;
    },
  ): void {
    if (!isRecord(payload) || !Array.isArray(payload.choices)) {
      return;
    }
    for (const choice of payload.choices) {
      collectChoice(choice, capture);
    }
  }

  function captureResponseText(text: string): void {
    const capture = {
      reasoningContent: "",
      text: "",
      toolCallIds: new Set<string>(),
    };

    const dataLines = text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.startsWith("data:"));

    if (dataLines.length > 0) {
      for (const line of dataLines) {
        const data = line.slice(5).trim();
        if (!data || data === "[DONE]") {
          continue;
        }
        try {
          captureJsonPayload(JSON.parse(data), capture);
        } catch {
          // ignore malformed SSE chunks from compatibility providers
        }
      }
    } else {
      try {
        captureJsonPayload(JSON.parse(text), capture);
      } catch {
        // ignore non-JSON bodies
      }
    }

    rememberCapture(capture);
  }

  function captureResponse(input: Parameters<NativeFetch>[0], response: Response): void {
    if (!isChatCompletionsRequest(input) || !response.ok) {
      return;
    }
    const capture = response
      .clone()
      .text()
      .then(captureResponseText)
      .catch(() => {});
    pendingCaptures.add(capture);
    capture.finally(() => pendingCaptures.delete(capture));
  }

  function findReasoningForOpenAIMessage(message: JsonRecord): string {
    const toolCalls = Array.isArray(message.tool_calls) ? message.tool_calls : [];
    for (const call of toolCalls) {
      const id = stringProp(call, "id");
      if (id) {
        const reasoning = reasoningByToolCallId.get(id);
        if (reasoning) {
          return reasoning;
        }
      }
    }

    const content = message.content;
    if (typeof content === "string") {
      return reasoningByText.get(content) ?? "";
    }
    return "";
  }

  async function patchRequestInit(
    input: Parameters<NativeFetch>[0],
    init: Parameters<NativeFetch>[1],
  ): Promise<Parameters<NativeFetch>[1]> {
    if (!isChatCompletionsRequest(input) || typeof init?.body !== "string") {
      return init;
    }

    await waitForPendingCaptures();

    let body: JsonRecord;
    try {
      body = JSON.parse(init.body) as JsonRecord;
    } catch {
      return init;
    }
    if (!Array.isArray(body.messages)) {
      return init;
    }

    let changed = false;
    const messages = body.messages.map((message) => {
      if (!isRecord(message) || message.role !== "assistant") {
        return message;
      }
      if (typeof message.reasoning_content === "string") {
        return message;
      }
      const reasoningContent = findReasoningForOpenAIMessage(message);
      if (!reasoningContent) {
        return message;
      }
      changed = true;
      return { ...message, reasoning_content: reasoningContent };
    });

    return changed ? { ...init, body: JSON.stringify({ ...body, messages }) } : init;
  }

  function preloadFromMessages(messages: ModelMessage[]): void {
    for (const message of messages as unknown[]) {
      if (!isRecord(message) || message.role !== "assistant") {
        continue;
      }
      const content = message.content;
      if (!Array.isArray(content)) {
        continue;
      }
      for (const part of content) {
        if (!isRecord(part)) {
          continue;
        }
        const reasoningContent = readStoredReasoningContent(part);
        if (!reasoningContent) {
          continue;
        }
        if (part.type === "tool-call") {
          const toolCallId = stringProp(part, "toolCallId");
          if (toolCallId) {
            reasoningByToolCallId.set(toolCallId, reasoningContent);
          }
        } else if (part.type === "text") {
          const text = stringProp(part, "text");
          if (text) {
            reasoningByText.set(text, reasoningContent);
          }
        }
      }
    }
  }

  function decorateMessages(messages: ModelMessage[]): ModelMessage[] {
    return messages.map((message) => {
      const record = message as unknown as JsonRecord;
      if (record.role !== "assistant") {
        return message;
      }

      const content = record.content;
      if (typeof content === "string") {
        const reasoningContent = reasoningByText.get(content);
        if (!reasoningContent) {
          return message;
        }
        return {
          ...record,
          content: [withStoredReasoningContent({ type: "text", text: content }, reasoningContent)],
        } as unknown as ModelMessage;
      }

      if (!Array.isArray(content)) {
        return message;
      }

      let changed = false;
      const nextContent = content.map((part) => {
        if (!isRecord(part)) {
          return part;
        }
        const existing = readStoredReasoningContent(part);
        if (part.type === "tool-call") {
          const toolCallId = stringProp(part, "toolCallId");
          const reasoningContent =
            (toolCallId ? reasoningByToolCallId.get(toolCallId) : undefined) ?? existing;
          if (!reasoningContent || existing === reasoningContent) {
            return part;
          }
          changed = true;
          return withStoredReasoningContent(part, reasoningContent);
        }
        if (part.type === "text") {
          const text = stringProp(part, "text");
          const reasoningContent = (text ? reasoningByText.get(text) : undefined) ?? existing;
          if (!reasoningContent || existing === reasoningContent) {
            return part;
          }
          changed = true;
          return withStoredReasoningContent(part, reasoningContent);
        }
        return part;
      });

      return changed ? ({ ...record, content: nextContent } as unknown as ModelMessage) : message;
    });
  }

  const fetchWithReasoningContent: NativeFetch = async (input, init) => {
    const patchedInit = await patchRequestInit(input, init);
    const response = await nativeFetch(input, patchedInit);
    captureResponse(input, response);
    return response;
  };

  return {
    fetch: fetchWithReasoningContent,
    preloadFromMessages,
    decorateMessages,
    waitForPendingCaptures,
  };
}

/**
 * 尝试从任意错误对象中提取可读错误信息。
 * 支持 AI SDK 的 APICallError、普通 Error、字符串、JSON 对象等。
 */
function formatError(err: unknown): string {
  if (!err) {
    return "未知错误";
  }
  if (typeof err === "string") {
    return err || "未知错误";
  }

  const parts: string[] = [];

  if (typeof err === "object") {
    const e = err as Record<string, unknown>;
    // 常规 message
    if (typeof e["message"] === "string" && e["message"]) {
      parts.push(e["message"]);
    }
    // AI SDK 标准字段
    if (typeof e["statusCode"] === "number") {
      parts.push(`HTTP ${e["statusCode"]}`);
    }
    if (typeof e["responseBody"] === "string" && e["responseBody"]) {
      const body = e["responseBody"].slice(0, 500);
      parts.push(`响应体: ${body}`);
    }
    // cause 链
    if (e["cause"] instanceof Error) {
      parts.push(`cause: ${e["cause"].message}`);
    } else if (typeof e["cause"] === "string" && e["cause"]) {
      parts.push(`cause: ${e["cause"]}`);
    }
    // 最后兑当：如果什么都没得到，直接 JSON 序列化
    if (parts.length === 0) {
      try {
        const json = JSON.stringify(err, Object.getOwnPropertyNames(err), 2);
        parts.push(json.slice(0, 800));
      } catch {
        parts.push(String(err));
      }
    }
  }

  const message = parts.join("\n") || "未知错误";
  if (/reasoning_content|thinking mode/i.test(message)) {
    return `${message}\n提示：当前版本会为 Chat Completions 兼容接口自动回传新产生的 reasoning_content；如果这是旧会话，或接口没有在流式响应里返回 reasoning_content，请新建任务，或切换到非思考模型 / Responses API 模式。`;
  }
  return message;
}

const TEST_TOOL_NAMES: Record<string, string> = {
  test_explore: "发现",
  test_search: "搜索",
  test_book_info: "书籍详情",
  test_chapter_list: "章节目录",
  test_chapter_content: "章节正文",
  test_purchase_chapter: "购买章节",
  run_builtin_tests: "内置 TEST",
};

// ── 主 Agent 函数 ─────────────────────────────────────────────────────────

export interface RunAiAgentOptions {
  /** 要写入的会话 ID，提供后自动持久化活动日志、测试结果、对话历史 */
  sessionId?: string;
  /**
   * 继续对话模式：true 时从会话中加载历史消息，不清空当前活动日志。
   * false（默认）时清空活动日志重新开始，对话历史仍追加到会话中。
   */
  continueConversation?: boolean;
}

export async function runAiAgent(
  config: AiConfig,
  userPrompt: string,
  options: RunAiAgentOptions = {},
): Promise<void> {
  if (state.isRunning) {
    throw new Error("Agent 正在运行中，请先停止");
  }

  const { sessionId, continueConversation = false } = options;

  state.isRunning = true;
  state.activeThinkingId = -1;
  _abortController = new AbortController();

  // 继续对话时：从会话恢复现有状态；全新任务时：清空
  let prevMessages: ModelMessage[] = [];
  if (sessionId) {
    const session = useAiSessionsStore().sessions.find((s) => s.id === sessionId);
    if (session) {
      if (continueConversation) {
        state.activities = [...session.activities];
        state.testResults = [...session.testResults];
        state.currentFileName = session.currentFileName;
        state.currentSourceCode = session.currentSourceCode;
        prevMessages = session.conversationHistory as ModelMessage[];
      } else {
        state.activities = [];
        state.testResults = [];
        state.currentFileName = session.currentFileName;
        state.currentSourceCode = session.currentSourceCode;
      }
    } else {
      state.activities = [];
      state.testResults = [];
      state.currentFileName = "";
      state.currentSourceCode = "";
    }
  } else {
    state.activities = [];
    state.testResults = [];
    state.currentFileName = "";
    state.currentSourceCode = "";
  }

  addActivity("info", continueConversation ? `继续对话：${userPrompt}` : `开始任务：${userPrompt}`);

  const useChatCompletions = (config.apiMode ?? "chat") === "chat";
  const requestTransport = config.requestTransport ?? "backend";
  const modelFetch =
    requestTransport === "frontend"
      ? createFrontendHttpFetch(_abortController.signal)
      : createBackendHttpFetch(_abortController.signal);

  addActivity(
    "info",
    requestTransport === "frontend"
      ? "请求通道：前端直连（支持流式显示，可能受 CORS 限制）"
      : "请求通道：后端 HTTP 代理（支持流式显示，并复用应用代理和 TLS 设置）",
  );

  const reasoningBridge = useChatCompletions
    ? createChatReasoningContentBridge(modelFetch)
    : undefined;
  if (reasoningBridge) {
    reasoningBridge.preloadFromMessages(prevMessages);
    prevMessages = reasoningBridge.decorateMessages(prevMessages);
  }

  // 构建发送给模型的消息列表（历史 + 本次用户指令）
  const inputMessages: ModelMessage[] = [...prevMessages, { role: "user", content: userPrompt }];

  try {
    const openaiProvider = createOpenAI({
      apiKey: config.apiKey || "placeholder",
      baseURL: config.apiUrl.replace(/\/$/, ""),
      fetch: reasoningBridge?.fetch ?? modelFetch,
    });

    // 根据 apiMode 选择请求路径：
    //   chat     → openaiProvider.chat(model)  → POST /v1/chat/completions
    //   responses → openaiProvider(model)         → POST /v1/responses
    const llmModel = !useChatCompletions
      ? openaiProvider(config.model)
      : openaiProvider.chat(config.model);

    // toolCallId → 对应的 tool_call 活动 id（用于 tool-result 时回填）
    const toolCallActivityMap = new Map<string, number>();
    let hadStreamError = false;

    const result = streamText({
      model: llmModel,
      system: buildSystemPrompt(),
      messages: inputMessages,
      tools: buildTools(sessionId),
      stopWhen: stepCountIs(Math.max(1, config.maxSteps ?? 30)),
      abortSignal: _abortController.signal,
      ...(config.temperature !== null && config.temperature !== undefined
        ? { temperature: config.temperature }
        : {}),
      ...(config.maxTokens !== null && config.maxTokens !== undefined && config.maxTokens > 0
        ? { maxTokens: config.maxTokens }
        : {}),
    });

    for await (const part of result.fullStream) {
      if (_abortController.signal.aborted) {
        break;
      }

      if (part.type === "start-step") {
        // 新推理步骤开始，创建思考条目
        state.activeThinkingId = addActivity("thinking", "");
      } else if (part.type === "text-delta") {
        // 流式文字 delta（AI SDK v6：属性名为 text，非 textDelta）
        if (state.activeThinkingId !== -1) {
          const existing = state.activities.find((a) => a.id === state.activeThinkingId);
          updateActivity(state.activeThinkingId, {
            content: (existing?.content ?? "") + part.text,
          });
        } else {
          state.activeThinkingId = addActivity("thinking", part.text);
        }
      } else if (part.type === "finish-step") {
        // 步骤结束，清除空思考条目
        if (state.activeThinkingId !== -1) {
          const existing = state.activities.find((a) => a.id === state.activeThinkingId);
          if (!existing?.content?.trim()) {
            const idx = state.activities.findIndex((a) => a.id === state.activeThinkingId);
            if (idx !== -1) {
              state.activities.splice(idx, 1);
            }
          }
          state.activeThinkingId = -1;
        }
      } else if (part.type === "tool-call") {
        // 工具调用（AI SDK v6：input 是参数，非 args）
        const argsStr = safeJsonStringify(part.input);
        const actId = addActivity("tool_call", "", {
          toolName: part.toolName,
          args: argsStr,
        });
        toolCallActivityMap.set(part.toolCallId, actId);
      } else if (part.type === "tool-result") {
        // 工具结果（AI SDK v6：output 是结果，非 result）
        const resultStr = safeJsonStringify(part.output);

        // 回填对应 tool_call 条目的 result 字段
        const actId = toolCallActivityMap.get(part.toolCallId);
        if (actId !== undefined) {
          updateActivity(actId, { result: resultStr });
        }

        // save_source → 更新代码预览
        if (part.toolName === "save_source") {
          const inp = part.input as { fileName?: string; content?: string };
          if (inp.fileName) {
            state.currentFileName = inp.fileName;
          }
          if (inp.content) {
            state.currentSourceCode = inp.content;
          }
        }

        // 测试工具 → 记录测试结果
        const testName = TEST_TOOL_NAMES[part.toolName ?? ""];
        if (testName) {
          const out = part.output;
          const isError = out !== null && typeof out === "object" && "error" in out;
          upsertTestResult(testName, isError ? "error" : "ok", resultStr.slice(0, 3000));
        }
      } else if (part.type === "error") {
        hadStreamError = true;
        addActivity("error", formatError(part.error));
      }
    }

    await reasoningBridge?.waitForPendingCaptures();

    // 获取最终文字回复（多步后通常是总结性内容）
    let finalText = "";
    try {
      finalText = await result.text;
      if (finalText?.trim()) {
        addActivity("message", finalText);
      }
    } catch {
      // 忽略（aborted 等情况）
    }

    addActivity(
      "info",
      hadStreamError
        ? "任务因错误终止"
        : state.currentFileName
          ? `书源 "${state.currentFileName}" 创建/更新完成`
          : "任务完成",
    );

    // 持久化会话：保存活动日志、测试结果，并追加本轮对话历史
    if (sessionId) {
      let newHistory: ModelMessage[] = inputMessages;
      try {
        const response = await result.response;
        if (response && "messages" in response && Array.isArray(response.messages)) {
          // AI SDK 返回本轮模型生成的消息（含工具调用），追加到对话历史
          newHistory = [...inputMessages, ...response.messages];
        } else {
          // 降级：只追加 AI 文字回复
          if (finalText?.trim()) {
            newHistory = [...inputMessages, { role: "assistant", content: finalText }];
          }
        }
      } catch {
        // 降级处理
        if (finalText?.trim()) {
          newHistory = [...inputMessages, { role: "assistant", content: finalText }];
        }
      }
      newHistory = reasoningBridge ? reasoningBridge.decorateMessages(newHistory) : newHistory;
      useAiSessionsStore().updateSession(sessionId, {
        activities: state.activities,
        testResults: state.testResults,
        currentFileName: state.currentFileName,
        currentSourceCode: state.currentSourceCode,
        conversationHistory: newHistory,
        status: hadStreamError ? "tested_fail" : "idle",
      });
    }
  } catch (e: unknown) {
    if (!_abortController?.signal.aborted) {
      addActivity("error", `Agent 运行出错：${formatError(e)}`);
    } else {
      addActivity("info", "已停止");
    }
    // 即使出错也持久化已有的活动日志
    if (sessionId) {
      useAiSessionsStore().updateSession(sessionId, {
        activities: state.activities,
        testResults: state.testResults,
        currentFileName: state.currentFileName,
        currentSourceCode: state.currentSourceCode,
      });
    }
  } finally {
    state.isRunning = false;
    state.activeThinkingId = -1;
    _abortController = null;
  }
}

export function stopAiAgent(): void {
  _abortController?.abort();
}

export function clearAgentState(): void {
  state.activities = [];
  state.testResults = [];
  state.currentFileName = "";
  state.currentSourceCode = "";
  state.activeThinkingId = -1;
}

// ── 导出 composable ───────────────────────────────────────────────────────

export function useAiAgent() {
  return {
    state,
    runAiAgent,
    stopAiAgent,
    clearAgentState,
    loadAiConfig,
    saveAiConfig,
  };
}
