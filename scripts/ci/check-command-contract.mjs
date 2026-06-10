#!/usr/bin/env node

/**
 * check-command-contract.mjs v2
 *
 * 扫描前端 invoke 调用和后端 Tauri command 注册，输出分层状态报告。
 *
 * 改进：
 * - 只解析 generate_handler![...] 块内真实 command 条目
 * - 排除 generate_handler / ipc 等宏内部符号
 * - 按 implemented / unsupported_stub / partial 分类
 * - 输出 security_blocked 标记
 */

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { resolve, relative, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

// ── 已知误报项 ──────────────────────────────────────────────
const FALSE_BACKEND_SYMBOLS = new Set(["generate_handler", "ipc"]);
const KNOWN_NON_COMMANDS = new Set([
  "audio_url",
  "bookshelf_cache",
  "booksource_watcher_enabled",
  "savePath",
]);
const SECURITY_BLOCKED = new Set(["js_eval"]);
const compareStrings = (a, b) => a.localeCompare(b);

function skipWhitespaceAndComments(text, index) {
  let i = index;
  while (i < text.length) {
    if (/\s/.test(text[i])) {
      i++;
      continue;
    }
    if (text.startsWith("//", i)) {
      const end = text.indexOf("\n", i + 2);
      i = end < 0 ? text.length : end + 1;
      continue;
    }
    if (text.startsWith("/*", i)) {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 2;
      continue;
    }
    break;
  }
  return i;
}

function skipQuoted(text, index) {
  const quote = text[index];
  let i = index + 1;
  while (i < text.length) {
    const ch = text[i];
    if (ch === "\\") {
      i += 2;
      continue;
    }
    if (ch === quote) {
      return i + 1;
    }
    i++;
  }
  return text.length;
}

function skipTypeArguments(text, index) {
  let i = skipWhitespaceAndComments(text, index);
  if (text[i] !== "<") {
    return i;
  }

  let depth = 0;
  while (i < text.length) {
    const ch = text[i];
    if (ch === '"' || ch === "'" || ch === "`") {
      i = skipQuoted(text, i);
      continue;
    }
    if (text.startsWith("//", i) || text.startsWith("/*", i)) {
      i = skipWhitespaceAndComments(text, i);
      continue;
    }
    if (ch === "<") {
      depth++;
    } else if (ch === ">") {
      depth--;
      if (depth === 0) {
        return i + 1;
      }
    }
    i++;
  }
  return index;
}

function readStringLiteral(text, index) {
  const quote = text[index];
  if (quote !== '"' && quote !== "'") {
    return null;
  }
  let value = "";
  for (let i = index + 1; i < text.length; i++) {
    const ch = text[i];
    if (ch === "\\") {
      if (i + 1 < text.length) {
        value += text[i + 1];
        i++;
      }
      continue;
    }
    if (ch === quote) {
      return value;
    }
    value += ch;
  }
  return null;
}

function readFirstInvokeArgument(text, index) {
  let i = skipTypeArguments(text, index);
  i = skipWhitespaceAndComments(text, i);
  if (text[i] !== "(") {
    return null;
  }
  i = skipWhitespaceAndComments(text, i + 1);
  return readStringLiteral(text, i);
}

function scanFrontendInvokeCommands(content) {
  const commands = [];
  const re = /invokeWithTimeout|(?<!\.)\binvoke\b/g;
  let m;
  while ((m = re.exec(content)) !== null) {
    const name = readFirstInvokeArgument(content, m.index + m[0].length);
    if (name) {
      commands.push(name);
    }
  }
  return commands;
}

// ── 扫描前端 invoke ────────────────────────────────────────
function collectFrontendCommands() {
  const cmdToFiles = new Map();
  const srcDir = resolve(projectRoot, "src");
  walkDir(srcDir, (filePath) => {
    if (!/\.(ts|vue|js|tsx)$/.test(filePath)) return;
    try {
      const content = readFileSync(filePath, "utf-8");
      for (const name of scanFrontendInvokeCommands(content)) {
        if (KNOWN_NON_COMMANDS.has(name)) continue;
        if (!cmdToFiles.has(name)) cmdToFiles.set(name, new Set());
        cmdToFiles.get(name).add(relative(projectRoot, filePath));
      }
    } catch {
      /* skip */
    }
  });
  return cmdToFiles;
}

function walkDir(dir, cb) {
  if (!existsSync(dir)) return;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name !== "node_modules" && e.name !== "dist") walkDir(full, cb);
    } else if (/\.(ts|vue|js|tsx)$/.test(e.name)) cb(full);
  }
}

// ── 解析 generate_handler! 块 ──────────────────────────────
function collectRegisteredCommands() {
  const modPath = resolve(projectRoot, "src-tauri", "src", "commands", "mod.rs");
  if (!existsSync(modPath)) {
    console.error("ERROR: mod.rs not found");
    process.exit(2);
  }
  const content = readFileSync(modPath, "utf-8");
  // Extract the generate_handler![...] block
  const m = content.match(/generate_handler!\[([\s\S]*?)\]/);
  if (!m) {
    console.error("ERROR: generate_handler! not found");
    process.exit(2);
  }
  const block = m[1];
  const cmds = new Set();
  // Match module::function_name entries
  for (const line of block.split("\n")) {
    const fm = line.match(/^\s*(\w+)::(\w+)\s*,?\s*$/);
    if (fm) {
      const name = fm[2];
      if (!FALSE_BACKEND_SYMBOLS.has(name)) cmds.add(name);
    }
  }
  return cmds;
}

// ── 逐函数分类（stub / impl）───────────────────────────────
// 旧版用无界 [\s\S]*? 全文正则，会跨函数错位匹配：真实实现被后方 stub 的
// Err(unsupported( 标记为 stub，真正的 stub 反而被 lastIndex 跳过。
// 现改为：按 #[tauri::command] 切块 → 括号配平提取函数体 → 判定函数体整体
// 是否为单个 UNSUPPORTED Err 表达式。

/** 提取从 openIdx（指向 '{'）开始的配平大括号内容，返回 [body, endIdx]；失败返回 null */
function extractBraceBody(text, openIdx) {
  let depth = 0;
  let inStr = null; // '"' | null（Rust 命令体内不含带大括号的原始字符串，简单状态机足够）
  for (let i = openIdx; i < text.length; i++) {
    const ch = text[i];
    if (inStr) {
      if (ch === "\\") i++;
      else if (ch === inStr) inStr = null;
      continue;
    }
    if (ch === '"') inStr = '"';
    else if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return [text.slice(openIdx + 1, i), i];
    }
  }
  return null;
}

/** 函数体是否为「整体一个 UNSUPPORTED Err 表达式」 */
function bodyIsUnsupportedStub(body) {
  const cleaned = body
    .split("\n")
    .map((l) => l.replace(/\/\/.*$/, ""))
    .join("\n")
    .trim();
  if (/^Err\s*\(\s*(?:u|unsupported)\s*\(/.test(cleaned)) return true;
  if (/^Err\s*\(\s*CommandError\s*[{(]/.test(cleaned) && cleaned.includes("UNSUPPORTED"))
    return true;
  return false;
}

/** 解析单个 Rust 源文件，返回 Map<fnName, "stub"|"impl"> */
function classifyFileCommands(content) {
  const result = new Map();
  const attrRe = /#\[tauri::command\]/g;
  let am;
  while ((am = attrRe.exec(content)) !== null) {
    const after = content.slice(am.index);
    const fnm = after.match(/^\s*#\[tauri::command\][^]*?pub\s+(?:async\s+)?fn\s+(\w+)/);
    if (!fnm) continue;
    const name = fnm[1];
    // 从 fn 名之后找第一个不在圆括号内的 '{'（即函数体开括号）
    const fnNameEnd = am.index + fnm[0].length;
    let parenDepth = 0;
    let bodyOpen = -1;
    let inStr = null;
    for (let i = fnNameEnd; i < content.length; i++) {
      const ch = content[i];
      if (inStr) {
        if (ch === "\\") i++;
        else if (ch === inStr) inStr = null;
        continue;
      }
      if (ch === '"') inStr = '"';
      else if (ch === "(") parenDepth++;
      else if (ch === ")") parenDepth--;
      else if (ch === "{" && parenDepth === 0) {
        bodyOpen = i;
        break;
      } else if (ch === "#" && content.slice(i, i + 17) === "#[tauri::command]") {
        break; // 防御：撞上下一个命令说明本函数无函数体
      }
    }
    if (bodyOpen < 0) continue;
    const extracted = extractBraceBody(content, bodyOpen);
    if (!extracted) continue;
    result.set(name, bodyIsUnsupportedStub(extracted[0]) ? "stub" : "impl");
  }
  return result;
}

/** 扫描全部 command 文件，返回 { stubs:Set, impls:Set } */
function classifyAllCommands() {
  const stubs = new Set();
  const impls = new Set();
  const cmdDir = resolve(projectRoot, "src-tauri", "src", "commands");
  for (const fname of readdirSync(cmdDir)) {
    if (!fname.endsWith(".rs")) continue;
    const content = readFileSync(join(cmdDir, fname), "utf-8");
    for (const [name, kind] of classifyFileCommands(content)) {
      if (FALSE_BACKEND_SYMBOLS.has(name)) continue;
      if (kind === "stub") stubs.add(name);
      else impls.add(name);
    }
  }
  return { stubs, impls };
}

// ── 分类器内置自检（夹具回归，分类错误时脚本自身报错）──────
function selfTestClassifier() {
  const fixture = `
fn unsupported(f: &str) -> CommandError { CommandError { code: "UNSUPPORTED".into(), message: f.into(), detail: None, retryable: false } }

#[tauri::command]
pub async fn fixture_real_impl(state: State<'_, AppState>) -> CommandResult<Report> {
    let stats = build_stats(&state).map_err(|e| CommandError { code: "IO_ERROR".into(), message: e.to_string(), detail: None, retryable: false })?;
    Ok(Report { stats })
}

#[tauri::command] pub async fn fixture_oneline_stub() -> CommandResult<()> { Err(unsupported("浏览器探测")) }

#[tauri::command]
pub async fn fixture_struct_stub(_req: Req) -> CommandResult<Vec<Option<[u32; 2]>>> {
    Err(CommandError {
        code: "UNSUPPORTED".into(),
        message: "尚未实现 {占位}".into(),
        detail: Some("not_implemented".into()),
        retryable: false,
    })
}

#[tauri::command]
pub async fn fixture_impl_after_stub() -> CommandResult<String> {
    let v = compute();
    Ok(v)
}
`;
  const got = classifyFileCommands(fixture);
  const expect = [
    ["fixture_real_impl", "impl"],
    ["fixture_oneline_stub", "stub"],
    ["fixture_struct_stub", "stub"],
    ["fixture_impl_after_stub", "impl"],
  ];
  for (const [name, kind] of expect) {
    if (got.get(name) !== kind) {
      console.error(
        `SELF-TEST FAILED: ${name} expected=${kind} got=${got.get(name) ?? "missing"} — 分类器损坏，输出不可信`,
      );
      process.exit(2);
    }
  }
}
selfTestClassifier();

function selfTestFrontendScanner() {
  const fixture = `
import { invokeWithTimeout } from "./useInvoke";
const data = await invokeWithTimeout<{
  fileName: string;
  nested: Array<{ id: string; value: Promise<{ ok: boolean }> }>;
}>("bookshelf_export_book_data", { id, format }, 10_000);
await invokeWithTimeout<{ valid: boolean }>(
  "sync_baidu_token_status",
  undefined,
  8000,
);
const auth = await invokeWithTimeout<{
  device_code: string;
  verification_url: string;
  user_code: string;
  expires_in: number;
  interval: number;
}>("sync_baidu_start_auth", undefined, 15000);
await invoke("plain_command", {});
bridge.invoke("not_a_tauri_command");
`;
  const got = new Set(scanFrontendInvokeCommands(fixture));
  const expected = [
    "bookshelf_export_book_data",
    "sync_baidu_token_status",
    "sync_baidu_start_auth",
    "plain_command",
  ];
  for (const name of expected) {
    if (!got.has(name)) {
      console.error(`SELF-TEST FAILED: frontend scanner missed ${name}`);
      process.exit(2);
    }
  }
  if (got.has("not_a_tauri_command")) {
    console.error("SELF-TEST FAILED: frontend scanner matched object.invoke");
    process.exit(2);
  }
}
selfTestFrontendScanner();

function detectStubs() {
  return classifyAllCommands().stubs;
}

function detectImplemented() {
  return classifyAllCommands().impls;
}

// ── 主逻辑 ──────────────────────────────────────────────────
const frontendCmds = collectFrontendCommands();
const registeredCmds = collectRegisteredCommands();
const unsupportedStubs = detectStubs();
const implementedCmds = detectImplemented();
const registeredUnsupportedStubs = [...unsupportedStubs].filter((name) => registeredCmds.has(name));
const registeredImplemented = [...implementedCmds].filter((name) => registeredCmds.has(name));

// Classify
const onlyFrontend = [];
const onlyBackend = [];
const both = [];
const classified = {
  implemented: [],
  unsupported_stub: [],
  partial: [],
  security_blocked: [],
  unknown: [],
};

for (const [name, files] of frontendCmds) {
  if (registeredCmds.has(name)) {
    both.push(name);
    const info = { name, files: [...files].sort(compareStrings) };
    if (SECURITY_BLOCKED.has(name)) {
      classified.security_blocked.push(info);
    } else if (unsupportedStubs.has(name)) {
      classified.unsupported_stub.push(info);
    } else if (implementedCmds.has(name)) {
      classified.implemented.push(info);
    } else {
      classified.unknown.push(info);
    }
  } else {
    onlyFrontend.push({ name, files: [...files].sort(compareStrings) });
  }
}

for (const name of registeredCmds) {
  if (!frontendCmds.has(name)) onlyBackend.push(name);
}

onlyFrontend.sort((a, b) => a.name.localeCompare(b.name));
onlyBackend.sort(compareStrings);

// ── 输出 ────────────────────────────────────────────────────
const jsonMode = process.argv.includes("--json");
if (jsonMode) {
  console.log(
    JSON.stringify(
      {
        frontendTotal: frontendCmds.size,
        registeredTotal: registeredCmds.size,
        bothCount: both.length,
        onlyFrontendCount: onlyFrontend.length,
        onlyBackendCount: onlyBackend.length,
        registered_unsupported_stub_count: registeredUnsupportedStubs.length,
        registered_implemented_count: registeredImplemented.length,
        frontend_unsupported_stub_count: classified.unsupported_stub.length,
        frontend_implemented_count: classified.implemented.length,
        classificationScope: "frontend-facing registered commands",
        registeredClassification: {
          implemented: registeredImplemented.sort(compareStrings),
          unsupported_stub: registeredUnsupportedStubs.sort(compareStrings),
        },
        classification: {
          implemented: classified.implemented.map((c) => c.name).sort(compareStrings),
          unsupported_stub: classified.unsupported_stub.map((c) => c.name).sort(compareStrings),
          partial: classified.partial.map((c) => c.name).sort(compareStrings),
          security_blocked: classified.security_blocked.map((c) => c.name).sort(compareStrings),
        },
        onlyFrontend: onlyFrontend.map((c) => c.name).sort(compareStrings),
        onlyBackend,
      },
      null,
      2,
    ),
  );
} else {
  console.log("\n=== Command Contract v2 ===\n");
  console.log(`Frontend calls:          ${frontendCmds.size}`);
  console.log(`Tauri registered:        ${registeredCmds.size}`);
  console.log(`Both sides match:        ${both.length}`);
  console.log(`Implemented:             ${classified.implemented.length}`);
  console.log(`Unsupported stubs:       ${classified.unsupported_stub.length}`);
  console.log(`Security blocked:        ${classified.security_blocked.length}`);
  console.log(`Frontend-only (missing): ${onlyFrontend.length}`);
  console.log(`Backend-only (unused):   ${onlyBackend.length}`);
  console.log(`False backend symbols:   ${FALSE_BACKEND_SYMBOLS.size} excluded`);
  if (classified.unsupported_stub.length > 0) {
    console.log(`\n--- UNSUPPORTED Frontend-facing (${classified.unsupported_stub.length}) ---`);
    for (const c of classified.unsupported_stub) console.log(`  ${c.name}`);
  }
}

process.exit(
  onlyFrontend.length > 0 && !onlyFrontend.every((f) => SECURITY_BLOCKED.has(f.name)) ? 1 : 0,
);
