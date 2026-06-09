#!/usr/bin/env node

/**
 * check-scripts.mjs
 *
 * 检查 package.json scripts 中引用的脚本文件是否存在。
 * 输出缺失文件列表，以 exit code 报告结果。
 *
 * 用法：
 *   node scripts/ci/check-scripts.mjs
 *   node scripts/ci/check-scripts.mjs --fix    （仅报告，不自动创建）
 */

import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

const pkgPath = resolve(projectRoot, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));

/** @type {Array<{scriptName: string, filePath: string, exists: boolean}>} */
const results = [];

for (const [scriptName, scriptBody] of Object.entries(pkg.scripts ?? {})) {
  // 匹配 node scripts/xxx.mjs 或类似的脚本文件引用
  const matches = scriptBody.matchAll(/node\s+(scripts\/[^\s;]+\.m?js)/g);
  for (const [, relativePath] of matches) {
    const absolutePath = resolve(projectRoot, relativePath);
    const exists = existsSync(absolutePath);
    results.push({ scriptName, filePath: relativePath, exists });
  }
}

// 输出结果
let missingCount = 0;
for (const r of results) {
  const status = r.exists ? "OK" : "MISSING";
  if (!r.exists) missingCount++;
  console.log(`[${status}] ${r.scriptName} -> ${r.filePath}`);
}

console.log(`\n${results.length} script file references checked, ${missingCount} missing.`);

if (missingCount > 0) {
  console.log("\nMissing script files:");
  for (const r of results) {
    if (!r.exists) {
      console.log(`  - ${r.filePath} (referenced by "${r.scriptName}")`);
    }
  }
  process.exit(1);
}
