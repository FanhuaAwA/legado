#!/usr/bin/env node

/**
 * run-command.mjs
 *
 * 统一执行命令，记录耗时、退出码、stdout、stderr。
 * 供其他 CI 脚本调用。
 *
 * 用法：
 *   node scripts/ci/run-command.mjs --id <id> --cwd <dir> --log-dir <dir> -- <cmd...>
 *
 * 输出：JSON 格式的执行结果到 stdout。
 * 详细日志写入 <log-dir>/<id>.stdout.log 和 <log-dir>/<id>.stderr.log。
 */

import { execSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");

const args = process.argv.slice(2);
let id = "run";
let cwd = projectRoot;
let logDir = null;
const cmdParts = [];

let i = 0;
while (i < args.length) {
  if (args[i] === "--id" && i + 1 < args.length) {
    id = args[++i];
    i++;
  } else if (args[i] === "--cwd" && i + 1 < args.length) {
    cwd = resolve(args[++i]);
    i++;
  } else if (args[i] === "--log-dir" && i + 1 < args.length) {
    logDir = resolve(args[++i]);
    i++;
  } else if (args[i] === "--") {
    cmdParts.push(...args.slice(i + 1));
    break;
  } else {
    cmdParts.push(args[i]);
    i++;
  }
}

if (cmdParts.length === 0) {
  console.error("Usage: run-command.mjs --id <id> -- <cmd...>");
  process.exit(2);
}

const cmd = cmdParts.join(" ");
const start = Date.now();
let status = "unknown";
let exitCode = -1;
let stdout = "";
let stderr = "";
let blocker = null;

try {
  stdout = execSync(cmd, { cwd, encoding: "utf-8", timeout: 300_000, maxBuffer: 50 * 1024 * 1024 });
  exitCode = 0;
  status = "success";
} catch (e) {
  stdout = e.stdout?.toString() || "";
  stderr = e.stderr?.toString() || e.message;
  exitCode = e.status || 1;
  status = exitCode === 124 ? "timeout" : "failed";
  if (e.message?.includes("command not found") || e.message?.includes("is not recognized")) {
    blocker = "tool-missing";
  }
}

const durationMs = Date.now() - start;

const result = {
  id,
  command: cmd,
  cwd,
  startTime: new Date(start).toISOString(),
  durationMs,
  exitCode,
  status,
  blocker,
  stdoutLog: null,
  stderrLog: null,
};

if (logDir) {
  mkdirSync(logDir, { recursive: true });
  const stdoutPath = resolve(logDir, `${id}.stdout.log`);
  const stderrPath = resolve(logDir, `${id}.stderr.log`);
  writeFileSync(stdoutPath, stdout || "(empty)");
  writeFileSync(stderrPath, stderr || "(empty)");
  result.stdoutLog = stdoutPath;
  result.stderrLog = stderrPath;
}

console.log(JSON.stringify(result, null, 2));
process.exit(status === "success" ? 0 : 1);
