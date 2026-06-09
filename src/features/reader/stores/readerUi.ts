import { defineStore } from "pinia";
import { computed, nextTick, ref } from "vue";

export const useReaderUiStore = defineStore("readerUi", () => {
  const activeReaderHostIds = ref<symbol[]>([]);
  const showMenu = ref(false);
  const showToc = ref(false);
  const settingsVisible = ref(false);
  const showTtsBar = ref(false);
  const showSourceSwitchDialog = ref(false);
  const sourceSwitchMode = ref<"whole-book" | "chapter-temp">("whole-book");
  const menuOpenTime = ref(0);
  const readerVisible = computed(() => activeReaderHostIds.value.length > 0);

  function setReaderHostVisible(hostId: symbol, visible: boolean) {
    const exists = activeReaderHostIds.value.includes(hostId);
    if (visible === exists) {
      return;
    }
    activeReaderHostIds.value = visible
      ? [...activeReaderHostIds.value, hostId]
      : activeReaderHostIds.value.filter((id) => id !== hostId);
  }

  function resetLayers() {
    showMenu.value = false;
    showToc.value = false;
    settingsVisible.value = false;
    showTtsBar.value = false;
    showSourceSwitchDialog.value = false;
    sourceSwitchMode.value = "whole-book";
    menuOpenTime.value = 0;
  }

  function openMenu() {
    showMenu.value = true;
    menuOpenTime.value = Date.now();
  }

  function closeMenu() {
    showMenu.value = false;
    settingsVisible.value = false;
  }

  async function openToc() {
    settingsVisible.value = false;
    await nextTick();
    showMenu.value = false;
    await nextTick();
    showToc.value = true;
  }

  function closeToc() {
    showToc.value = false;
  }

  function openSourceSwitch(mode: "whole-book" | "chapter-temp") {
    sourceSwitchMode.value = mode;
    showSourceSwitchDialog.value = true;
  }

  return {
    readerVisible,
    showMenu,
    showToc,
    settingsVisible,
    showTtsBar,
    showSourceSwitchDialog,
    sourceSwitchMode,
    menuOpenTime,
    setReaderHostVisible,
    resetLayers,
    openMenu,
    closeMenu,
    openToc,
    closeToc,
    openSourceSwitch,
  };
});
