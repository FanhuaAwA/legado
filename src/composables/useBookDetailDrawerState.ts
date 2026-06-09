import { computed, ref, type ComputedRef, type Ref } from "vue";
import type { BookItem } from "@/stores";
import type { BookSourceMeta } from "./useBookSource";

interface UseBookDetailDrawerStateOptions {
  sources: Ref<BookSourceMeta[]>;
  onOpenDetail?: (payload: {
    bookUrl: string;
    fileName: string;
    sourceDir?: string;
    book?: BookItem;
  }) => void;
}

export function useBookDetailDrawerState(options: UseBookDetailDrawerStateOptions) {
  const showDrawer = ref(false);
  const drawerBookUrl = ref("");
  const drawerFileName = ref("");
  const drawerSourceDir = ref("");

  function findSource(fileName: string, sourceDir?: string) {
    return options.sources.value.find(
      (item) => item.fileName === fileName && (!sourceDir || item.sourceDir === sourceDir),
    );
  }

  function openDetail(book: BookItem, fileName: string, sourceDir?: string) {
    options.onOpenDetail?.({
      bookUrl: book.bookUrl,
      fileName,
      sourceDir,
      book,
    });
    drawerBookUrl.value = book.bookUrl;
    drawerFileName.value = fileName;
    drawerSourceDir.value = sourceDir ?? findSource(fileName)?.sourceDir ?? "";
    showDrawer.value = true;
  }

  function openDetailByUrl(bookUrl: string, fileName: string, sourceDir?: string) {
    options.onOpenDetail?.({
      bookUrl,
      fileName,
      sourceDir,
    });
    drawerBookUrl.value = bookUrl;
    drawerFileName.value = fileName;
    drawerSourceDir.value = sourceDir ?? findSource(fileName)?.sourceDir ?? "";
    showDrawer.value = true;
  }

  const drawerSourceName: ComputedRef<string> = computed(() => {
    const source = findSource(drawerFileName.value, drawerSourceDir.value);
    return source?.name ?? drawerFileName.value;
  });

  const drawerSourceType: ComputedRef<string> = computed(() => {
    const source = findSource(drawerFileName.value, drawerSourceDir.value);
    return source?.sourceType ?? "novel";
  });

  return {
    showDrawer,
    drawerBookUrl,
    drawerFileName,
    drawerSourceDir,
    drawerSourceName,
    drawerSourceType,
    openDetail,
    openDetailByUrl,
  };
}
