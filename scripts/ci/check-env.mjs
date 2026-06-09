#!/usr/bin/env node

/**
 * check-env.mjs
 *
 * 检查必要的开发环境工具是否存在并输出版本信息。
 *
 * 用法：
 *   node scripts/ci/check-env.mjs
 *   node scripts/ci/check-env.mjs --json
 */

import { execSync } from "node:child_process";

/** @type {Array<{name: string, check: string, args: string[], required: boolean}>} */
const tools = [
  { name: "node", check: "node", args: ["--version"], required: true },
  { name: "pnpm", check: "pnpm", args: ["--version"], required: true },
  { name: "cargo", check: "cargo", args: ["--version"], required: true },
  { name: "rustc", check: "rustc", args: ["--version"], required: true },
  { name: "git", check: "git", args: ["--version"], required: false },
  { name: "tauri", check: "pnpm", args: ["exec", "tauri", "--version"], required: false },
];

const jsonMode = process.argv.includes("--json");
const results = [];

for (const tool of tools) {
  try {
    const out = execSync(`${tool.check} ${tool.args.join(" ")}`, {
      encoding: "utf-8",
      timeout: 10_000,
    }).trim();
    results.push({
      name: tool.name,
      found: true,
      version: out.split("\n")[0],
      required: tool.required,
    });
  } catch {
    results.push({ name: tool.name, found: false, version: null, required: tool.required });
  }
}

if (jsonMode) {
  console.log(JSON.stringify(results, null, 2));
} else {
  console.log("\n=== Environment Check ===\n");
  for (const r of results) {
    const icon = r.found ? "[OK]" : r.required ? "[MISSING]" : "[NOT FOUND]";
    console.log(`${icon} ${r.name}: ${r.version ?? "not installed"}`);
  }
  const missing = results.filter((r) => r.required && !r.found);
  if (missing.length > 0) {
    console.log(`\n${missing.length} required tools missing.`);
  } else {
    console.log("\nAll required tools available.");
  }
}

const missingRequired = results.filter((r) => r.required && !r.found);
process.exit(missingRequired.length > 0 ? 1 : 0);
