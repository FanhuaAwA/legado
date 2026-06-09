#!/usr/bin/env node

/**
 * booksource-node-runtime.mjs
 *
 * 书源 Node.js 运行器 — 在 Node 环境下测试书源 JS 脚本。
 * 提供 test 和 eval 两个子命令。
 *
 * 用法：
 *   node scripts/booksource-node-runtime.mjs test <source-file.js>
 *   node scripts/booksource-node-runtime.mjs eval <source-file.js> <entry-code>
 */

import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const command = args[0];

if (!command || !["test", "eval"].includes(command)) {
  console.log(
    "用法: node scripts/booksource-node-runtime.mjs <test|eval> <source-file> [entry-code]",
  );
  process.exit(1);
}

const sourceFile = args[1];
if (!sourceFile) {
  console.error("缺少书源文件路径");
  process.exit(1);
}

const absPath = resolve(sourceFile);
if (!existsSync(absPath)) {
  console.error(`书源文件不存在: ${absPath}`);
  process.exit(1);
}

const content = readFileSync(absPath, "utf-8");

if (command === "test") {
  console.log(`[booksource:node:test] 文件: ${absPath}`);
  console.log(`[booksource:node:test] 大小: ${content.length} bytes`);
  console.log(`[booksource:node:test] 当前仅支持词法分析，完整运行时需 Tauri/reader-core 环境。`);

  // 词法分析：检测顶层函数定义
  const fnPattern = /^(?:async\s+)?function\s+(\w+)/gm;
  const fns = [];
  for (const m of content.matchAll(fnPattern)) {
    fns.push(m[1]);
  }

  const metaPattern = /^\/\/\s*@(\w+)\s+(.+)/gm;
  const meta = {};
  for (const m of content.matchAll(metaPattern)) {
    meta[m[1]] = m[2];
  }

  console.log(`[booksource:node:test] 检测到 ${fns.length} 个函数: ${fns.join(", ") || "(无)"}`);
  if (Object.keys(meta).length > 0) {
    console.log(`[booksource:node:test] 元数据:`, meta);
  }
} else {
  // eval
  const entryCode = args[2];
  if (!entryCode) {
    console.error("eval 子命令需要 <entry-code> 参数");
    process.exit(1);
  }
  console.log(`[booksource:node:eval] 文件: ${absPath}`);
  console.log(`[booksource:node:eval] Node.js 环境中无法执行书源 JS（缺少 legado.* API）。`);
  console.log(`[booksource:node:eval] 请改用 Tauri 环境中的 booksource_eval 命令。`);
}
