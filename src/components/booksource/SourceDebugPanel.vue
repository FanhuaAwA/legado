<!--
  书源调试面板 — 对单个书源运行 search/bookInfo/toc/content/explore 测试
  并展示每步耗时、状态、错误信息、样本数据。
  使用 booksource_run_tests 命令。
-->
<template>
  <div class="source-debug-panel">
    <div class="debug-header">
      <h4>书源调试：{{ fileName }}</h4>
      <button class="btn-run" :disabled="running" @click="runTests">
        {{ running ? "运行中..." : "运行测试" }}
      </button>
      <label class="filter-label">
        仅测试：
        <select v-model="stepFilter" :disabled="running">
          <option value="">全部</option>
          <option value="search">搜索</option>
          <option value="bookInfo">详情</option>
          <option value="toc">目录</option>
          <option value="content">正文</option>
          <option value="explore">发现</option>
        </select>
      </label>
    </div>

    <!-- 结果表格 -->
    <div v-if="results.length > 0" class="results-table">
      <table>
        <thead>
          <tr>
            <th>步骤</th>
            <th>状态</th>
            <th>耗时</th>
            <th>数量</th>
            <th>预览</th>
            <th>错误</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in results" :key="r.name" :class="rowClass(r.status)">
            <td>{{ r.name }}</td>
            <td class="status-cell">
              <span :class="statusBadge(r.status)">{{ statusLabel(r.status) }}</span>
            </td>
            <td>{{ r.elapsed_ms ? r.elapsed_ms + "ms" : "—" }}</td>
            <td>{{ r.sample_count ?? "—" }}</td>
            <td class="preview-cell">
              <span :title="r.output_preview">{{
                r.output_preview ? truncate(r.output_preview, 60) : "—"
              }}</span>
            </td>
            <td class="error-cell" :title="r.error">
              {{ r.error ? truncate(r.error, 80) : "—" }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 汇总 -->
    <div v-if="results.length > 0" class="summary-row">
      通过 {{ passedCount }} / {{ results.length }}，总耗时 {{ totalTime }}ms
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { invokeWithTimeout } from "@/composables/useInvoke";

const props = defineProps<{
  fileName: string;
  sourceDir?: string | null;
}>();

function sourceDirOrUndef(): string | undefined {
  return props.sourceDir ?? undefined;
}

interface StepResult {
  name: string;
  status: string;
  elapsed_ms?: number;
  error?: string;
  sample_count?: number;
  output_preview?: string;
}

const running = ref(false);
const stepFilter = ref("");
const results = ref<StepResult[]>([]);

const passedCount = computed(() => results.value.filter((r) => r.status === "passed").length);
const totalTime = computed(() => results.value.reduce((sum, r) => sum + (r.elapsed_ms || 0), 0));

async function runTests() {
  running.value = true;
  results.value = [];
  try {
    const raw = await invokeWithTimeout<unknown>(
      "booksource_run_tests",
      {
        fileName: props.fileName,
        sourceDir: sourceDirOrUndef(),
        timeoutSecs: 120,
        stepFilter: stepFilter.value || null,
      },
      130_000,
    );
    if (Array.isArray(raw)) {
      results.value = raw as StepResult[];
    }
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    results.value = [{ name: "error", status: "failed", elapsed_ms: 0, error: msg }];
  } finally {
    running.value = false;
  }
}

function rowClass(status: string) {
  if (status === "passed") return "row-pass";
  if (status === "failed") return "row-fail";
  if (status === "skipped") return "row-skip";
  return "";
}
function statusBadge(status: string) {
  if (status === "passed") return "badge-pass";
  if (status === "failed") return "badge-fail";
  return "badge-skip";
}
function statusLabel(status: string) {
  const map: Record<string, string> = {
    passed: "通过",
    failed: "失败",
    skipped: "跳过",
    available: "可用(未测)",
    not_configured: "未配置",
  };
  return map[status] || status;
}
function truncate(s: string, n: number) {
  return s.length > n ? s.slice(0, n) + "…" : s;
}
</script>

<style scoped>
.source-debug-panel {
  padding: 12px;
  border: 1px solid var(--color-border, #333);
  border-radius: 8px;
  background: var(--color-bg-secondary, #1a1a1a);
}
.debug-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.debug-header h4 {
  margin: 0;
  font-size: 1rem;
}
.btn-run {
  padding: 4px 16px;
  border: none;
  border-radius: 4px;
  background: #4a90d9;
  color: #fff;
  cursor: pointer;
}
.btn-run:disabled {
  background: #555;
  cursor: not-allowed;
}
.filter-label {
  font-size: 0.85rem;
  color: var(--color-text-secondary, #999);
}
.filter-label select {
  margin-left: 4px;
  padding: 2px 8px;
  background: var(--color-bg, #222);
  color: var(--color-text, #eee);
  border: 1px solid var(--color-border, #444);
  border-radius: 3px;
}
.results-table table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
}
.results-table th,
.results-table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--color-border, #333);
  text-align: left;
}
.row-pass {
  background: rgba(0, 255, 0, 0.04);
}
.row-fail {
  background: rgba(255, 0, 0, 0.06);
}
.row-skip {
  background: rgba(255, 255, 0, 0.04);
}
.badge-pass {
  color: #4caf50;
  font-weight: 600;
}
.badge-fail {
  color: #f44336;
  font-weight: 600;
}
.badge-skip {
  color: #ff9800;
}
.preview-cell {
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.error-cell {
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #f44336;
}
.summary-row {
  margin-top: 8px;
  font-size: 0.85rem;
  color: var(--color-text-secondary, #999);
}
</style>
