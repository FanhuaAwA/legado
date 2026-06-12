// navigation — 管理主视图切换，以及跨视图深链接请求的延迟投递。
import { defineStore } from "pinia";
import { ref } from "vue";

export interface OnlineRepoDeepLinkRequest {
  id: number;
  url: string;
  name?: string;
}

export interface PluginDeepLinkRequest {
  id: number;
  url: string;
}

export interface BookSourceImportDeepLinkRequest {
  id: number;
  url: string;
}

export const useNavigationStore = defineStore("navigation", () => {
  /** 当前激活的视图 ID */
  const activeView = ref("bookshelf");

  const onlineRepoDeepLinkRequest = ref<OnlineRepoDeepLinkRequest | null>(null);
  let onlineRepoDeepLinkSeq = 0;
  const pluginDeepLinkRequest = ref<PluginDeepLinkRequest | null>(null);
  let pluginDeepLinkSeq = 0;
  const bookSourceImportDeepLinkRequest = ref<BookSourceImportDeepLinkRequest | null>(null);
  let bookSourceImportDeepLinkSeq = 0;

  /** 搜索视图的初始限定书源（优先 sourceKey，兼容 fileName），null 表示搜索全部书源 */
  const searchInitSource = ref<string | null>(null);

  /** 导航到搜索视图，可选限定单一书源 */
  function navigateToSearch(sourceId?: string) {
    searchInitSource.value = sourceId ?? null;
    activeView.value = "search";
  }

  function setActiveView(view: string) {
    activeView.value = view;
  }

  function navigateToOnlineRepo(url: string, name?: string) {
    onlineRepoDeepLinkRequest.value = {
      id: ++onlineRepoDeepLinkSeq,
      url,
      name,
    };
    activeView.value = "booksource";
  }

  function navigateToPluginInstall(url: string) {
    pluginDeepLinkRequest.value = {
      id: ++pluginDeepLinkSeq,
      url,
    };
    activeView.value = "extensions";
  }

  function navigateToBookSourceImport(url: string) {
    bookSourceImportDeepLinkRequest.value = {
      id: ++bookSourceImportDeepLinkSeq,
      url,
    };
    activeView.value = "booksource";
  }

  function consumeOnlineRepoDeepLinkRequest(id: number) {
    if (onlineRepoDeepLinkRequest.value?.id === id) {
      onlineRepoDeepLinkRequest.value = null;
    }
  }

  function consumePluginDeepLinkRequest(id: number) {
    if (pluginDeepLinkRequest.value?.id === id) {
      pluginDeepLinkRequest.value = null;
    }
  }

  function consumeBookSourceImportDeepLinkRequest(id: number) {
    if (bookSourceImportDeepLinkRequest.value?.id === id) {
      bookSourceImportDeepLinkRequest.value = null;
    }
  }

  return {
    activeView,
    searchInitSource,
    onlineRepoDeepLinkRequest,
    pluginDeepLinkRequest,
    bookSourceImportDeepLinkRequest,
    navigateToSearch,
    setActiveView,
    navigateToOnlineRepo,
    navigateToPluginInstall,
    navigateToBookSourceImport,
    consumeOnlineRepoDeepLinkRequest,
    consumePluginDeepLinkRequest,
    consumeBookSourceImportDeepLinkRequest,
  };
});
