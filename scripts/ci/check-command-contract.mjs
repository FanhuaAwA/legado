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

// ── 扫描前端 invoke ────────────────────────────────────────
function collectFrontendCommands() {
  const cmdToFiles = new Map();
  const srcDir = resolve(projectRoot, "src");
  walkDir(srcDir, (filePath) => {
    if (!/\.(ts|vue|js|tsx)$/.test(filePath)) return;
    try {
      const content = readFileSync(filePath, "utf-8");
      const re = /(?:invokeWithTimeout|(?<!\.)\binvoke)(?:<.+?>)?\s*\(\s*["']([^"']+)["']/g;
      let m;
      while ((m = re.exec(content)) !== null) {
        const name = m[1];
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

// ── 检测 unsupported stubs ────────────────────────────────
function detectStubs() {
  const stubs = new Set();
  const cmdDir = resolve(projectRoot, "src-tauri", "src", "commands");
  for (const fname of readdirSync(cmdDir)) {
    if (!fname.endsWith(".rs")) continue;
    const content = readFileSync(join(cmdDir, fname), "utf-8");
    // Match: #[tauri::command] pub async fn xxx(...) -> CommandResult<...> { Err(u("...")) }
    // or: Err(unsupported("..."))
    const re =
      /#\[tauri::command\][\s\S]*?pub\s+(?:async\s+)?fn\s+(\w+)\s*\([\s\S]*?\)\s*->\s*CommandResult[\s\S]*?\{\s*(?:Err\((?:u|unsupported)\()/g;
    let m;
    while ((m = re.exec(content)) !== null) {
      stubs.add(m[1]);
    }
  }
  return stubs;
}

// ── 检测已实现 command ─────────────────────────────────────
function detectImplemented() {
  const impl = new Set();
  const stubs = detectStubs();
  const cmdDir = resolve(projectRoot, "src-tauri", "src", "commands");
  for (const fname of readdirSync(cmdDir)) {
    if (!fname.endsWith(".rs")) continue;
    const content = readFileSync(join(cmdDir, fname), "utf-8");
    const re = /#\[tauri::command\][\s\S]*?pub\s+(?:async\s+)?fn\s+(\w+)\s*\(/g;
    let m;
    while ((m = re.exec(content)) !== null) {
      const name = m[1];
      if (!stubs.has(name) && !FALSE_BACKEND_SYMBOLS.has(name)) impl.add(name);
    }
  }
  return impl;
}

// ── 主逻辑 ──────────────────────────────────────────────────
const frontendCmds = collectFrontendCommands();
const registeredCmds = collectRegisteredCommands();
const unsupportedStubs = detectStubs();
const implementedCmds = detectImplemented();

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
    const info = { name, files: [...files].sort() };
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
    onlyFrontend.push({ name, files: [...files].sort() });
  }
}

for (const name of registeredCmds) {
  if (!frontendCmds.has(name)) onlyBackend.push(name);
}

onlyFrontend.sort((a, b) => a.name.localeCompare(b.name));
onlyBackend.sort();

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
        registered_unsupported_stub_count: unsupportedStubs.size,
        registered_implemented_count: implementedCmds.size,
        classification: {
          implemented: classified.implemented.map((c) => c.name).sort(),
          unsupported_stub: classified.unsupported_stub.map((c) => c.name).sort(),
          partial: classified.partial.map((c) => c.name).sort(),
          security_blocked: classified.security_blocked.map((c) => c.name).sort(),
        },
        onlyFrontend: onlyFrontend.map((c) => c.name).sort(),
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
