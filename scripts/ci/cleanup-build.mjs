#!/usr/bin/env node

/**
 * cleanup-build.mjs
 *
 * 清理构建产物和临时文件，保留增量编译缓存。
 *
 * 用法：
 *   node scripts/ci/cleanup-build.mjs
 *   node scripts/ci/cleanup-build.mjs --dry-run
 */

import { readdirSync, statSync, rmSync, existsSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..", "..");
const dryRun = process.argv.includes("--dry-run");

const safeDirs = [
  { path: join(projectRoot, "reports", "gates"), desc: "gate reports" },
  { path: join(projectRoot, "构建结果"), desc: "build artifacts" },
];

let totalFiles = 0;
let totalDirs = 0;
let totalBytes = 0;

function countRecursive(dir) {
  if (!existsSync(dir)) return { files: 0, dirs: 0, bytes: 0 };
  let files = 0,
    dirs = 0,
    bytes = 0;
  try {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        const sub = countRecursive(full);
        files += sub.files;
        dirs += sub.dirs + 1;
        bytes += sub.bytes;
      } else {
        files += 1;
        bytes += statSync(full).size;
      }
    }
  } catch {
    /* skip inaccessible */
  }
  return { files, dirs, bytes };
}

console.log(dryRun ? "\n=== Cleanup DRY RUN ===\n" : "\n=== Cleanup ===\n");

for (const { path, desc } of safeDirs) {
  const { files, dirs, bytes } = countRecursive(path);
  if (files === 0) {
    console.log(`  [SKIP] ${desc}: empty`);
    continue;
  }
  const mb = (bytes / (1024 * 1024)).toFixed(1);
  if (dryRun) {
    console.log(`  [WOULD DELETE] ${desc}: ${files} files, ${dirs} dirs, ~${mb} MB`);
  } else {
    try {
      rmSync(path, { recursive: true, force: true });
      console.log(`  [DELETED] ${desc}: ${files} files, ${dirs} dirs, ~${mb} MB`);
    } catch (e) {
      console.log(`  [ERROR] ${desc}: ${e.message}`);
    }
  }
  totalFiles += files;
  totalDirs += dirs;
  totalBytes += bytes;
}

const totalMb = (totalBytes / (1024 * 1024)).toFixed(1);
if (dryRun) {
  console.log(`\nWould free ~${totalMb} MB (${totalFiles} files, ${totalDirs} dirs).`);
  console.log("Run without --dry-run to execute.");
} else {
  console.log(`\nFreed ~${totalMb} MB (${totalFiles} files, ${totalDirs} dirs).`);
}
