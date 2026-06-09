#!/usr/bin/env node

/**
 * check-command-contract.mjs
 *
 * 扫描前端 invoke 调用和后端 Tauri command 注册，输出差集报告。
 *
 * 检查逻辑：
 *   1. 扫描 src/**\/*.ts, src/**\/*.vue, src/**\/*.js 中的 invoke("...") 和 invokeWithTimeout("...")
 *   2. 解析 src-tauri/src/commands/mod.rs 中 generate_handler! 注册项
 *   3. 输出：前端调用但后端未注册、后端注册但前端未调用
 *
 * 用法：
 *   node scripts/ci/check-command-contract.mjs
 *   node scripts/ci/check-command-contract.mjs --json
 */

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { resolve, relative, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

// ── 已知配置项/非 command 误判 ──────────────────────────────
const KNOWN_NON_COMMANDS = new Set([
  "audio_url",
  "bookshelf_cache",
  "booksource_watcher_enabled",
  "savePath", // 有可能是变量名
]);

// ── 扫描前端 invoke 调用 ──────────────────────────────────
function collectFrontendCommands() {
  /** @type {Map<string, Set<string>>} */
  const cmdToFiles = new Map();

  const srcDir = resolve(projectRoot, "src");
  walkDir(srcDir, (filePath) => {
    if (!/\.(ts|vue|js|tsx)$/.test(filePath)) return;
    try {
      const content = readFileSync(filePath, "utf-8");
      // 统一模式: 匹配 invokeWithTimeout("...") 或 invoke("...")
      // 处理可选的 TypeScript 泛型参数 e.g. invokeWithTimeout<ShelfBook[]>("cmd", ...)
      const re = /(?:invokeWithTimeout|(?<!\.)\binvoke)(?:<[^>]*>)?\s*\(\s*["']([^"']+)["']/g;
      let match;
      while ((match = re.exec(content)) !== null) {
        const name = match[1];
        if (KNOWN_NON_COMMANDS.has(name)) continue;
        if (!cmdToFiles.has(name)) {
          cmdToFiles.set(name, new Set());
        }
        cmdToFiles.get(name).add(relative(projectRoot, filePath));
      }
    } catch {
      // skip unreadable files
    }
  });

  return cmdToFiles;
}

function walkDir(dir, callback) {
  if (!existsSync(dir)) return;
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "dist") continue;
      walkDir(full, callback);
    } else if (entry.isFile()) {
      callback(full);
    }
  }
}

// ── 解析后端注册命令 ──────────────────────────────────────
function collectRegisteredCommands() {
  const modPath = resolve(projectRoot, "src-tauri", "src", "commands", "mod.rs");
  if (!existsSync(modPath)) {
    console.error("ERROR: mod.rs not found at", modPath);
    process.exit(2);
  }
  const content = readFileSync(modPath, "utf-8");
  // 匹配 module::function_name 格式
  const pattern = /\b\w+::(\w+)\b/g;
  const cmds = new Set();
  let match;
  while ((match = pattern.exec(content)) !== null) {
    cmds.add(match[1]);
  }
  return cmds;
}

// ── 主逻辑 ────────────────────────────────────────────────
const frontendCmds = collectFrontendCommands();
const registeredCmds = collectRegisteredCommands();

// 分类
const onlyFrontend = [];
const onlyBackend = [];
const both = [];

for (const [name, files] of frontendCmds) {
  if (registeredCmds.has(name)) {
    both.push(name);
  } else {
    onlyFrontend.push({ name, files: [...files].sort() });
  }
}

for (const name of registeredCmds) {
  if (!frontendCmds.has(name)) {
    onlyBackend.push(name);
  }
}

onlyFrontend.sort((a, b) => a.name.localeCompare(b.name));
onlyBackend.sort();

// ── 输出 ──────────────────────────────────────────────────
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
        onlyFrontend,
        onlyBackend,
        both,
      },
      null,
      2,
    ),
  );
} else {
  console.log("\n=== Command Contract Check ===\n");
  console.log(`Frontend invoke calls: ${frontendCmds.size} unique`);
  console.log(`Tauri registered:      ${registeredCmds.size} commands`);
  console.log(`Both sides match:      ${both.length} commands`);

  if (onlyFrontend.length > 0) {
    console.log(`\n--- MISSING: Frontend calls NOT registered (${onlyFrontend.length}) ---\n`);
    for (const { name, files } of onlyFrontend) {
      console.log(`  ${name}`);
      for (const f of files.slice(0, 3)) {
        console.log(`    -> ${f}`);
      }
      if (files.length > 3) console.log(`    ... and ${files.length - 3} more files`);
    }
  }

  if (onlyBackend.length > 0) {
    console.log(
      `\n--- UNUSED: Registered but no frontend call found (${onlyBackend.length}) ---\n`,
    );
    for (const name of onlyBackend) {
      console.log(`  ${name}`);
    }
  }

  if (onlyFrontend.length === 0 && onlyBackend.length === 0) {
    console.log("\nAll frontend commands are registered, all registered have calls.\n");
  }

  console.log(
    `\nSummary: ${frontendCmds.size} frontend, ${registeredCmds.size} backend, ${onlyFrontend.length} unregistered`,
  );
}

process.exit(onlyFrontend.length > 0 ? 1 : 0);
