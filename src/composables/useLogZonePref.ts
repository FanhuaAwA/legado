/**
 * useLogZonePref — 实时日志区域显示开关
 * 纯前端 UI 偏好，持久化到 frontend storage，并保留旧 key 一次性迁移。
 */
import { computed } from "vue";
import { useDynamicConfig } from "./useDynamicConfig";

const LS_KEY = "legado-ui-log-zone-enabled";

const prefStore = useDynamicConfig<{ enabled: boolean }>({
  namespace: "ui.logZone",
  version: 1,
  defaults: () => ({ enabled: false }),
  migrate: ({ readLegacy }) => {
    const legacy = readLegacy(LS_KEY);
    return legacy === null ? null : { enabled: legacy === "true" };
  },
  legacyKeys: [LS_KEY],
});

const logZoneEnabled = computed({
  get: () => prefStore.state.enabled,
  set: (enabled: boolean) => {
    void prefStore.replace({ enabled });
  },
});

export function useLogZonePref() {
  return { logZoneEnabled };
}
