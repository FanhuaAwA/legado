<!-- AiSourceTab — AI 写书源工作台，管理 AI 配置、会话草稿、生成日志与调试面板。 -->
<script setup lang="ts">
import {
  Bot,
  Copy,
  FileCode2,
  History,
  Play,
  Plus,
  RotateCcw,
  Save,
  Settings,
  Sparkles,
  Square,
} from "lucide-vue-next";
import { useMessage } from "naive-ui";
import { storeToRefs } from "pinia";
import { ref, watch, nextTick, computed, onMounted } from "vue";
import AppDialog from "@/components/base/AppDialog.vue";
import { useBackAwareDialog as useDialog } from "@/composables/useBackAwareDialog";
import { useAiSessionsStore } from "@/stores";
import {
  ACTIVITY_LABEL,
  getActivityClass,
  formatTime,
  formatDate,
  getDisplayContent,
  truncateResult,
  sessionStatusLabel,
  sessionStatusType,
} from "@/utils/aiActivityUtils";
import {
  useAiAgent,
  loadAiConfig,
  ensureAiConfigLoaded,
  saveAiConfig,
  type AiConfig,
  type AgentActivity,
} from "../../composables/useAiAgent";
import {
  readBookSource,
  saveBookSource,
  type BookSourceMeta,
} from "../../composables/useBookSource";
import { invokeWithTimeout } from "../../composables/useInvoke";
import AiTestPanel from "./AiTestPanel.vue";

const props = defineProps<{
  sources: BookSourceMeta[];
}>();

const emit = defineEmits<{ reload: [] }>();

const message = useMessage();
const dialog = useDialog();
const { state, runAiAgent, stopAiAgent, clearAgentState } = useAiAgent();
const aiSessionsStore = useAiSessionsStore();
const { sessions, currentSession } = storeToRefs(aiSessionsStore);
const { createSession, selectSession, updateSession, deleteSession } = aiSessionsStore;

// ── AI 配置 ───────────────────────────────────────────────────────────────
const config = ref<AiConfig>(loadAiConfig());
const settingsDialogShow = ref(false);

interface AiProviderPreset {
  key: string;
  label: string;
  apiUrl: string;
  model: string;
  apiMode: AiConfig["apiMode"];
  requestTransport: AiConfig["requestTransport"];
  temperature?: number;
}

const AI_PROVIDER_PRESETS: AiProviderPreset[] = [
  {
    key: "deepseek-chat",
    label: "DeepSeek V3",
    apiUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
    apiMode: "chat",
    requestTransport: "backend",
    temperature: 0.2,
  },
  {
    key: "deepseek-reasoner",
    label: "DeepSeek R1",
    apiUrl: "https://api.deepseek.com/v1",
    model: "deepseek-reasoner",
    apiMode: "chat",
    requestTransport: "backend",
    temperature: 0.2,
  },
  {
    key: "openai-gpt4o",
    label: "OpenAI GPT-4o",
    apiUrl: "https://api.openai.com/v1",
    model: "gpt-4o",
    apiMode: "chat",
    requestTransport: "backend",
  },
  {
    key: "openai-responses",
    label: "OpenAI Responses",
    apiUrl: "https://api.openai.com/v1",
    model: "gpt-4.1",
    apiMode: "responses",
    requestTransport: "backend",
  },
];

const activeProviderPresetKey = computed(() => {
  const apiUrl = config.value.apiUrl.trim().replace(/\/$/, "");
  const model = config.value.model.trim();
  return (
    AI_PROVIDER_PRESETS.find(
      (preset) =>
        preset.apiUrl.replace(/\/$/, "") === apiUrl &&
        preset.model === model &&
        preset.apiMode === config.value.apiMode,
    )?.key ?? ""
  );
});

function onConfigChange() {
  saveAiConfig(config.value);
}

function applyProviderPreset(preset: AiProviderPreset) {
  config.value = {
    ...config.value,
    apiUrl: preset.apiUrl,
    model: preset.model,
    apiMode: preset.apiMode,
    requestTransport: preset.requestTransport,
    temperature: preset.temperature,
  };
  saveAiConfig(config.value);
}

onMounted(async () => {
  config.value = await ensureAiConfigLoaded();
});

// ── 侧边栏 ────────────────────────────────────────────────────────────────
const sidebarCollapsed = ref(false);
const sourceSearch = ref("");

// ── 模式选择 ──────────────────────────────────────────────────────────────
/** 工作模式：new = 从零创建，modify = 基于已有书源修改 */
const workMode = ref<"new" | "modify">("new");

/** 修改模式下选中的书源文件名 */
const selectedBaseSource = ref("");

/** 从 sources prop 获取选项 */
const sourceOptions = computed(() =>
  props.sources.map((s) => ({
    label: s.name || s.fileName,
    value: s.fileName,
  })),
);

const filteredSources = computed(() => {
  const keyword = sourceSearch.value.trim().toLowerCase();
  if (!keyword) {
    return props.sources;
  }
  return props.sources.filter((source) => {
    const haystack = `${source.name} ${source.fileName}`.toLowerCase();
    return haystack.includes(keyword);
  });
});

// ── 用户输入 ──────────────────────────────────────────────────────────────
const userPrompt = ref("");
const NEW_PLACEHOLDER =
  '请描述目标网站，例如：\n为 https://www.biquge.com 创建小说书源，名叫"笔趣阁"，实现发现、搜索、详情、目录、正文。\n也可以说明：漫画/API/CF 盾/登录 Cookie/加密签名/浏览器嗅探/m3u8/Android JSON 旧书源转换。';
const MODIFY_PLACEHOLDER =
  "请描述要做的修改，例如：\n修复搜索为空、目录倒序、正文为空、图片 403、CF 验证、AES 解密、签名错误、漫画 processImage 或视频 m3u8 嗅探。";

type PromptTemplateMode = "new" | "modify" | "both";

interface PromptTemplate {
  label: string;
  mode: PromptTemplateMode;
  text: string;
}

const PROMPT_TEMPLATES: PromptTemplate[] = [
  {
    label: "小说站",
    mode: "new",
    text: '为 https://example.com 创建小说书源，名叫"站点名"，实现发现、搜索、详情、目录、正文。请逐模块探测真实页面并测试。',
  },
  {
    label: "漫画/API",
    mode: "new",
    text: '为 https://example.com 创建漫画书源，名叫"站点名"。重点处理 API、图片列表、Referer 防盗链、加密签名和 processImage 图片还原。',
  },
  {
    label: "CF/登录",
    mode: "new",
    text: "为 https://example.com 创建书源，站点可能有 CF 盾、Turnstile、验证码或登录 Cookie。请先普通 HTTP 探测，失败再用浏览器探测并同步 Cookie。",
  },
  {
    label: "媒体嗅探",
    mode: "new",
    text: "为 https://example.com 创建视频或音频书源，保留线路 group，嗅探 m3u8/mp4/mp3 播放地址，并测试章节内容返回值。",
  },
  {
    label: "旧源转换",
    mode: "new",
    text: "把下面的 Android JSON 旧书源转换为 Legado Tauri 书源 JS。请转换 search、explore、bookInfo、chapterList、chapterContent，并逐项测试。\n\n",
  },
  {
    label: "搜索为空",
    mode: "modify",
    text: "修复当前书源 search 返回为空的问题。请先用 eval_in_source 探测真实搜索页或搜索 API，只修改相关函数并重新测试。",
  },
  {
    label: "目录/正文",
    mode: "modify",
    text: "修复当前书源目录为空、目录倒序或正文为空的问题。请检查 tocUrl、章节选择器、正文容器、分页和广告清理。",
  },
  {
    label: "图片/播放",
    mode: "modify",
    text: "修复当前书源图片 403、漫画图片缺页、processImage 还原、m3u8 嗅探或播放地址不可用的问题。",
  },
];

const ENTITY_CHIPS = [
  "CF 盾",
  "API JSON",
  "AES 解密",
  "签名 token",
  "浏览器嗅探",
  "m3u8",
  "图片防盗链",
  "processImage",
  "GBK 搜索",
  "Android JSON",
  "VIP 章节",
  "目录分页",
];

const visiblePromptTemplates = computed(() =>
  PROMPT_TEMPLATES.filter((tpl) => tpl.mode === "both" || tpl.mode === workMode.value),
);

const promptCharCount = computed(() => userPrompt.value.trim().length);
const configReady = computed(() => !!config.value.apiUrl.trim() && !!config.value.model.trim());
const canStartAgent = computed(
  () => !state.isRunning && configReady.value && promptCharCount.value > 0,
);

function applyPromptTemplate(template: PromptTemplate) {
  userPrompt.value = template.text;
}

function appendEntityToken(token: string) {
  const current = userPrompt.value.trim();
  if (!current) {
    userPrompt.value = token;
    return;
  }
  if (current.includes(token)) {
    return;
  }
  userPrompt.value = `${current}；${token}`;
}

function clearPrompt() {
  userPrompt.value = "";
}

// ── 内容标签页 ────────────────────────────────────────────────────────────
const activePane = ref<"source" | "test" | "history">("source");

// 日志自动滚动
const logListRef = ref<HTMLElement | null>(null);
watch(
  () => state.activities.length,
  () => {
    nextTick(() => {
      if (logListRef.value) {
        logListRef.value.scrollTop = logListRef.value.scrollHeight;
      }
    });
  },
);

// ── 会话名称编辑 ──────────────────────────────────────────────────────────
const editingName = ref(false);
const nameInputRef = ref("");
function startEditName() {
  if (!currentSession.value) {
    return;
  }
  nameInputRef.value = currentSession.value.name;
  editingName.value = true;
  nextTick(() => {
    const el = document.getElementById("session-name-input");
    if (el) {
      (el as HTMLInputElement).focus();
    }
  });
}
function confirmEditName() {
  if (!currentSession.value) {
    return;
  }
  const trimmed = nameInputRef.value.trim();
  if (trimmed) {
    updateSession(currentSession.value.id, { name: trimmed });
  }
  editingName.value = false;
}

// ── 切换会话（同步 state 到选中会话）─────────────────────────────────────
function onSelectSession(id: string) {
  selectSession(id);
  const session = sessions.value.find((s) => s.id === id);
  if (session) {
    state.activities = [...session.activities];
    state.testResults = [...session.testResults];
    state.currentFileName = session.currentFileName;
    state.currentSourceCode = session.currentSourceCode;
    activePane.value = session.currentSourceCode ? "source" : "test";
  }
}

// ── 新建会话 ──────────────────────────────────────────────────────────────
async function onNewSession() {
  if (workMode.value === "modify" && selectedBaseSource.value) {
    await createModifySession();
  } else {
    const session = createSession("new");
    clearAgentState();
    activePane.value = "source";
    message.success(`已创建新草稿：${session.name}`);
  }
}

async function createModifySession() {
  const fileName = selectedBaseSource.value;
  if (!fileName) {
    message.warning("请先选择要修改的书源");
    return;
  }
  try {
    const code = await readBookSource(fileName);
    const session = createSession("modify", { fileName, code });
    state.activities = [];
    state.testResults = [];
    state.currentFileName = fileName;
    state.currentSourceCode = code;
    activePane.value = "source";
    message.success(`已载入《${fileName.replace(/\.js$/, "")}》作为基础版本`);
    return session;
  } catch (e: unknown) {
    message.error(`读取书源失败：${e instanceof Error ? e.message : String(e)}`);
  }
}

// ── 启动 / 继续 Agent ──────────────────────────────────────────────────────
async function startAgent(continueConversation = false) {
  const prompt = userPrompt.value.trim();
  if (!prompt) {
    message.warning("请先输入任务描述");
    return;
  }
  if (!config.value.apiUrl.trim()) {
    message.warning("请填写 API 地址");
    return;
  }
  if (!config.value.model.trim()) {
    message.warning("请填写模型名称");
    return;
  }

  // 确保有当前会话
  if (!currentSession.value) {
    if (workMode.value === "modify" && selectedBaseSource.value) {
      await createModifySession();
      if (!currentSession.value) {
        return;
      }
    } else {
      createSession("new");
    }
  }

  const sessionId = currentSession.value?.id ?? "";
  activePane.value = "source";

  try {
    await runAiAgent(config.value, prompt, { sessionId, continueConversation });
    emit("reload");
    if (state.currentFileName) {
      message.success(`书源 "${state.currentFileName}" 已保存`);
    }
  } catch (e: unknown) {
    message.error(`错误：${e instanceof Error ? e.message : String(e)}`);
  }

  userPrompt.value = "";
}

// ── 保存为正式书源 ────────────────────────────────────────────────────────
async function saveAsFormal() {
  const session = currentSession.value;
  const code = state.currentSourceCode || session?.currentSourceCode;
  const fileName = state.currentFileName || session?.currentFileName;
  if (!code || !fileName) {
    message.warning("当前草稿没有可保存的代码");
    return;
  }
  try {
    await saveBookSource(fileName, code);
    // 正式保存后清理草稿文件
    await invokeWithTimeout("booksource_delete_draft", { fileName }, 5_000).catch(() => {});
    if (session) {
      updateSession(session.id, { status: "saved" });
    }
    emit("reload");
    message.success(`已保存为正式书源：${fileName}`);
  } catch (e: unknown) {
    message.error(`保存失败：${e instanceof Error ? e.message : String(e)}`);
  }
}

/** 覆盖原书源（仅修改模式可用） */
async function overwriteOriginal() {
  const session = currentSession.value;
  if (!session || session.mode !== "modify" || !session.baseSourceFileName) {
    return;
  }
  const code = state.currentSourceCode || session.currentSourceCode;
  if (!code) {
    message.warning("当前草稿没有可保存的代码");
    return;
  }
  dialog.warning({
    title: "覆盖原书源",
    content: `确定要用当前草稿覆盖《${session.baseSourceFileName.replace(/\.js$/, "")}》吗？此操作不可撤销。`,
    positiveText: "覆盖",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await saveBookSource(session.baseSourceFileName ?? "", code);
        // 正式保存后清理草稿文件（草稿文件名可能与原书源不同）
        const draftFileName = state.currentFileName || session.currentFileName;
        if (draftFileName) {
          await invokeWithTimeout(
            "booksource_delete_draft",
            { fileName: draftFileName },
            5_000,
          ).catch(() => {});
        }
        updateSession(session.id, { status: "saved" });
        emit("reload");
        message.success(`已覆盖原书源：${session.baseSourceFileName}`);
      } catch (e: unknown) {
        message.error(`保存失败：${e instanceof Error ? e.message : String(e)}`);
      }
    },
  });
}

// ── 版本回滚 ──────────────────────────────────────────────────────────────
function rollbackToDraft(version: number) {
  const session = currentSession.value;
  if (!session) {
    return;
  }
  const draft = session.drafts.find((d) => d.version === version);
  if (!draft) {
    return;
  }
  state.currentFileName = draft.fileName;
  state.currentSourceCode = draft.content;
  updateSession(session.id, {
    currentFileName: draft.fileName,
    currentSourceCode: draft.content,
  });
  message.success(`已回滚到版本 v${version}`);
  activePane.value = "source";
}

async function openInstalledSource(fileName: string) {
  if (state.isRunning) {
    return;
  }
  workMode.value = "modify";
  selectedBaseSource.value = fileName;
  await createModifySession();
}

// ── 删除会话 ──────────────────────────────────────────────────────────────
function onDeleteSession(id: string) {
  dialog.warning({
    title: "删除草稿",
    content: "删除后无法恢复，确定继续吗？",
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: () => {
      // 删除会话关联的草稿文件（安静失败，文件可能已手动保存为正式书源）
      const session = sessions.value.find((s) => s.id === id);
      const draftFileName = session?.currentFileName;
      if (draftFileName) {
        invokeWithTimeout("booksource_delete_draft", { fileName: draftFileName }, 5_000).catch(
          () => {},
        );
      }
      deleteSession(id);
      if (!currentSession.value) {
        clearAgentState();
      }
    },
  });
}

// ── 复制代码 ──────────────────────────────────────────────────────────────
async function copySourceCode() {
  const code = state.currentSourceCode || currentSession.value?.currentSourceCode;
  if (!code) {
    return;
  }
  try {
    await navigator.clipboard.writeText(code);
    message.success("已复制到剪贴板");
  } catch {
    message.error("复制失败");
  }
}

const hasDraftCode = computed(
  () => !!(state.currentSourceCode || currentSession.value?.currentSourceCode),
);

const hasConversationHistory = computed(
  () => (currentSession.value?.conversationHistory?.length ?? 0) > 0,
);

const displayActivities = computed<AgentActivity[]>(() => {
  if (state.isRunning) {
    return state.activities;
  }
  return currentSession.value?.activities ?? state.activities;
});

const displaySourceCode = computed(
  () => state.currentSourceCode || currentSession.value?.currentSourceCode || "",
);
const displayFileName = computed(
  () => state.currentFileName || currentSession.value?.currentFileName || "",
);
const displayTestResults = computed(() =>
  state.isRunning ? state.testResults : (currentSession.value?.testResults ?? state.testResults),
);
const okTestCount = computed(
  () => displayTestResults.value.filter((r) => r.status === "ok").length,
);
const errorTestCount = computed(
  () => displayTestResults.value.filter((r) => r.status === "error").length,
);
const toolCallCount = computed(
  () => displayActivities.value.filter((a) => a.type === "tool_call").length,
);
const sourceLineCount = computed(() =>
  displaySourceCode.value ? displaySourceCode.value.split(/\r?\n/).length : 0,
);

function isUserActivity(activity: AgentActivity): boolean {
  return (
    activity.type === "info" &&
    (activity.content.startsWith("开始任务：") || activity.content.startsWith("继续对话："))
  );
}

function isSystemActivity(activity: AgentActivity): boolean {
  return activity.type === "info" && !isUserActivity(activity);
}

function getActivityLayoutClass(activity: AgentActivity): string {
  if (isUserActivity(activity)) {
    return "log-item--user";
  }
  if (isSystemActivity(activity)) {
    return "log-item--system";
  }
  return "log-item--assistant";
}

function getChatContent(activity: AgentActivity): string {
  const content = getDisplayContent(activity);
  if (activity.type !== "info") {
    return content;
  }
  return content.replace(/^(开始任务|继续对话)：/, "");
}

function getToolStatus(activity: AgentActivity): string {
  if (activity.result) {
    return "已完成";
  }
  return state.isRunning ? "执行中" : "无返回";
}
</script>

<template>
  <div class="ai-workbench">
    <header class="workbench-topbar">
      <div class="topbar-title">
        <span class="topbar-title-main">AI 写书源</span>
        <span class="topbar-title-sub">{{
          displayFileName || currentSession?.name || "未选择草稿"
        }}</span>
      </div>
      <div class="work-mode-toggle" role="group" aria-label="书源创作模式">
        <button
          type="button"
          class="work-mode-button"
          :class="{ 'work-mode-button--active': workMode === 'new' }"
          :aria-pressed="workMode === 'new'"
          @click="workMode = 'new'"
        >
          新建书源
        </button>
        <button
          type="button"
          class="work-mode-button"
          :class="{ 'work-mode-button--active': workMode === 'modify' }"
          :aria-pressed="workMode === 'modify'"
          @click="workMode = 'modify'"
        >
          修改已有书源
        </button>
      </div>
      <div class="topbar-stats">
        <span :class="{ muted: !configReady }">配置 {{ configReady ? "就绪" : "待配置" }}</span>
        <span>测试 {{ okTestCount }} / {{ errorTestCount }}</span>
        <span>工具 {{ toolCallCount }}</span>
        <span>代码 {{ sourceLineCount }} 行</span>
      </div>
      <div class="topbar-actions">
        <n-tag v-if="config.model" size="small" round>{{ config.model }}</n-tag>
        <n-button size="small" quaternary @click="settingsDialogShow = true">
          <template #icon>
            <n-icon><Settings /></n-icon>
          </template>
          设置
        </n-button>
        <n-button v-if="state.isRunning" size="small" type="error" @click="stopAiAgent()">
          <template #icon>
            <n-icon><Square /></n-icon>
          </template>
          停止
        </n-button>
      </div>
    </header>

    <div class="workbench-grid" :class="{ 'workbench-grid--sidebar-collapsed': sidebarCollapsed }">
      <aside class="ai-sidebar" :class="{ 'ai-sidebar--collapsed': sidebarCollapsed }">
        <div class="sidebar-header">
          <span v-if="!sidebarCollapsed" class="sidebar-title">Explorer</span>
          <n-button
            size="tiny"
            quaternary
            class="sidebar-toggle"
            :title="sidebarCollapsed ? '展开' : '收起'"
            @click="sidebarCollapsed = !sidebarCollapsed"
          >
            {{ sidebarCollapsed ? "›" : "‹" }}
          </n-button>
        </div>

        <template v-if="!sidebarCollapsed">
          <section class="sidebar-section">
            <div class="sidebar-section-hd">
              <span>工作草稿</span>
              <n-button size="tiny" type="primary" @click="onNewSession">
                <template #icon>
                  <n-icon><Plus /></n-icon>
                </template>
                新建
              </n-button>
            </div>
            <div class="session-list">
              <div
                v-for="s in sessions"
                :key="s.id"
                class="session-item"
                :class="{ 'session-item--active': s.id === currentSession?.id }"
                @click="onSelectSession(s.id)"
              >
                <div class="session-item-main">
                  <div class="session-item-name">{{ s.name }}</div>
                  <div class="session-item-meta">
                    <n-tag
                      :type="sessionStatusType(s)"
                      size="tiny"
                      round
                      style="font-size: 10px; padding: 0 5px; height: 16px"
                    >
                      {{ sessionStatusLabel(s) }}
                    </n-tag>
                    <span class="session-item-time">{{ formatDate(s.updatedAt) }}</span>
                  </div>
                </div>
                <n-button
                  size="tiny"
                  quaternary
                  class="session-delete-btn"
                  title="删除草稿"
                  @click.stop="onDeleteSession(s.id)"
                >
                  ✕
                </n-button>
              </div>

              <div v-if="sessions.length === 0" class="session-empty">
                <p>还没有草稿</p>
                <p>点击新建开始</p>
              </div>
            </div>
          </section>

          <section class="sidebar-section sidebar-section--sources">
            <div class="sidebar-section-hd">
              <span>已安装书源</span>
              <span class="sidebar-count">{{ filteredSources.length }}</span>
            </div>
            <n-input
              v-model:value="sourceSearch"
              size="tiny"
              placeholder="搜索书源"
              clearable
              class="source-search"
            />
            <div class="source-list">
              <button
                v-for="source in filteredSources"
                :key="source.fileName"
                type="button"
                class="source-item"
                :class="{ 'source-item--active': source.fileName === selectedBaseSource }"
                :disabled="state.isRunning"
                @click="openInstalledSource(source.fileName)"
              >
                <span class="source-item-name">{{ source.name || source.fileName }}</span>
                <span class="source-item-file">{{ source.fileName }}</span>
              </button>
              <div v-if="filteredSources.length === 0" class="session-empty">
                <p>没有匹配书源</p>
              </div>
            </div>
          </section>
        </template>

        <template v-else>
          <div class="sidebar-collapsed-sessions">
            <div
              v-for="s in sessions"
              :key="s.id"
              class="session-dot"
              :class="{ 'session-dot--active': s.id === currentSession?.id }"
              :title="s.name"
              @click="onSelectSession(s.id)"
            />
          </div>
        </template>
      </aside>

      <main class="workspace-panel">
        <div class="workspace-titlebar">
          <div class="draft-info">
            <input
              v-if="editingName"
              id="session-name-input"
              v-model="nameInputRef"
              class="draft-name-input"
              @blur="confirmEditName"
              @keydown.enter="confirmEditName"
              @keydown.esc="editingName = false"
            />
            <button
              v-else-if="currentSession"
              class="draft-name"
              title="点击编辑名称"
              @click="startEditName"
            >
              {{ currentSession.name }}
            </button>
            <span v-else class="draft-name draft-name--placeholder">未选择草稿</span>
            <n-tag
              v-if="currentSession?.mode === 'modify' && currentSession.baseSourceFileName"
              size="tiny"
              type="info"
              round
            >
              基于《{{ currentSession.baseSourceFileName.replace(/\.js$/, "") }}》
            </n-tag>
            <n-tag
              v-if="currentSession"
              :type="sessionStatusType(currentSession)"
              size="tiny"
              round
            >
              {{ sessionStatusLabel(currentSession) }}
            </n-tag>
          </div>
          <div class="draft-actions">
            <n-button
              size="small"
              type="primary"
              :disabled="!hasDraftCode || state.isRunning"
              @click="saveAsFormal"
            >
              <template #icon>
                <n-icon><Save /></n-icon>
              </template>
              保存
            </n-button>
            <n-button
              v-if="currentSession?.mode === 'modify'"
              size="small"
              type="warning"
              :disabled="!hasDraftCode || state.isRunning"
              @click="overwriteOriginal"
            >
              覆盖
            </n-button>
            <n-button size="small" quaternary :disabled="!hasDraftCode" @click="copySourceCode">
              <template #icon>
                <n-icon><Copy /></n-icon>
              </template>
              复制
            </n-button>
          </div>
        </div>

        <div class="pane-tabs">
          <button
            class="pane-tab"
            :class="{ 'pane-tab--active': activePane === 'source' }"
            @click="activePane = 'source'"
          >
            当前草稿{{ displayFileName ? ` (${displayFileName})` : "" }}
          </button>
          <button
            class="pane-tab"
            :class="{ 'pane-tab--active': activePane === 'test' }"
            @click="activePane = 'test'"
          >
            调试测试{{ displayTestResults.length ? ` (${displayTestResults.length})` : "" }}
          </button>
          <button
            class="pane-tab"
            :class="{ 'pane-tab--active': activePane === 'history' }"
            @click="activePane = 'history'"
          >
            版本历史{{ currentSession?.drafts.length ? ` (${currentSession.drafts.length})` : "" }}
          </button>
        </div>

        <div class="workspace-body">
          <div v-show="activePane === 'source'" class="source-panel">
            <div v-if="!displaySourceCode" class="empty-hint">
              <n-icon class="empty-icon"><FileCode2 /></n-icon>
              <p>AI 尚未创建书源代码</p>
            </div>
            <template v-else>
              <div class="source-toolbar">
                <span class="source-name">{{ displayFileName }}</span>
                <div class="source-toolbar-actions">
                  <n-button size="tiny" quaternary @click="copySourceCode">
                    <template #icon>
                      <n-icon><Copy /></n-icon>
                    </template>
                    复制代码
                  </n-button>
                  <n-button
                    size="tiny"
                    type="primary"
                    :disabled="state.isRunning"
                    @click="saveAsFormal"
                  >
                    <template #icon>
                      <n-icon><Save /></n-icon>
                    </template>
                    保存为正式书源
                  </n-button>
                </div>
              </div>
              <pre class="source-code">{{ displaySourceCode }}</pre>
            </template>
          </div>

          <AiTestPanel
            v-show="activePane === 'test'"
            :file-name="displayFileName"
            :ai-test-results="displayTestResults"
          />

          <div v-show="activePane === 'history'" class="history-panel">
            <div v-if="!currentSession || currentSession.drafts.length === 0" class="empty-hint">
              <n-icon class="empty-icon"><History /></n-icon>
              <p>暂无版本快照</p>
              <p style="font-size: 12px; color: var(--color-text-muted)">
                每次 AI 保存书源时自动创建快照
              </p>
            </div>
            <div v-else class="history-list">
              <div
                v-for="draft in [...(currentSession?.drafts ?? [])].reverse()"
                :key="draft.version"
                class="history-item"
                :class="{
                  'history-item--current':
                    draft.fileName === displayFileName && draft.content === displaySourceCode,
                }"
              >
                <div class="history-item-hd">
                  <span class="history-version">v{{ draft.version }}</span>
                  <span class="history-filename">{{ draft.fileName }}</span>
                  <span class="history-time">{{ formatDate(draft.createdAt) }}</span>
                  <span class="history-size">{{ Math.ceil(draft.content.length / 1024) }} KB</span>
                </div>
                <div class="history-item-actions">
                  <n-tag
                    v-if="draft.testResults.some((r) => r.status === 'ok')"
                    size="tiny"
                    type="success"
                    round
                  >
                    {{ draft.testResults.filter((r) => r.status === "ok").length }} 项通过
                  </n-tag>
                  <n-tag
                    v-if="draft.testResults.some((r) => r.status === 'error')"
                    size="tiny"
                    type="error"
                    round
                  >
                    {{ draft.testResults.filter((r) => r.status === "error").length }} 项失败
                  </n-tag>
                  <n-button
                    size="tiny"
                    quaternary
                    :disabled="
                      draft.fileName === displayFileName && draft.content === displaySourceCode
                    "
                    @click="rollbackToDraft(draft.version)"
                  >
                    回滚到此版本
                  </n-button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </main>

      <aside class="chat-panel">
        <div class="chat-header">
          <div>
            <span class="chat-title">AI 对话</span>
            <span v-if="state.isRunning" class="chat-live">流式生成中</span>
          </div>
          <n-button
            v-if="state.isRunning"
            size="tiny"
            type="error"
            quaternary
            @click="stopAiAgent()"
          >
            <template #icon>
              <n-icon><Square /></n-icon>
            </template>
            停止
          </n-button>
        </div>

        <div ref="logListRef" class="log-list">
          <div v-if="displayActivities.length === 0" class="empty-hint">
            <n-icon class="empty-icon"><Bot /></n-icon>
            <p>
              {{
                currentSession
                  ? "描述目标网站或选择左侧书源开始"
                  : "选择一个草稿继续工作，或点击新建开始"
              }}
            </p>
          </div>
          <div
            v-for="activity in displayActivities"
            :key="activity.id"
            class="log-item"
            :class="[getActivityClass(activity.type), getActivityLayoutClass(activity)]"
          >
            <template v-if="activity.type === 'tool_call'">
              <details class="tool-call-card">
                <summary class="tool-call-summary">
                  <span class="log-time">{{ formatTime(activity.timestamp) }}</span>
                  <span class="log-badge">{{ ACTIVITY_LABEL[activity.type] }}</span>
                  <span class="log-tool">{{ activity.toolName || "tool" }}</span>
                  <span class="tool-call-status">{{ getToolStatus(activity) }}</span>
                  <span class="tool-call-expand">详情</span>
                </summary>
                <div v-if="activity.args" class="log-section">
                  <div class="log-section-label">参数</div>
                  <pre class="log-pre log-pre--args">{{ activity.args }}</pre>
                </div>
                <div v-if="activity.result" class="log-section">
                  <div class="log-section-label">返回值</div>
                  <pre class="log-pre log-pre--result">{{ truncateResult(activity.result) }}</pre>
                </div>
              </details>
            </template>
            <template v-else>
              <div class="log-bubble">
                <div class="log-hd">
                  <span class="log-time">{{ formatTime(activity.timestamp) }}</span>
                  <span class="log-badge">{{
                    isUserActivity(activity) ? "用户" : ACTIVITY_LABEL[activity.type]
                  }}</span>
                  <span
                    v-if="
                      activity.type === 'thinking' &&
                      state.isRunning &&
                      activity.id === state.activeThinkingId
                    "
                    class="log-spinner"
                  />
                </div>
                <pre v-if="activity.content" class="log-pre">{{ getChatContent(activity) }}</pre>
              </div>
            </template>
          </div>
        </div>

        <div class="chat-composer">
          <div v-if="workMode === 'modify'" class="source-selector-row">
            <n-select
              v-model:value="selectedBaseSource"
              :options="sourceOptions"
              size="small"
              placeholder="选择要修改的书源"
              filterable
              clearable
              class="source-selector-input"
            />
            <n-button
              size="small"
              :disabled="!selectedBaseSource || state.isRunning"
              @click="createModifySession"
            >
              载入
            </n-button>
          </div>
          <div class="prompt-tools">
            <div class="prompt-chip-list">
              <button
                v-for="template in visiblePromptTemplates"
                :key="template.label"
                type="button"
                class="prompt-chip prompt-chip--template"
                :disabled="state.isRunning"
                @click="applyPromptTemplate(template)"
              >
                <n-icon><Sparkles /></n-icon>
                {{ template.label }}
              </button>
            </div>
            <div class="prompt-chip-list prompt-chip-list--entities">
              <button
                v-for="token in ENTITY_CHIPS"
                :key="token"
                type="button"
                class="prompt-chip"
                :disabled="state.isRunning"
                @click="appendEntityToken(token)"
              >
                {{ token }}
              </button>
            </div>
          </div>
          <div class="prompt-meta-row">
            <span>任务描述</span>
            <span>{{ promptCharCount }} 字</span>
          </div>
          <n-input
            v-model:value="userPrompt"
            type="textarea"
            :placeholder="workMode === 'modify' ? MODIFY_PLACEHOLDER : NEW_PLACEHOLDER"
            :autosize="{ minRows: 3, maxRows: 7 }"
            :disabled="state.isRunning"
            class="prompt-input"
            @keydown.ctrl.enter.prevent="startAgent(hasConversationHistory)"
          />
          <div class="prompt-buttons">
            <n-button
              size="small"
              quaternary
              :disabled="state.isRunning || promptCharCount === 0"
              @click="clearPrompt"
            >
              清空
            </n-button>
            <n-button
              v-if="hasConversationHistory"
              type="primary"
              :loading="state.isRunning"
              :disabled="!canStartAgent"
              @click="startAgent(true)"
            >
              <template #icon>
                <n-icon><Play /></n-icon>
              </template>
              继续
            </n-button>
            <n-button
              type="primary"
              :loading="state.isRunning && !hasConversationHistory"
              :disabled="!canStartAgent"
              :ghost="hasConversationHistory"
              @click="startAgent(false)"
            >
              <template #icon>
                <n-icon>
                  <RotateCcw v-if="hasConversationHistory" />
                  <Play v-else />
                </n-icon>
              </template>
              {{ hasConversationHistory ? "重开" : "开始" }}
            </n-button>
          </div>
        </div>
      </aside>
    </div>

    <AppDialog
      v-model:show="settingsDialogShow"
      title="AI 设置"
      width="720px"
      class="ai-settings-dialog"
    >
      <div class="ai-config-panel">
        <div class="cfg-grid">
          <div class="cfg-row cfg-row--presets">
            <span class="cfg-label">供应商预设</span>
            <div class="provider-preset-list">
              <n-button
                v-for="preset in AI_PROVIDER_PRESETS"
                :key="preset.key"
                size="small"
                :type="activeProviderPresetKey === preset.key ? 'primary' : 'default'"
                secondary
                @click="applyProviderPreset(preset)"
              >
                {{ preset.label }}
              </n-button>
            </div>
          </div>
          <div class="cfg-row">
            <span class="cfg-label">API 地址</span>
            <n-input
              v-model:value="config.apiUrl"
              size="small"
              placeholder="https://api.openai.com/v1"
              class="cfg-input"
              @update:value="onConfigChange"
            />
          </div>
          <div class="cfg-row">
            <span class="cfg-label">API 密钥</span>
            <n-input
              v-model:value="config.apiKey"
              type="password"
              size="small"
              placeholder="sk-..."
              class="cfg-input"
              show-password-on="click"
              @update:value="onConfigChange"
            />
          </div>
          <div class="cfg-row">
            <span class="cfg-label">模型名称</span>
            <n-input
              v-model:value="config.model"
              size="small"
              placeholder="gpt-4o / deepseek-chat / qwen-plus"
              class="cfg-input"
              @update:value="onConfigChange"
            />
          </div>
          <div class="cfg-row">
            <span class="cfg-label">最大步骤</span>
            <n-input-number
              v-model:value="config.maxSteps"
              size="small"
              :min="1"
              :max="200"
              :step="5"
              class="cfg-input"
              @update:value="onConfigChange"
            />
          </div>
          <div class="cfg-row">
            <span class="cfg-label">请求模式</span>
            <n-radio-group
              v-model:value="config.apiMode"
              size="small"
              @update:value="onConfigChange"
            >
              <n-radio-button value="chat">Chat Completions</n-radio-button>
              <n-radio-button value="responses">Responses API</n-radio-button>
            </n-radio-group>
          </div>
          <div class="cfg-row">
            <span class="cfg-label">请求通道</span>
            <n-radio-group
              v-model:value="config.requestTransport"
              size="small"
              @update:value="onConfigChange"
            >
              <n-radio-button value="backend">后端 HTTP</n-radio-button>
              <n-radio-button value="frontend">前端直连</n-radio-button>
            </n-radio-group>
          </div>
          <div class="cfg-row">
            <span class="cfg-label">Temperature</span>
            <n-input-number
              v-model:value="config.temperature"
              size="small"
              :min="0"
              :max="2"
              :step="0.1"
              :precision="1"
              placeholder="留空=模型默认"
              class="cfg-input"
              :clearable="true"
              @update:value="onConfigChange"
            />
          </div>
        </div>
        <p class="cfg-hint">
          后端 HTTP 通道通过本地代理流式转发，可复用应用代理和 TLS
          设置；前端直连适合需要绕开后端代理时使用。
        </p>
      </div>
    </AppDialog>
  </div>
</template>

<style scoped>
/* ── 整体布局 ── */
.ai-workbench {
  flex: 1;
  display: flex;
  flex-direction: row;
  overflow: hidden;
  height: 100%;
  min-height: 0;
}

/* ── 左侧会话列表 ── */
.ai-sidebar {
  width: 212px;
  min-width: 212px;
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--color-surface);
  transition:
    width var(--transition-base),
    min-width var(--transition-base);
}
.ai-sidebar--collapsed {
  width: 32px;
  min-width: 32px;
}
.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 8px 6px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}
.sidebar-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.sidebar-toggle {
  flex-shrink: 0;
  font-size: 16px;
  line-height: 1;
  padding: 0 4px;
}
.sidebar-actions {
  padding: 8px;
  flex-shrink: 0;
}
.session-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}
.session-item {
  display: flex;
  align-items: center;
  padding: 6px 8px;
  cursor: pointer;
  transition: background var(--transition-fast);
}
.session-item:hover {
  background: var(--color-surface-hover);
}
.session-item--active {
  background: var(--color-accent-subtle);
}
.session-item--active .session-item-name {
  color: var(--color-accent);
  font-weight: 600;
}
.session-item-main {
  flex: 1;
  min-width: 0;
}
.session-item-name {
  font-size: 12px;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.session-item-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 2px;
}
.session-item-time {
  font-size: 10px;
  color: var(--color-text-muted);
}
.session-delete-btn {
  opacity: 0;
  flex-shrink: 0;
  transition: opacity var(--transition-fast);
  font-size: 10px;
  padding: 0 3px;
}
.session-item:hover .session-delete-btn {
  opacity: 1;
}
.session-empty {
  padding: 20px 8px;
  text-align: center;
  color: var(--color-text-muted);
  font-size: 11px;
  line-height: 1.8;
}
.sidebar-collapsed-sessions {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 6px 0;
  gap: 5px;
}
.session-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--color-border);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.session-dot--active {
  background: var(--color-accent);
}

/* ── 主内容区 ── */
.ai-main {
  flex: 1;
  display: grid;
  grid-template-columns: minmax(360px, 420px) minmax(0, 1fr);
  overflow: hidden;
  min-width: 0;
  min-height: 0;
}
.ai-setup-panel {
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  border-right: 1px solid var(--color-border);
  background: var(--color-surface);
}
.ai-output-panel {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--color-surface);
}
.ai-feature-warning {
  margin: 10px 12px 0;
  flex-shrink: 0;
}

/* ── 状态总览 ── */
.ai-status-strip {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface);
  flex-shrink: 0;
}
.status-item {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 42px;
  padding: 7px 9px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface-raised);
}
.status-item--muted {
  opacity: 0.7;
}
.status-icon {
  flex-shrink: 0;
  font-size: 16px;
  color: var(--color-accent);
}
.status-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.35;
}
.status-text span {
  font-size: 10px;
  color: var(--color-text-muted);
}
.status-text strong {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── 顶部工具栏 ── */
.main-toolbar {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  gap: 8px;
  background: var(--color-surface);
  flex-wrap: wrap;
}
.toolbar-spacer {
  flex: 1;
}
.work-mode-toggle {
  display: inline-grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  flex-shrink: 0;
  min-width: 220px;
  height: 30px;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xs);
  background: var(--color-surface-raised);
}
.work-mode-button {
  min-width: 0;
  height: 100%;
  padding: 0 12px;
  border: none;
  border-right: 1px solid var(--color-border);
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 12px;
  line-height: 1;
  text-align: center;
  white-space: nowrap;
  transition:
    background var(--transition-fast),
    color var(--transition-fast);
}
.work-mode-button:last-child {
  border-right: none;
}
.work-mode-button:hover {
  color: var(--color-text-primary);
}
.work-mode-button--active {
  background: var(--color-accent);
  color: #fff;
}
.work-mode-button--active:hover {
  color: #fff;
}
.toolbar-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

/* ── AI 配置面板 ── */
.ai-config-panel {
  padding: 12px;
  background: var(--color-surface);
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}
.cfg-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 8px;
}
.cfg-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.cfg-row--presets {
  align-items: flex-start;
}
.cfg-label {
  flex-shrink: 0;
  width: 66px;
  font-size: 12px;
  color: var(--color-text-secondary);
}
.cfg-input {
  flex: 1;
}
.provider-preset-list {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
}
.cfg-hint {
  margin: 8px 0 0;
  font-size: 11px;
  color: var(--color-text-muted);
}

/* ── 草稿状态条 ── */
.draft-status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  background: var(--color-surface-raised);
  gap: 8px;
  flex-wrap: wrap;
}
.draft-info {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  min-width: 0;
}
.draft-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-primary);
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-decoration: underline dotted;
  text-underline-offset: 2px;
}
.draft-name:hover {
  color: var(--color-accent);
}
.draft-name-input {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-primary);
  background: var(--color-surface);
  border: 1px solid var(--color-accent);
  border-radius: var(--radius-xs);
  padding: 1px 6px;
  outline: none;
  width: 180px;
}
.draft-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
  flex-wrap: wrap;
}

/* ── 输入区 ── */
.input-section {
  padding: 10px 12px 12px;
  border-bottom: none;
  flex-shrink: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-areas:
    "source"
    "notice"
    "tools"
    "meta"
    "prompt";
  gap: 8px;
  align-items: start;
  min-width: 0;
}
.source-selector-row {
  grid-area: source;
  display: flex;
  align-items: center;
  gap: 8px;
}
.source-selector-label {
  font-size: 12px;
  color: var(--color-text-secondary);
  flex-shrink: 0;
}
.source-selector-input {
  flex: 1;
  min-width: 0;
}
.base-source-notice {
  grid-area: notice;
  font-size: 11px;
  color: var(--color-text-secondary);
  background: var(--color-accent-subtle);
  border: 1px solid color-mix(in srgb, var(--color-accent) 20%, transparent);
  border-radius: var(--radius-xs);
  padding: 4px 8px;
}
.prompt-tools {
  grid-area: tools;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface-raised);
}
.prompt-tool-row {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  align-items: start;
  gap: 8px;
  min-width: 0;
  max-width: 100%;
}
.prompt-tool-label {
  width: 52px;
  flex-shrink: 0;
  padding-top: 4px;
  color: var(--color-text-muted);
  font-size: 11px;
}
.prompt-chip-list {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: 5px;
  min-width: 0;
  max-width: 100%;
}
.prompt-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-height: 24px;
  max-width: 160px;
  padding: 3px 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xs);
  background: var(--color-surface);
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 11px;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition:
    border-color var(--transition-fast),
    color var(--transition-fast),
    background var(--transition-fast);
}
.prompt-chip:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--color-accent) 45%, transparent);
  background: var(--color-accent-subtle);
  color: var(--color-accent);
}
.prompt-chip:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.prompt-chip--template {
  color: var(--color-text-primary);
  font-weight: 600;
}
.prompt-meta-row {
  grid-area: meta;
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--color-text-muted);
  font-size: 11px;
}
.prompt-row {
  grid-area: prompt;
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: stretch;
  min-width: 0;
}
.prompt-input {
  flex: 1;
  min-width: 0;
  font-size: 13px;
}
.prompt-buttons {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: 6px;
  flex-shrink: 0;
  min-width: 0;
}
.prompt-buttons :deep(.n-button) {
  flex: 1 1 96px;
}

/* ── 主体 Tab 区 ── */
.ai-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
  width: 100%;
}
.pane-tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  background: var(--color-surface);
  overflow-x: auto;
  scrollbar-width: thin;
}
.pane-tab {
  flex: 0 0 auto;
  padding: 7px 12px;
  font-size: 12px;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  color: var(--color-text-secondary);
  transition:
    color var(--transition-fast),
    border-color var(--transition-fast);
  white-space: nowrap;
  display: flex;
  align-items: center;
  gap: 5px;
}
.pane-tab:hover {
  color: var(--color-text-primary);
}
.pane-tab--active {
  color: var(--color-accent);
  border-bottom-color: var(--color-accent);
  font-weight: 600;
}
.tab-running-dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--color-accent);
  animation: pulse-dot 1s ease-in-out infinite;
}
@keyframes pulse-dot {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.3;
  }
}

/* ── 日志列表 ── */
.log-list {
  flex: 1;
  overflow-y: auto;
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.empty-hint {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 0;
  color: var(--color-text-muted);
  gap: 8px;
  font-size: 13px;
  text-align: center;
}
.empty-icon {
  font-size: 32px;
  width: 32px;
  height: 32px;
  opacity: 0.5;
}
.log-item {
  display: flex;
  width: 100%;
}
.log-item--assistant {
  justify-content: flex-start;
}
.log-item--user {
  justify-content: flex-end;
}
.log-item--system {
  justify-content: center;
}
.log-bubble {
  max-width: min(76%, 880px);
  min-width: 160px;
  border-radius: 8px;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  background: var(--color-surface-raised);
  transition: border-color var(--transition-fast);
}
.log-item--user .log-bubble {
  background: color-mix(in srgb, var(--color-accent) 18%, var(--color-surface-raised));
  border-color: color-mix(in srgb, var(--color-accent) 38%, transparent);
}
.log-item--system .log-bubble {
  max-width: min(70%, 720px);
  min-width: 0;
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--color-surface-hover);
  opacity: 0.86;
}
.log-hd {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 3px;
}
.log-item--system .log-hd {
  display: none;
}
.log-time {
  font-size: 11px;
  font-family: monospace;
  color: var(--color-text-muted);
  flex-shrink: 0;
}
.log-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: 3px;
  flex-shrink: 0;
  background: var(--badge-bg, var(--color-surface-hover));
  color: var(--badge-color, var(--color-text-secondary));
}
.log-tool {
  font-size: 11px;
  font-family: monospace;
  color: var(--color-accent);
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.log-spinner {
  display: inline-block;
  width: 10px;
  height: 10px;
  border: 2px solid var(--color-text-muted);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  flex-shrink: 0;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
.log-section {
  margin-top: 8px;
}
.log-section-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-muted);
  margin-bottom: 2px;
  font-weight: 600;
}
.log-pre {
  margin: 0;
  font-size: 12px;
  font-family: "Consolas", "Menlo", monospace;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--color-text-primary);
  line-height: 1.55;
  max-height: 280px;
  overflow-y: auto;
  padding: 0;
}
.log-item--system .log-pre {
  font-size: 11px;
  line-height: 1.4;
  color: var(--color-text-secondary);
  text-align: center;
}
.log-pre--args {
  color: var(--color-text-secondary);
}
.log-pre--result {
  color: var(--color-text-primary);
}
.tool-call-card {
  width: min(420px, 64%);
  border: 1px solid color-mix(in srgb, var(--color-accent) 28%, transparent);
  border-radius: 7px;
  background: color-mix(in srgb, var(--color-accent) 5%, var(--color-surface-raised));
}
.tool-call-summary {
  height: 30px;
  padding: 0 10px;
  display: flex;
  align-items: center;
  gap: 7px;
  cursor: pointer;
  list-style: none;
}
.tool-call-summary::-webkit-details-marker {
  display: none;
}
.tool-call-status {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--color-text-muted);
}
.tool-call-expand {
  margin-left: auto;
  flex-shrink: 0;
  font-size: 11px;
  color: var(--color-text-muted);
}
.tool-call-card[open] .tool-call-expand {
  color: var(--color-accent);
}
.tool-call-card[open] {
  width: min(760px, 82%);
  padding-bottom: 8px;
}
.tool-call-card[open] .log-section {
  padding: 0 10px;
}
.log-item--thinking {
  --badge-bg: var(--color-surface-hover);
  --badge-color: var(--color-text-secondary);
}
.log-item--tool-call {
  --badge-bg: color-mix(in srgb, var(--color-accent) 15%, transparent);
  --badge-color: var(--color-accent);
}
.log-item--message {
  --badge-bg: color-mix(in srgb, var(--color-warning) 20%, transparent);
  --badge-color: var(--color-warning);
}
.log-item--message .log-bubble {
  border-color: color-mix(in srgb, var(--color-warning) 35%, transparent);
}
.log-item--error {
  --badge-bg: color-mix(in srgb, var(--color-danger) 15%, transparent);
  --badge-color: var(--color-danger);
}
.log-item--error .log-bubble {
  border-color: color-mix(in srgb, var(--color-danger) 30%, transparent);
  background: color-mix(in srgb, var(--color-danger) 6%, var(--color-surface-raised));
}
.log-item--info {
  --badge-bg: var(--color-surface-hover);
  --badge-color: var(--color-text-secondary);
}

/* ── 书源代码面板 ── */
.source-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.source-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  background: var(--color-surface);
}
.source-toolbar-actions {
  display: flex;
  gap: 6px;
}
.source-name {
  font-size: 12px;
  font-family: monospace;
  color: var(--color-accent);
  font-weight: 600;
}
.source-code {
  flex: 1;
  margin: 0;
  padding: 14px;
  overflow: auto;
  font-size: 12px;
  font-family: "Consolas", "Menlo", monospace;
  line-height: 1.6;
  white-space: pre;
  color: var(--color-text-primary);
  background: var(--color-surface);
}

/* ── 版本历史面板 ── */
.history-panel {
  flex: 1;
  overflow-y: auto;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.history-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.history-item {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  padding: 10px 12px;
  background: var(--color-surface-raised);
}
.history-item--current {
  border-color: var(--color-accent);
  background: var(--color-accent-subtle);
}
.history-item-hd {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  flex-wrap: wrap;
}
.history-version {
  font-size: 12px;
  font-weight: 700;
  color: var(--color-accent);
  font-family: monospace;
  min-width: 28px;
}
.history-filename {
  font-size: 12px;
  font-family: monospace;
  color: var(--color-text-primary);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.history-time {
  font-size: 11px;
  color: var(--color-text-muted);
  flex-shrink: 0;
}
.history-size {
  font-size: 11px;
  color: var(--color-text-muted);
  flex-shrink: 0;
}
.history-item-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

@media (max-width: 1180px) and (min-width: 721px) {
  .ai-main {
    display: flex;
    flex-direction: column;
  }
  .ai-setup-panel {
    flex: 0 0 auto;
    max-height: min(430px, 58%);
    border-right: none;
    border-bottom: 1px solid var(--color-border);
  }
  .ai-output-panel {
    flex: 1;
  }
}

@media (max-width: 900px) {
  .ai-sidebar {
    width: 188px;
    min-width: 188px;
  }
  .ai-status-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .cfg-grid {
    grid-template-columns: 1fr;
  }
  .input-section {
    grid-template-columns: minmax(0, 1fr);
    grid-template-areas:
      "source"
      "notice"
      "tools"
      "meta"
      "prompt";
  }
  .prompt-row {
    flex-direction: column;
    align-items: stretch;
  }
  .prompt-buttons {
    flex-direction: row;
    flex-wrap: wrap;
  }
}

@media (pointer: coarse), (max-width: 720px) {
  .ai-workbench {
    height: 100%;
    flex-direction: column;
    overflow-y: auto;
    overflow-x: hidden;
    -webkit-overflow-scrolling: touch;
  }
  .ai-sidebar,
  .ai-sidebar--collapsed {
    width: 100%;
    min-width: 0;
    max-height: none;
    border-right: none;
    border-bottom: 1px solid var(--color-border);
  }
  .ai-sidebar--collapsed .sidebar-header {
    border-bottom: none;
  }
  .sidebar-header {
    padding: 8px 12px;
  }
  .sidebar-title {
    font-size: 11px;
    letter-spacing: 0;
  }
  .sidebar-toggle {
    display: none;
  }
  .sidebar-actions {
    padding: 8px 12px 4px;
  }
  .session-list {
    display: flex;
    gap: 8px;
    flex: 0 0 auto;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 4px 12px 10px;
    scrollbar-width: thin;
  }
  .session-item {
    flex: 0 0 150px;
    min-width: 0;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface-raised);
  }
  .session-item--active {
    border-color: color-mix(in srgb, var(--color-accent) 55%, transparent);
  }
  .session-delete-btn {
    opacity: 1;
  }
  .session-empty {
    flex: 1;
    padding: 10px 12px 14px;
  }
  .sidebar-collapsed-sessions {
    flex-direction: row;
    justify-content: flex-start;
    padding: 0 12px 10px;
  }
  .ai-main {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    overflow: visible;
  }
  .ai-setup-panel {
    overflow: visible;
    border-right: none;
    border-bottom: 1px solid var(--color-border);
  }
  .ai-output-panel {
    overflow: visible;
  }
  .ai-feature-warning {
    margin: 8px 10px 0;
    font-size: 12px;
  }
  .main-toolbar,
  .draft-status-bar {
    padding: 8px 10px;
  }
  .main-toolbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: stretch;
  }
  .toolbar-spacer {
    display: none;
  }
  .work-mode-toggle {
    width: 100%;
    min-width: 0;
    height: 36px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .work-mode-button {
    min-width: 0;
    padding: 0 8px;
    font-size: 13px;
    line-height: 1;
    text-align: center;
  }
  .toolbar-actions {
    width: 100%;
    justify-content: flex-start;
  }
  .ai-config-panel {
    padding: 10px;
  }
  .cfg-row {
    align-items: stretch;
    flex-direction: column;
    gap: 4px;
  }
  .cfg-label {
    width: auto;
  }
  .ai-status-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
    padding: 8px 10px;
  }
  .status-item {
    min-height: 38px;
    padding: 6px 8px;
  }
  .draft-info,
  .draft-actions {
    width: 100%;
  }
  .draft-actions {
    flex-wrap: wrap;
  }
  .draft-actions :deep(.n-button) {
    flex: 1 1 128px;
  }
  .input-section {
    padding: 10px;
    border-bottom: none;
  }
  .source-selector-row {
    align-items: stretch;
    flex-wrap: wrap;
  }
  .source-selector-label {
    width: 100%;
  }
  .source-selector-input {
    min-width: 0;
  }
  .prompt-tools {
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
    padding: 7px;
  }
  .prompt-chip-list {
    width: 100%;
    max-width: 100%;
    flex-wrap: nowrap;
    overflow-x: auto;
    padding-bottom: 2px;
    scrollbar-width: thin;
  }
  .prompt-chip {
    flex: 0 0 auto;
  }
  .prompt-buttons {
    width: 100%;
    min-width: 0;
  }
  .prompt-buttons :deep(.n-button) {
    flex: 1 1 96px;
  }
  .ai-body {
    flex: 0 0 auto;
    min-height: 520px;
    overflow: visible;
  }
  .pane-tabs {
    position: sticky;
    top: 0;
    z-index: 2;
  }
  .pane-tab {
    padding: 9px 12px;
  }
  .log-list,
  .history-panel {
    min-height: 420px;
    overflow: visible;
    padding: 10px;
  }
  .log-bubble,
  .tool-call-card,
  .tool-call-card[open] {
    width: auto;
    max-width: 92%;
  }
  .log-item--system .log-bubble {
    max-width: 96%;
  }
  .source-panel {
    min-height: 520px;
  }
  .source-toolbar {
    align-items: flex-start;
    gap: 8px;
    flex-direction: column;
  }
  .source-toolbar-actions {
    width: 100%;
    flex-wrap: wrap;
  }
  .source-toolbar-actions :deep(.n-button) {
    flex: 1 1 128px;
  }
  .source-code {
    min-height: 420px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .log-pre {
    max-height: none;
  }
}

@media (max-width: 520px) {
  .ai-status-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .status-icon {
    display: none;
  }
  .status-item {
    min-height: 36px;
  }
  .prompt-tool-row {
    flex-direction: column;
    gap: 5px;
    display: flex;
  }
  .prompt-tool-label {
    width: auto;
    padding-top: 0;
  }
  .prompt-buttons {
    flex-direction: column;
  }
  .draft-actions :deep(.n-button),
  .prompt-buttons :deep(.n-button),
  .source-toolbar-actions :deep(.n-button) {
    flex-basis: 100%;
  }
}

/* ── Workbench 重构布局 ── */
.ai-workbench {
  flex-direction: column;
  background: var(--color-surface);
}
.workbench-topbar {
  height: 46px;
  flex: 0 0 46px;
  display: grid;
  grid-template-columns: minmax(180px, 1fr) auto minmax(280px, auto) auto;
  align-items: center;
  gap: 12px;
  padding: 0 14px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-raised);
  min-width: 0;
}
.topbar-title {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}
.topbar-title-main {
  flex-shrink: 0;
  color: var(--color-text-primary);
  font-size: 14px;
  font-weight: 700;
}
.topbar-title-sub {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-muted);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.topbar-stats {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  min-width: 0;
  color: var(--color-text-secondary);
  font-size: 11px;
  white-space: nowrap;
}
.topbar-stats .muted {
  color: var(--color-warning);
}
.topbar-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  min-width: 0;
}
.workbench-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 248px minmax(460px, 1fr) clamp(330px, 28vw, 430px);
  min-height: 0;
  overflow: hidden;
}
.workbench-grid--sidebar-collapsed {
  grid-template-columns: 36px minmax(460px, 1fr) clamp(330px, 28vw, 430px);
}
.ai-sidebar {
  width: auto;
  min-width: 0;
  background: var(--color-surface);
}
.ai-sidebar--collapsed {
  width: auto;
  min-width: 36px;
}
.sidebar-header {
  height: 38px;
  padding: 0 10px;
}
.sidebar-section {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-bottom: 1px solid var(--color-border);
}
.sidebar-section:first-of-type {
  flex: 0 0 min(250px, 36%);
}
.sidebar-section--sources {
  flex: 1;
  border-bottom: none;
}
.sidebar-section-hd {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 10px;
  color: var(--color-text-secondary);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
}
.sidebar-count {
  color: var(--color-text-muted);
  font-family: "Consolas", "Menlo", monospace;
  font-weight: 500;
}
.session-list,
.source-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 6px 8px;
}
.session-item {
  min-height: 46px;
  margin: 1px 0;
  border-radius: 6px;
  padding: 6px 8px;
}
.session-item--active {
  background: color-mix(in srgb, var(--color-accent) 14%, transparent);
}
.source-search {
  padding: 0 8px 8px;
}
.source-item {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  min-height: 42px;
  padding: 6px 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-primary);
  cursor: pointer;
  text-align: left;
}
.source-item:hover:not(:disabled),
.source-item--active {
  background: var(--color-surface-hover);
}
.source-item:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.source-item-name,
.source-item-file {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.source-item-name {
  font-size: 12px;
  font-weight: 600;
}
.source-item-file {
  color: var(--color-text-muted);
  font-family: "Consolas", "Menlo", monospace;
  font-size: 10px;
}
.workspace-panel,
.chat-panel {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--color-surface);
}
.workspace-panel {
  border-right: 1px solid var(--color-border);
}
.workspace-titlebar {
  min-height: 44px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 12px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-raised);
}
.draft-name--placeholder {
  color: var(--color-text-muted);
  text-decoration: none;
}
.pane-tabs {
  height: 38px;
  background: var(--color-surface);
}
.pane-tab {
  padding: 0 14px;
}
.workspace-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.source-panel,
.history-panel {
  min-height: 0;
}
.source-toolbar {
  min-height: 38px;
}
.source-code {
  background: color-mix(in srgb, var(--color-surface) 92%, #000);
}
.chat-panel {
  background: color-mix(in srgb, var(--color-surface) 96%, #000);
}
.chat-header {
  height: 42px;
  flex: 0 0 42px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 12px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-raised);
}
.chat-title {
  display: block;
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
}
.chat-live {
  display: block;
  color: var(--color-accent);
  font-size: 11px;
}
.chat-panel .log-list {
  flex: 1;
  min-height: 0;
  padding: 12px;
  overflow-y: auto;
}
.chat-panel .log-bubble {
  max-width: 88%;
}
.chat-panel .tool-call-card {
  width: min(320px, 88%);
}
.chat-panel .tool-call-card[open] {
  width: 94%;
}
.chat-composer {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px 12px;
  border-top: 1px solid var(--color-border);
  background: var(--color-surface-raised);
}
.chat-composer .source-selector-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
}
.chat-composer .prompt-tools {
  padding: 0;
  border: 0;
  background: transparent;
}
.chat-composer .prompt-chip-list {
  flex-wrap: nowrap;
  overflow-x: auto;
  padding-bottom: 2px;
}
.chat-composer .prompt-chip-list--entities {
  opacity: 0.9;
}
.chat-composer .prompt-chip {
  flex: 0 0 auto;
}
.chat-composer .prompt-buttons {
  display: grid;
  grid-template-columns: auto 1fr 1fr;
  gap: 6px;
}
.ai-config-panel {
  padding: 0;
  border-bottom: 0;
}
.ai-settings-dialog .cfg-grid {
  gap: 10px;
}

@media (max-width: 1280px) {
  .workbench-grid {
    grid-template-columns: 220px minmax(380px, 1fr) 340px;
  }
  .topbar-stats {
    display: none;
  }
}

@media (max-width: 980px) {
  .workbench-topbar {
    height: auto;
    grid-template-columns: minmax(0, 1fr);
    padding: 8px 10px;
  }
  .workbench-grid {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }
  .ai-sidebar,
  .workspace-panel,
  .chat-panel {
    border-right: none;
    border-bottom: 1px solid var(--color-border);
    overflow: visible;
  }
  .sidebar-section:first-of-type {
    flex-basis: auto;
  }
  .session-list,
  .source-list {
    max-height: 220px;
  }
  .workspace-panel,
  .chat-panel {
    min-height: 560px;
  }
}

@media (max-width: 560px) {
  .work-mode-toggle {
    width: 100%;
  }
  .workspace-titlebar,
  .draft-actions,
  .topbar-actions,
  .chat-composer .prompt-buttons {
    display: flex;
    flex-wrap: wrap;
  }
  .draft-actions :deep(.n-button),
  .chat-composer .prompt-buttons :deep(.n-button) {
    flex: 1 1 120px;
  }
}
</style>
