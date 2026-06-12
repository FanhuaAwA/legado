<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, ref, watch } from "vue";
import { platform } from "@/composables/useEnv";
import { useAppConfigStore, usePreferencesStore } from "@/stores";
import SettingItem from "./SettingItem.vue";
import SettingSection from "./SettingSection.vue";

const _appCfg = useAppConfigStore();
const { config } = storeToRefs(_appCfg);
const { setConfig } = _appCfg;

const prefsStore = usePreferencesStore();
const tocCfg = computed(() => prefsStore.tocAutoUpdate);
const searchCfg = computed(() => prefsStore.search);
const isAndroidPlatform = computed(() => platform.value === "Android");

async function handleSet(key: string, value: string) {
  try {
    await setConfig(key, value);
  } catch (e: unknown) {
    console.error(`保存失败: ${e}`);
  }
}

function setReaderAwakeTimeoutMinutes(value: number | null) {
  const minutes = Math.min(120, Math.max(1, Math.round(value ?? 10)));
  void handleSet("power_reader_awake_timeout_secs", String(minutes * 60));
}

// ── 主题颜色选择器 ─────────────────────────────────────────────────────────
const THEME_COLOR_PRESETS = [
  "#6366f1", // 默认 indigo
  "#3b82f6", // 蓝
  "#06b6d4", // 青
  "#10b981", // 绿
  "#f59e0b", // 琥珀
  "#ef4444", // 红
  "#ec4899", // 粉
  "#8b5cf6", // 紫
  "#f97316", // 橙
  "#84cc16", // 黄绿
  "#14b8a6", // 蓝绿
  "#6b7280", // 灰
];

const HEX_RE = /^#[0-9a-fA-F]{6}$/;
const themeColorInput = ref(config.value.ui_theme_color ?? "");

watch(
  () => config.value.ui_theme_color,
  (v) => {
    themeColorInput.value = v ?? "";
  },
);

function applyThemeColor(hex: string) {
  const trimmed = hex.trim().slice(0, 7);
  if (HEX_RE.test(trimmed)) {
    themeColorInput.value = trimmed;
    void handleSet("ui_theme_color", trimmed);
  }
}

function onColorInputBlur() {
  const trimmed = themeColorInput.value.trim().slice(0, 7);
  if (trimmed === "") {
    // 重置为默认
    void handleSet("ui_theme_color", "");
    themeColorInput.value = "";
    return;
  }
  if (HEX_RE.test(trimmed)) {
    applyThemeColor(trimmed);
  } else {
    themeColorInput.value = config.value.ui_theme_color ?? "";
  }
}

function resetThemeColor() {
  themeColorInput.value = "";
  void handleSet("ui_theme_color", "");
}

const INTERVAL_OPTIONS = [
  { label: "2 小时", value: 7200 },
  { label: "4 小时", value: 14400 },
  { label: "8 小时", value: 28800 },
  { label: "24 小时", value: 86400 },
];
</script>

<template>
  <SettingSection title="通用" section-id="section-general">
    <SettingItem label="主题" desc="选择应用外观主题；自动模式跟随系统">
      <n-radio-group
        :value="config.ui_theme"
        size="small"
        @update:value="(v: string) => handleSet('ui_theme', v)"
      >
        <n-radio-button value="auto">跟随系统</n-radio-button>
        <n-radio-button value="light">亮色</n-radio-button>
        <n-radio-button value="dark">暗色</n-radio-button>
      </n-radio-group>
    </SettingItem>

    <SettingItem label="主题颜色" desc="自定义应用主题色，留空则使用默认颜色" :vertical="true">
      <div class="theme-color-control">
        <div class="theme-color-field">
          <label class="theme-color-picker" :style="{ background: themeColorInput || '#6366f1' }">
            <span class="theme-color-picker__sr">选择主题颜色</span>
            <input
              type="color"
              :value="themeColorInput || '#6366f1'"
              @input="(e: Event) => applyThemeColor((e.target as HTMLInputElement).value)"
            />
          </label>
          <n-input
            :value="themeColorInput"
            class="theme-color-input"
            size="small"
            placeholder="#6366f1"
            maxlength="7"
            @update:value="(v: string) => (themeColorInput = v.slice(0, 7))"
            @blur="onColorInputBlur"
          />
          <n-button v-if="themeColorInput" size="small" quaternary @click="resetThemeColor">
            重置
          </n-button>
        </div>
        <div class="theme-color-presets" aria-label="主题颜色预设">
          <button
            v-for="preset in THEME_COLOR_PRESETS"
            :key="preset"
            type="button"
            class="theme-color-swatch"
            :class="{ 'theme-color-swatch--active': themeColorInput === preset }"
            :style="{ background: preset }"
            :aria-label="`使用颜色 ${preset}`"
            :title="preset"
            @click="applyThemeColor(preset)"
          />
        </div>
      </div>
    </SettingItem>

    <SettingItem label="布局模式" desc="切换手机 / 电脑界面；自动模式根据设备自动判断">
      <n-radio-group
        :value="config.ui_layout_mode"
        size="small"
        @update:value="(v: string) => handleSet('ui_layout_mode', v)"
      >
        <n-radio-button value="auto">自动</n-radio-button>
        <n-radio-button value="mobile">手机</n-radio-button>
        <n-radio-button value="desktop">电脑</n-radio-button>
      </n-radio-group>
    </SettingItem>
  </SettingSection>

  <SettingSection v-if="isAndroidPlatform" title="屏幕唤醒" section-id="section-power">
    <SettingItem
      label="朗读时保持亮屏"
      desc="朗读播放期间阻止屏幕自动休眠，停止或暂停后恢复系统规则"
    >
      <n-switch
        :value="config.power_keep_awake_on_tts"
        @update:value="(v: boolean) => handleSet('power_keep_awake_on_tts', String(v))"
      >
        <template #checked>开启</template>
        <template #unchecked>关闭</template>
      </n-switch>
    </SettingItem>

    <SettingItem label="阅读页保持亮屏" desc="仅在阅读页面生效，离开阅读后恢复系统休眠">
      <n-radio-group
        :value="config.power_reader_awake_mode"
        size="small"
        @update:value="(v: string) => handleSet('power_reader_awake_mode', v)"
      >
        <n-radio-button value="off">关闭</n-radio-button>
        <n-radio-button value="always">始终</n-radio-button>
        <n-radio-button value="timeout">自定义时长</n-radio-button>
      </n-radio-group>
    </SettingItem>

    <SettingItem
      v-if="config.power_reader_awake_mode === 'timeout'"
      label="保持时长"
      desc="每次触摸、翻页、滚动或按键后重新计时"
    >
      <div style="display: flex; gap: 6px; align-items: center">
        <n-input-number
          :value="Math.round((config.power_reader_awake_timeout_secs ?? 600) / 60)"
          size="small"
          :min="1"
          :max="120"
          style="width: 96px"
          @update:value="setReaderAwakeTimeoutMinutes"
        />
        <span>分钟</span>
      </div>
    </SettingItem>
  </SettingSection>

  <SettingSection title="书架" section-id="section-bookshelf">
    <SettingItem label="自动更新目录" desc="定期在后台静默检测书籍是否有新章节，不影响阅读">
      <n-switch
        :value="tocCfg.enabled"
        @update:value="(v: boolean) => prefsStore.patchTocAutoUpdate({ enabled: v })"
      >
        <template #checked>开启</template>
        <template #unchecked>关闭</template>
      </n-switch>
    </SettingItem>

    <template v-if="tocCfg.enabled">
      <SettingItem label="最小检测间隔" desc="两次自动检测同一本书之间的最短等待时间">
        <n-radio-group
          :value="tocCfg.minIntervalSecs"
          size="small"
          @update:value="(v: number) => prefsStore.patchTocAutoUpdate({ minIntervalSecs: v })"
        >
          <n-radio-button v-for="opt in INTERVAL_OPTIONS" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </n-radio-button>
        </n-radio-group>
      </SettingItem>

      <SettingItem
        label="打开图书时检测"
        desc="每次打开书籍时，若距离上次检测超过最小间隔则自动更新目录"
      >
        <n-switch
          :value="tocCfg.onBookOpen"
          @update:value="(v: boolean) => prefsStore.patchTocAutoUpdate({ onBookOpen: v })"
        >
          <template #checked>开启</template>
          <template #unchecked>关闭</template>
        </n-switch>
      </SettingItem>

      <SettingItem label="启动 App 时检测" desc="每次启动应用时，后台逐本检测所有书籍的目录更新">
        <n-switch
          :value="tocCfg.onAppStart"
          @update:value="(v: boolean) => prefsStore.patchTocAutoUpdate({ onAppStart: v })"
        >
          <template #checked>开启</template>
          <template #unchecked>关闭</template>
        </n-switch>
      </SettingItem>

      <SettingItem
        label="切换到书架时检测"
        desc="每次从其他页面切换回书架时，后台逐本检测所有书籍的目录更新"
      >
        <n-switch
          :value="tocCfg.onShelfView"
          @update:value="(v: boolean) => prefsStore.patchTocAutoUpdate({ onShelfView: v })"
        >
          <template #checked>开启</template>
          <template #unchecked>关闭</template>
        </n-switch>
      </SettingItem>
    </template>
  </SettingSection>

  <SettingSection title="搜索与换源" section-id="section-search">
    <SettingItem
      label="搜索并发数"
      desc="同时向多少个书源发起搜索请求，越大速度越快但对网络压力越高，默认 5"
    >
      <div style="display: flex; gap: 6px; align-items: center">
        <n-input-number
          :value="searchCfg.searchConcurrency ?? 5"
          size="small"
          :min="1"
          :max="20"
          style="width: 80px"
          @update:value="
            (v: number | null) => prefsStore.patchSearch({ searchConcurrency: Math.max(1, v ?? 5) })
          "
        />
        <span>个</span>
      </div>
    </SettingItem>

    <SettingItem label="换源并发数" desc="换源搜索时同时查询的书源数量，默认 5">
      <div style="display: flex; gap: 6px; align-items: center">
        <n-input-number
          :value="searchCfg.switchSourceConcurrency ?? 5"
          size="small"
          :min="1"
          :max="20"
          style="width: 80px"
          @update:value="
            (v: number | null) =>
              prefsStore.patchSearch({
                switchSourceConcurrency: Math.max(1, v ?? 5),
              })
          "
        />
        <span>个</span>
      </div>
    </SettingItem>
  </SettingSection>
</template>

<style scoped>
.theme-color-control {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: min(100%, 390px);
}

.theme-color-field {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.theme-color-picker {
  position: relative;
  width: 36px;
  height: 36px;
  flex: 0 0 36px;
  overflow: hidden;
  border: 2px solid var(--color-border-strong, var(--color-border));
  border-radius: 6px;
  cursor: pointer;
}

.theme-color-picker input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: 0;
  padding: 0;
  opacity: 0;
  cursor: pointer;
}

.theme-color-picker__sr {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
}

.theme-color-input {
  width: 104px;
  flex: 0 0 104px;
}

.theme-color-presets {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(24px, 24px));
  gap: 6px;
  align-items: center;
}

.theme-color-swatch {
  width: 24px;
  height: 24px;
  border: 2px solid transparent;
  border-radius: 5px;
  cursor: pointer;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
}

.theme-color-swatch--active {
  border-color: var(--color-text);
  box-shadow:
    0 0 0 1px var(--color-bg, var(--color-surface)),
    inset 0 0 0 1px rgba(255, 255, 255, 0.28);
}

.theme-color-swatch:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

@media (max-width: 520px) {
  .theme-color-control {
    width: 100%;
  }

  .theme-color-field {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    width: 100%;
  }

  .theme-color-input {
    width: 100%;
    flex-basis: auto;
  }
}
</style>
