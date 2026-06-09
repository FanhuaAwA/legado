#!/usr/bin/env node

/**
 * write-report.mjs
 *
 * 从执行结果 JSON 生成 Markdown 和 JSON 格式的变更门禁报告。
 *
 * 用法：
 *   node scripts/ci/write-report.mjs --in commands.json --out report-dir/
 */

import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

const args = process.argv.slice(2);
let inputFile = null;
let outputDir = null;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--in" && i + 1 < args.length) {
    inputFile = resolve(args[++i]);
  } else if (args[i] === "--out" && i + 1 < args.length) {
    outputDir = resolve(args[++i]);
  }
}

if (!inputFile || !outputDir) {
  console.error("Usage: write-report.mjs --in <commands.json> --out <report-dir>");
  process.exit(2);
}

if (!existsSync(inputFile)) {
  console.error(`Input file not found: ${inputFile}`);
  process.exit(1);
}

const commands = JSON.parse(readFileSync(inputFile, "utf-8"));
const date = new Date().toISOString().replace(/T.*/, "");
const total = commands.length;
const passed = commands.filter((c) => c.status === "success").length;
const failed = commands.filter((c) => c.status === "failed").length;
const blocked = commands.filter((c) => c.status === "blocked").length;
const skipped = commands.filter((c) => c.status === "skipped").length;

let gateStatus = "success";
if (failed > 0) gateStatus = "failed";
else if (blocked > 0) gateStatus = "partial";
else if (passed < total) gateStatus = "partial";

mkdirSync(outputDir, { recursive: true });

// Generate summary.md
const summary = `# Change Gate Report

**Date:** ${date}
**Project:** ${projectRoot}
**Status:** ${gateStatus}

## Summary

| Metric | Count |
|--------|-------|
| Total checks | ${total} |
| Passed | ${passed} |
| Failed | ${failed} |
| Blocked | ${blocked} |
| Skipped | ${skipped} |

## Failed Tasks

${
  commands
    .filter((c) => c.status === "failed")
    .map((c) => `- **${c.id}**: exit code ${c.exitCode}, duration ${c.durationMs}ms`)
    .join("\n") || "_None_"
}

## Blocked Tasks

${
  commands
    .filter((c) => c.status === "blocked")
    .map((c) => `- **${c.id}**: ${c.blocker || "unknown"}`)
    .join("\n") || "_None_"
}

## All Commands

| ID | Status | Exit | Duration |
|----|--------|------|----------|
${commands.map((c) => `| ${c.id} | ${c.status} | ${c.exitCode} | ${c.durationMs}ms |`).join("\n")}
`;

writeFileSync(resolve(outputDir, "summary.md"), summary);
writeFileSync(
  resolve(outputDir, "commands.json"),
  JSON.stringify({ date, status: gateStatus, commands }, null, 2),
);

console.log(`Report written to ${outputDir}`);
console.log(`Status: ${gateStatus} (${passed}/${total} passed)`);

process.exit(gateStatus === "success" ? 0 : gateStatus === "partial" ? 0 : 1);
