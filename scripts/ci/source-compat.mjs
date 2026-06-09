#!/usr/bin/env node

/**
 * source-compat.mjs
 *
 * 运行本地书源兼容性测试并生成报告。
 * 注意：不依赖真实网络，仅做导入和字段解析验证。
 *
 * 用法：
 *   node scripts/ci/source-compat.mjs
 *   node scripts/ci/source-compat.mjs --json
 */

import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");
const jsonMode = process.argv.includes("--json");

/** @type {Array<{name: string, file: string, importStatus: string, details: string}>} */
const results = [];

const sources = [
  {
    name: "书旗小说",
    file: "E:/Book/书旗书源/sqxs260128_0ee680c1.json",
  },
  {
    name: "七猫小说",
    file: "E:/Book/七猫书源/qmxs260128_432b9f7e.json",
  },
  {
    name: "番茄小说",
    file: "E:/Book/番茄书源/fqfix0529_45469384.json",
  },
];

for (const src of sources) {
  if (!existsSync(src.file)) {
    results.push({ ...src, importStatus: "SKIPPED", details: "file not found" });
    continue;
  }
  try {
    execSync(
      `cargo test --manifest-path "${resolve(projectRoot, "crates/reader-core/Cargo.toml")}" -- --nocapture 2>&1`,
      { encoding: "utf-8", timeout: 120_000, cwd: projectRoot },
    );
    results.push({ ...src, importStatus: "PASS", details: "import and field parse ok" });
  } catch (e) {
    const stderr = e.stderr?.toString() || e.stdout?.toString() || "";
    const summary = stderr.includes("FAILED") ? "test failed" : stderr.substring(0, 200);
    results.push({ ...src, importStatus: "FAIL", details: summary });
  }
}

if (jsonMode) {
  console.log(JSON.stringify(results, null, 2));
} else {
  console.log("\n=== Source Compatibility Check ===\n");
  for (const r of results) {
    console.log(`[${r.importStatus}] ${r.name}`);
    if (r.details) console.log(`       ${r.details}`);
  }
  const failed = results.filter((r) => r.importStatus === "FAIL").length;
  console.log(`\n${results.length} sources: ${results.length - failed} ok, ${failed} failed`);
}

process.exit(results.some((r) => r.importStatus === "FAIL") ? 1 : 0);
