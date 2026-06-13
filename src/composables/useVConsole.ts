import { storeToRefs } from "pinia";
import type VConsoleModule from "vconsole";
/**
 * useVConsole — 管理 vConsole 调试面板的生命周期
 *
 * 根据 preferences.devTools.vConsoleEnabled 的值动态初始化或销毁 vConsole，
 * 并在系统主题（暗色/亮色）切换时同步更新 vConsole 的 theme 配置。
 */
import { watch, type Ref } from "vue";
import { usePreferencesStore } from "@/stores/preferences";

type VConsoleConstructor = typeof VConsoleModule;
type VConsoleInstance = InstanceType<VConsoleConstructor>;

let _instance: VConsoleInstance | null = null;
let _loading: Promise<VConsoleConstructor> | null = null;
let _enabled = false;

function loadVConsole(): Promise<VConsoleConstructor> {
  if (!_loading) {
    _loading = import("vconsole")
      .then((mod) => mod.default)
      .catch((error: unknown) => {
        _loading = null;
        throw error;
      });
  }
  return _loading;
}

export function useVConsole(effectiveDark: Ref<boolean>) {
  const prefStore = usePreferencesStore();
  const { devTools } = storeToRefs(prefStore);

  function getTheme(): "dark" | "light" {
    return effectiveDark.value ? "dark" : "light";
  }

  async function init() {
    if (_instance) {
      return;
    }
    try {
      const VConsole = await loadVConsole();
      if (!_enabled || _instance) {
        return;
      }
      _instance = new VConsole({ theme: getTheme() });
    } catch (error: unknown) {
      console.warn("vConsole 加载失败", error);
    }
  }

  function destroy() {
    if (!_instance) {
      return;
    }
    _instance.destroy();
    _instance = null;
  }

  // 跟随 enabled 开关
  watch(
    () => devTools.value.vConsoleEnabled,
    (enabled) => {
      _enabled = enabled;
      if (enabled) {
        void init();
      } else {
        destroy();
      }
    },
    { immediate: true },
  );

  // 跟随主题变化实时切换
  watch(effectiveDark, (dark) => {
    if (_instance) {
      _instance.setOption("theme", dark ? "dark" : "light");
    }
  });
}
