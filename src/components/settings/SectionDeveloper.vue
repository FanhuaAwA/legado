<script setup lang="ts">
import { useMessage } from "naive-ui";
import { storeToRefs } from "pinia";
import { computed, ref, watch } from "vue";
import { isMobile, isTauri } from "@/composables/useEnv";
import { useLogZonePref } from "@/composables/useLogZonePref";
import { useAppConfigStore, useShellStatusStore } from "@/stores";
import { usePreferencesStore } from "@/stores/preferences";
import FullModeUnlockDialog from "./FullModeUnlockDialog.vue";
import SettingItem from "./SettingItem.vue";
import SettingSection from "./SettingSection.vue";

const message = useMessage();
const _appCfg = useAppConfigStore();
const { config, savingKey } = storeToRefs(_appCfg);
const { setConfig } = _appCfg;
const shellStore = useShellStatusStore();
const { logZoneEnabled } = useLogZonePref();
const prefStore = usePreferencesStore();
const { devTools } = storeToRefs(prefStore);

const showUnlockDialog = ref(false);
const remoteDebugHostInput = ref(config.value.web_remote_debug_host || "");
const remoteDebugPortInput = ref<number | null>(config.value.web_remote_debug_port || 8080);

// 追踪 vConsole 开关是否在本次会话中被修改过
const _initVConsole = devTools.value.vConsoleEnabled;
const vConsoleChanged = computed(() => devTools.value.vConsoleEnabled !== _initVConsole);
const remoteDebugSaving = computed(
  () => savingKey.value === "web_remote_debug_host" || savingKey.value === "web_remote_debug_port",
);

watch(
  () => [config.value.web_remote_debug_host, config.value.web_remote_debug_port] as const,
  ([host, port]) => {
    remoteDebugHostInput.value = host || "";
    remoteDebugPortInput.value = port || 8080;
  },
);

async function handleSet(key: string, value: string) {
  try {
    await setConfig(key, value);
    message.success("已保存");
  } catch (e: unknown) {
    message.error(`保存失败: ${e}`);
  }
}

async function saveRemoteDebugEndpoint(silent = false) {
  const host = remoteDebugHostInput.value.trim();
  const port = remoteDebugPortInput.value;
  if (!host) {
    message.error("请填写运行 Chii 的电脑 IP 或主机名");
    return false;
  }
  if (port === null || !Number.isInteger(port) || port < 1 || port > 65535) {
    message.error("端口号必须为 1 ~ 65535 的整数");
    return false;
  }
  try {
    await setConfig("web_remote_debug_host", host);
    await setConfig("web_remote_debug_port", String(port));
    if (!silent) {
      message.success("Chii 调试地址已保存");
    }
    return true;
  } catch (e: unknown) {
    message.error(`保存失败: ${e}`);
    return false;
  }
}

async function handleRemoteDebugToggle(enabled: boolean) {
  try {
    if (enabled) {
      const saved = await saveRemoteDebugEndpoint(true);
      if (!saved) {
        return;
      }
    }
    await setConfig("web_remote_debug_enabled", String(enabled));
    message.success(enabled ? "已开启远程调试注入" : "已关闭远程调试注入");
  } catch (e: unknown) {
    message.error(`保存失败: ${e}`);
  }
}
</script>

<template>
  <SettingSection title="开发设置" section-id="section-developer">
    <!-- 实时日志：开关只控制 PC 底部任务栏日志区域，按钮直接打开通用日志面板 -->
    <SettingItem
      label="实时日志"
      desc="开关控制 PC 底部任务栏是否显示实时日志区域；点击「打开」直接查看脚本运行日志、HTTP 请求等"
    >
      <div style="display: flex; align-items: center; gap: 8px">
        <n-switch v-if="!isMobile" v-model:value="logZoneEnabled" size="small" />
        <n-button size="small" @click="shellStore.openLogWindow()">打开</n-button>
      </div>
    </SettingItem>

    <!-- 书源文件监听（仅 Tauri） -->
    <SettingItem
      v-if="isTauri"
      label="书源文件监听"
      desc="开启后，书源目录中的 .js 变更会自动触发发现页/能力缓存刷新（热重载）。修改后需重启生效。"
    >
      <n-switch
        :value="config.booksource_watcher_enabled"
        size="small"
        :loading="savingKey === 'booksource_watcher_enabled'"
        @update:value="(v: boolean) => handleSet('booksource_watcher_enabled', String(v))"
      />
    </SettingItem>

    <!-- vConsole 调试面板 -->
    <SettingItem
      label="vConsole 调试面板"
      desc="启用后，页面右下角显示 vConsole 浮动按钮，可查看日志、网络请求、存储等调试信息。支持深色模式。"
    >
      <div style="display: flex; flex-direction: column; gap: 6px; align-items: flex-start">
        <n-switch
          :value="devTools.vConsoleEnabled"
          size="small"
          @update:value="(v: boolean) => prefStore.patchDevTools({ vConsoleEnabled: v })"
        />
        <span
          v-if="vConsoleChanged"
          style="
            font-size: 0.72rem;
            color: var(--color-text-muted);
            display: flex;
            align-items: center;
            gap: 3px;
          "
          >↺ 重启后生效</span
        >
      </div>
    </SettingItem>

    <!-- Chii 远程调试：前端统一注入，覆盖 Tauri 与服务模式 -->
    <SettingItem
      label="Chii 远程调试"
      desc="开启后会在当前前端页面注入 Chii target 脚本，Tauri 模式和浏览器服务模式都生效。"
    >
      <n-switch
        :value="config.web_remote_debug_enabled"
        size="small"
        :loading="savingKey === 'web_remote_debug_enabled'"
        @update:value="handleRemoteDebugToggle"
      />
    </SettingItem>

    <SettingItem
      label="Chii 地址"
      desc="填写运行 Chii 的电脑 IP/主机名和端口。手机或其他设备必须能访问该地址。"
      :vertical="true"
    >
      <div class="remote-debug-editor">
        <n-input
          v-model:value="remoteDebugHostInput"
          size="small"
          placeholder="例如 192.168.1.10"
          class="remote-debug-host"
          @keydown.enter.prevent="saveRemoteDebugEndpoint()"
        />
        <n-input-number
          v-model:value="remoteDebugPortInput"
          size="small"
          :min="1"
          :max="65535"
          :show-button="false"
          placeholder="8080"
          class="remote-debug-port"
          @keydown.enter.prevent="saveRemoteDebugEndpoint()"
        />
        <n-button size="small" :loading="remoteDebugSaving" @click="saveRemoteDebugEndpoint()">
          保存
        </n-button>
      </div>
      <div class="remote-debug-help">
        <div>1. 在电脑上执行 <code>npm install chii -g</code> 安装 Chii。</div>
        <div>
          2. 执行
          <code>chii start -p {{ config.web_remote_debug_port }}</code>
          启动调试服务。
        </div>
        <div>
          3. 在这里填写电脑局域网 IP 和端口，开启开关后会注入
          <code>http://IP:端口/target.js</code>。
        </div>
        <div>
          4. 在电脑浏览器打开
          <code>http://localhost:{{ config.web_remote_debug_port }}</code>
          ，点击目标页面的「检查」开始调试。
        </div>
      </div>
    </SettingItem>

    <!-- 完全体模式解锁 -->
    <SettingItem
      label="解除限制"
      desc="激活完全体模式,需通过挑战码验证。(仅供开发者调试使用，勿泄露挑战码) "
    >
      <n-button
        size="small"
        :type="devTools.fullModeEnabled ? 'success' : 'default'"
        @click="showUnlockDialog = true"
      >
        {{ devTools.fullModeEnabled ? "已激活" : "解除限制" }}
      </n-button>
    </SettingItem>
  </SettingSection>

  <FullModeUnlockDialog v-model:show="showUnlockDialog" />
</template>

<style scoped>
.remote-debug-editor {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.remote-debug-host {
  width: min(100%, 260px);
}

.remote-debug-port {
  width: 100px;
  font-family: var(--font-mono, monospace);
}

.remote-debug-help {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: var(--space-2);
  color: var(--color-text-soft);
  font-size: var(--fs-13);
  line-height: 1.55;
}

.remote-debug-help code {
  font-family: var(--font-mono, monospace);
  color: var(--color-text);
  word-break: break-all;
}

@media (max-width: 640px) {
  .remote-debug-editor,
  .remote-debug-host {
    width: 100%;
  }
}
</style>
