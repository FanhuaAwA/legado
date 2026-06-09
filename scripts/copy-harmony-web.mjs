#!/usr/bin/env node

/**
 * copy-harmony-web.mjs
 *
 * 将前端构建产物复制到 Harmony 工程目标目录。
 * 当前 Harmony 目标未启用 — 脚本仅提供最小骨架以便 package.json scripts 不报错。
 *
 * TODO：Harmony 工程集成后，补充真实的目标目录路径和资源复制逻辑。
 */

import { existsSync, cpSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const distDir = resolve(projectRoot, "dist");

console.log("[copy-harmony-web] Harmony target is not yet configured.");
if (!existsSync(distDir)) {
  console.error("[copy-harmony-web] dist/ directory not found. Run `pnpm build` first.");
  process.exit(1);
}
console.log("[copy-harmony-web] Skipping — no Harmony target path configured.");
