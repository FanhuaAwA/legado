#!/usr/bin/env node

/**
 * quality-gate.mjs
 *
 * 最小质量门禁脚本：检查项目基本健康状态。
 * 每个检查项独立运行，汇总报告。
 *
 * 用法：
 *   node scripts/ci/quality-gate.mjs
 *
 * 检查内容：
 *   1. package.json scripts 引用的文件是否存在
 *   2. 关键目录结构是否完整
 */

import { existsSync, statSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

/** @type {Array<{check: string, status: string, detail: string}>} */
const checks = [];

function addCheck(check, passed, detail) {
  checks.push({ check, status: passed ? "PASS" : "FAIL", detail });
}

// ── 1. 检查关键目录 ────────────────────
const requiredDirs = [
  "src",
  "src-tauri",
  "src-tauri/src",
  "src-tauri/src/commands",
  "crates/reader-core/src",
];
for (const dir of requiredDirs) {
  const full = resolve(projectRoot, dir);
  addCheck(`dir:${dir}`, existsSync(full) && statSync(full).isDirectory(), full);
}

// ── 2. 检查关键文件 ────────────────────
const requiredFiles = [
  "package.json",
  "Cargo.toml",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
  "index.html",
  "vite.config.ts",
];
for (const file of requiredFiles) {
  const full = resolve(projectRoot, file);
  addCheck(`file:${file}`, existsSync(full), full);
}

// ── 3. 检查 package scripts 引用的脚本文件 ────────────────────
try {
  execSync(`node "${resolve(__dirname, "check-scripts.mjs")}"`, {
    cwd: projectRoot,
    stdio: "pipe",
    timeout: 10_000,
  });
  addCheck("script-refs", true, "all script file references valid");
} catch (e) {
  const stderr = e.stderr?.toString() || "";
  const stdout = e.stdout?.toString() || "";
  addCheck("script-refs", false, stdout + stderr);
}

// ── 汇总 ──────────────────────────────
const passed = checks.filter((c) => c.status === "PASS").length;
const failed = checks.filter((c) => c.status === "FAIL").length;

console.log("\n=== Quality Gate Report ===\n");
for (const c of checks) {
  const icon = c.status === "PASS" ? "[PASS]" : "[FAIL]";
  console.log(`${icon} ${c.check}`);
  if (c.status === "FAIL") console.log(`     ${c.detail}`);
}
console.log(`\n${checks.length} checks: ${passed} passed, ${failed} failed`);

process.exit(failed > 0 ? 1 : 0);
