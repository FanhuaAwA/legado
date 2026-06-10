#!/usr/bin/env node

/**
 * generate-gate-report.mjs
 *
 * 运行门禁流水线并生成结构化报告到 reports/gates/YYYY-MM-DD-HHMM/。
 * 用法：node scripts/ci/generate-gate-report.mjs
 */

import { execSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

const pad = (n) => String(n).padStart(2, "0");
const now = new Date();
const ts = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}`;
const reportDir = resolve(projectRoot, "reports", "gates", ts);
mkdirSync(reportDir, { recursive: true });

const results = [];

function runStep(name, command, cwd = projectRoot) {
  try {
    const stdout = execSync(command, { cwd, timeout: 120_000, encoding: "utf-8", stdio: "pipe" });
    const status = "PASS";
    results.push({ step: name, status, output: stdout.slice(-500) });
    writeFileSync(resolve(reportDir, `${name.replace(/[^a-z0-9-]/g, "-")}.log`), stdout);
    console.log(`[PASS] ${name}`);
  } catch (e) {
    const stdout = e.stdout?.toString() || "";
    const stderr = e.stderr?.toString() || "";
    const status = "FAIL";
    results.push({ step: name, status, output: (stdout + stderr).slice(-500) });
    writeFileSync(
      resolve(reportDir, `${name.replace(/[^a-z0-9-]/g, "-")}.log`),
      stdout + "\n" + stderr,
    );
    console.log(`[FAIL] ${name}`);
  }
}

// ── 环境信息 ──────────────────────────
const env = {
  node: process.version,
  platform: process.platform,
  cwd: projectRoot,
  timestamp: new Date().toISOString(),
};
writeFileSync(resolve(reportDir, "env.json"), JSON.stringify(env, null, 2));
console.log(`[INFO] env recorded: Node ${env.node}, ${env.platform}`);

// ── 门禁步骤 ──────────────────────────
runStep("check-scripts", "node scripts/ci/check-scripts.mjs");
runStep("frontend-lint", "pnpm lint");
runStep("frontend-build", "pnpm build");
runStep("cargo-check-core", "cargo check -p reader-core");
runStep("cargo-test-core", "cargo test -p reader-core");
runStep("cargo-check-tauri", "cargo check -p legado-tauri");

// ── 生成摘要 ──────────────────────────
const passed = results.filter((r) => r.status === "PASS").length;
const failed = results.filter((r) => r.status === "FAIL").length;
const overall = failed === 0 ? "PASS" : "FAIL";

const summary = [
  `# 变更门禁报告 — ${ts}`,
  `\n## 结果：${overall}`,
  `\n| 步骤 | 状态 |`,
  `| ---- | ---- |`,
  ...results.map((r) => `| ${r.step} | ${r.status} |`),
  `\n${results.length} steps: ${passed} passed, ${failed} failed`,
].join("\n");

writeFileSync(resolve(reportDir, "summary.md"), summary);
console.log(`\n${results.length} steps: ${passed} passed, ${failed} failed`);
console.log(`Report: ${reportDir}`);

process.exit(failed > 0 ? 1 : 0);
