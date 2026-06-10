<script setup lang="ts">
import { NButton, NCheckbox, NEmpty, NSpace, NSpin, useMessage } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import { useBackAwareDialog as useDialog } from "@/composables/useBackAwareDialog";
import {
  createBackup,
  createBackupData,
  inspectBackup,
  listAllBackupCategories,
  peekBackup,
  peekBackupData,
  restoreBackup,
  restoreBackupData,
  type BackupCategoryId,
  type BackupCategoryStat,
  type BackupInspectReport,
  type BackupPeekReport,
} from "@/composables/useBackup";
import { hasNativeTransport, isHarmonyNative, isTauri, platform } from "@/composables/useEnv";
import { useAppConfigStore, useBookshelfStore } from "@/stores";
import { base64ToBytes, bytesToBase64, readExportFile, writeExportFile } from "@/utils/exportFile";
import SettingSection from "./SettingSection.vue";

const message = useMessage();
const dialog = useDialog();
const appCfg = useAppConfigStore();
const bookshelf = useBookshelfStore();

const transportReady = hasNativeTransport;

// ── 导出端 ──────────────────────────────────────────────
const inspectLoading = ref(false);
const inspectReport = ref<BackupInspectReport | null>(null);
const exportSelected = ref<Set<BackupCategoryId>>(new Set(listAllBackupCategories()));
const exporting = ref(false);

// ── 导入端 ──────────────────────────────────────────────
const peekLoading = ref(false);
const peekReport = ref<BackupPeekReport | null>(null);
const peekJsonPath = ref("");
const peekZipBase64 = ref("");
const importSelected = ref<Set<BackupCategoryId>>(new Set());
const importing = ref(false);

const isTauriMobile = computed(() => {
  const value = platform.value.toLowerCase();
  return isTauri && (value === "android" || value === "ios");
});

function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) {
    return "0 B";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function pad(n: number, w = 2) {
  return String(n).padStart(w, "0");
}

function defaultBackupName(): string {
  const d = new Date();
  return `legado-backup-${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.json`;
}

const exportTotal = computed(() => {
  if (!inspectReport.value) {
    return { count: 0, bytes: 0 };
  }
  let count = 0;
  let bytes = 0;
  for (const cat of inspectReport.value.categories) {
    if (exportSelected.value.has(cat.id)) {
      count += cat.itemCount;
      bytes += cat.byteSize;
    }
  }
  return { count, bytes };
});

function toggleExport(id: BackupCategoryId, checked: boolean) {
  const next = new Set(exportSelected.value);
  if (checked) {
    next.add(id);
  } else {
    next.delete(id);
  }
  exportSelected.value = next;
}

function toggleImport(id: BackupCategoryId, checked: boolean) {
  const next = new Set(importSelected.value);
  if (checked) {
    next.add(id);
  } else {
    next.delete(id);
  }
  importSelected.value = next;
}

function selectAllExport(all: boolean) {
  if (!inspectReport.value) {
    return;
  }
  exportSelected.value = all ? new Set(inspectReport.value.categories.map((c) => c.id)) : new Set();
}

function selectAllImport(all: boolean) {
  if (!peekReport.value) {
    return;
  }
  importSelected.value = all
    ? new Set(peekReport.value.manifest.categories.map((c) => c.id))
    : new Set();
}

async function loadInspect() {
  if (!transportReady) {
    return;
  }
  inspectLoading.value = true;
  try {
    inspectReport.value = await inspectBackup();
    // 默认勾选所有非空类别
    const next = new Set<BackupCategoryId>();
    for (const cat of inspectReport.value.categories) {
      if (cat.itemCount > 0 || cat.byteSize > 0) {
        next.add(cat.id);
      }
    }
    if (next.size === 0) {
      // 若全部为空也允许选择，便于备份空配置
      for (const cat of inspectReport.value.categories) {
        next.add(cat.id);
      }
    }
    exportSelected.value = next;
  } catch (e) {
    message.error(`扫描备份内容失败: ${e}`);
  } finally {
    inspectLoading.value = false;
  }
}

async function chooseSavePath(name = defaultBackupName()): Promise<string | null> {
  if (isTauri) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const target = await save({
      defaultPath: name,
      filters: [{ name: "JSON 备份", extensions: ["json"] }],
    });
    return target ? String(target) : null;
  }
  if (isHarmonyNative) {
    const { invokeWithTimeout } = await import("@/composables/useInvoke");
    const target = await invokeWithTimeout<string | null>(
      "bookshelf_pick_save_path",
      { defaultName: name, filterName: "JSON 备份", filterExts: ["json"] },
      60_000,
    );
    return target ?? null;
  }
  return null;
}

async function chooseOpenPath(): Promise<string | null> {
  if (isTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const target = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "JSON 备份", extensions: ["json"] }],
    });
    if (Array.isArray(target)) {
      return target[0] ?? null;
    }
    return target ? String(target) : null;
  }
  message.warning("当前平台暂不支持选择备份文件");
  return null;
}

async function handleExport() {
  if (exporting.value) {
    return;
  }
  if (exportSelected.value.size === 0) {
    message.warning("请至少选择一个类别");
    return;
  }
  const name = defaultBackupName();
  const target = await chooseSavePath(name);
  if (!target) {
    return;
  }
  exporting.value = true;
  try {
    const selected = [...exportSelected.value];
    let res: { byteSize: number; categories: BackupCategoryStat[] };
    if (isTauriMobile.value) {
      const data = await createBackupData(name, selected);
      await writeExportFile(target, { bytes: base64ToBytes(data.base64) });
      res = data;
    } else {
      res = await createBackup(target, selected);
    }
    const items = res.categories.reduce((s, c) => s + c.itemCount, 0);
    message.success(`备份完成（${items} 项 · ${formatBytes(res.byteSize)}）`);
    // 刷新一次统计
    void loadInspect();
  } catch (e) {
    message.error(`导出失败: ${e}`);
  } finally {
    exporting.value = false;
  }
}

async function handlePickAndPeek() {
  if (peekLoading.value) {
    return;
  }
  const path = await chooseOpenPath();
  if (!path) {
    return;
  }
  peekLoading.value = true;
  try {
    let report: BackupPeekReport;
    if (isTauriMobile.value) {
      const bytes = await readExportFile(path);
      peekZipBase64.value = bytesToBase64(bytes);
      report = await peekBackupData(peekZipBase64.value);
    } else {
      peekZipBase64.value = "";
      report = await peekBackup(path);
    }
    peekReport.value = report;
    peekJsonPath.value = path;
    importSelected.value = new Set(report.manifest.categories.map((c) => c.id));
  } catch (e) {
    peekReport.value = null;
    peekJsonPath.value = "";
    peekZipBase64.value = "";
    message.error(`读取备份失败: ${e}`);
  } finally {
    peekLoading.value = false;
  }
}

function confirmRestore(): Promise<boolean> {
  return new Promise((resolve) => {
    dialog.warning({
      title: "确认还原",
      content:
        "还原会按所选类别合并写入到当前数据，相同条目将被覆盖（不会删除未在备份中的条目）。是否继续？",
      positiveText: "继续还原",
      negativeText: "取消",
      onPositiveClick: () => resolve(true),
      onNegativeClick: () => resolve(false),
      onClose: () => resolve(false),
      onMaskClick: () => resolve(false),
    });
  });
}

async function handleImport() {
  if (importing.value) {
    return;
  }
  if (!peekJsonPath.value || !peekReport.value) {
    message.warning("请先选择备份文件");
    return;
  }
  if (importSelected.value.size === 0) {
    message.warning("请至少选择一个类别");
    return;
  }
  const ok = await confirmRestore();
  if (!ok) {
    return;
  }
  importing.value = true;
  try {
    const selected = [...importSelected.value];
    const res = isTauriMobile.value
      ? await restoreBackupData(peekZipBase64.value, selected)
      : await restoreBackup(peekJsonPath.value, selected);
    const restoredCount = res.restored.reduce((s, c) => s + c.itemCount, 0);
    message.success(`已还原 ${res.restored.length} 类 / ${restoredCount} 项`);
    if (res.skipped.length > 0) {
      message.info(`已跳过未识别类别: ${res.skipped.join(", ")}`);
    }
    // 同步刷新前端 store
    try {
      await appCfg.loadConfig();
    } catch {
      /* ignore */
    }
    try {
      await bookshelf.loadBooks();
    } catch {
      /* ignore */
    }
  } catch (e) {
    message.error(`还原失败: ${e}`);
  } finally {
    importing.value = false;
  }
}

function describeCategory(cat: BackupCategoryStat): string {
  const parts = [cat.description];
  parts.push(`${cat.itemCount} 项`);
  parts.push(formatBytes(cat.byteSize));
  return parts.join(" · ");
}

onMounted(() => {
  void loadInspect();
});
</script>

<template>
  <div class="settings-backup">
    <SettingSection title="导出备份">
      <p class="hint">勾选要导出的内容；导出为单个 JSON 文件，可在新设备上选择性还原。</p>

      <NSpin :show="inspectLoading">
        <div v-if="!transportReady" class="empty">
          <NEmpty description="当前运行环境不支持本机备份。" />
        </div>
        <div v-else-if="inspectReport" class="cat-list">
          <div v-for="cat in inspectReport.categories" :key="cat.id" class="cat-row">
            <NCheckbox
              :checked="exportSelected.has(cat.id)"
              @update:checked="(v: boolean) => toggleExport(cat.id, v)"
            >
              <div class="cat-main">
                <div class="cat-title">{{ cat.label }}</div>
                <div class="cat-sub">{{ describeCategory(cat) }}</div>
              </div>
            </NCheckbox>
          </div>
        </div>
      </NSpin>

      <NSpace class="actions">
        <NButton size="small" @click="selectAllExport(true)">全选</NButton>
        <NButton size="small" @click="selectAllExport(false)">全不选</NButton>
        <NButton size="small" :loading="inspectLoading" @click="loadInspect"> 重新扫描 </NButton>
        <NButton
          type="primary"
          :loading="exporting"
          :disabled="!transportReady || exportSelected.size === 0"
          @click="handleExport"
        >
          导出选中 ({{ exportTotal.count }} 项 / {{ formatBytes(exportTotal.bytes) }})
        </NButton>
      </NSpace>
    </SettingSection>

    <SettingSection title="从备份还原">
      <p class="hint">
        选择 JSON 备份后，将列出备份中包含的类别，可逐项勾选还原。还原为合并写入，不会删除现有数据。
      </p>

      <NSpace class="actions">
        <NButton :loading="peekLoading" @click="handlePickAndPeek"> 选择备份文件… </NButton>
      </NSpace>

      <div v-if="peekReport" class="peek">
        <div class="peek-meta">
          来源: {{ peekJsonPath }}<br />
          创建时间:
          {{ new Date(peekReport.manifest.createdAt).toLocaleString() }} · 来源版本:
          {{ peekReport.manifest.appVersion }}
        </div>
        <div class="cat-list">
          <div v-for="cat in peekReport.manifest.categories" :key="cat.id" class="cat-row">
            <NCheckbox
              :checked="importSelected.has(cat.id)"
              @update:checked="(v: boolean) => toggleImport(cat.id, v)"
            >
              <div class="cat-main">
                <div class="cat-title">{{ cat.label }}</div>
                <div class="cat-sub">{{ describeCategory(cat) }}</div>
              </div>
            </NCheckbox>
          </div>
        </div>
        <div v-if="peekReport.unknownCategories.length" class="warn">
          备份中含有当前版本无法识别的类别（将被忽略）:
          {{ peekReport.unknownCategories.join(", ") }}
        </div>
        <NSpace class="actions">
          <NButton size="small" @click="selectAllImport(true)">全选</NButton>
          <NButton size="small" @click="selectAllImport(false)">全不选</NButton>
          <NButton
            type="primary"
            :loading="importing"
            :disabled="importSelected.size === 0"
            @click="handleImport"
          >
            还原选中
          </NButton>
        </NSpace>
      </div>
    </SettingSection>
  </div>
</template>

<style scoped>
.settings-backup {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.hint {
  margin: 0 0 var(--space-2);
  color: var(--color-text-2);
  font-size: var(--fs-13);
}

.cat-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2) 0;
}

.cat-row {
  padding: 6px 0;
}

.cat-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.cat-title {
  font-weight: var(--fw-bold);
  color: var(--color-text);
}

.cat-sub {
  font-size: var(--fs-12);
  color: var(--color-text-2);
}

.actions {
  margin-top: var(--space-2);
}

.peek {
  margin-top: var(--space-3);
  padding: var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.peek-meta {
  font-size: var(--fs-12);
  color: var(--color-text-2);
  line-height: 1.6;
}

.warn {
  font-size: var(--fs-12);
  color: var(--color-warning, #d97706);
}

.empty {
  padding: var(--space-4) 0;
}
</style>
