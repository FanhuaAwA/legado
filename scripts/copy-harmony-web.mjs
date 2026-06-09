#!/usr/bin/env node

/**
 * copy-harmony-web.mjs
 *
 * ⚠️ Harmony 构建目标当前不可用 ⚠️
 *
 * 此脚本尚未配置真实 Harmony 工程目标目录。
 * 如需启用 Harmony 构建，必须先完成：
 *   1. 配置 Harmony 工程路径
 *   2. 配置资源复制逻辑
 *   3. 验证产物可部署
 *
 * 在此之前，`pnpm run build:harmony` 将始终以非零退出码退出，
 * 以明确标记该能力尚未就绪。
 */

import { existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const distDir = resolve(projectRoot, "dist");

console.error("══════════════════════════════════════════════");
console.error("  Harmony 构建目标当前不可用");
console.error("  此平台尚未配置，构建产物不会生成");
console.error("══════════════════════════════════════════════");

if (!existsSync(distDir)) {
  console.error("此外，dist/ 目录不存在。请先运行 pnpm build。");
}

// 以非零退出码标记不可用
process.exit(2);
