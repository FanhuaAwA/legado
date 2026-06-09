<script setup lang="ts">
import { ImagePlus, Trash2 } from "lucide-vue-next";
import { computed, ref } from "vue";
import type { PluginSettingImageItem, PluginSettingValue } from "@/composables/useFrontendPlugins";

const props = withDefaults(
  defineProps<{
    value?: PluginSettingValue;
    disabled?: boolean;
    placeholder?: string;
    max?: number;
  }>(),
  {
    disabled: false,
    placeholder: "",
  },
);

const emit = defineEmits<{
  "update:value": [value: PluginSettingImageItem[]];
}>();

const fileInput = ref<HTMLInputElement | null>(null);
const isDragOver = ref(false);
const uploadError = ref("");

const MAX_IMAGE_BYTES = 8 * 1024 * 1024;
const DEFAULT_MAX_IMAGES = 40;

function isSupportedImageFile(file: File): boolean {
  return file.type.startsWith("image/") || /\.(png|jpe?g|webp|gif|bmp|svg)$/i.test(file.name);
}

function isImageItem(value: unknown): value is PluginSettingImageItem {
  if (!value || typeof value !== "object") {
    return false;
  }
  const item = value as Record<string, unknown>;
  return (
    typeof item.id === "string" && typeof item.name === "string" && typeof item.dataUrl === "string"
  );
}

function normalizeImages(value: PluginSettingValue | undefined): PluginSettingImageItem[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((item, index) => {
      if (isImageItem(item)) {
        return item;
      }
      if (typeof item === "string" && item.startsWith("data:image/")) {
        return {
          id: `legacy-${index}`,
          name: `图片 ${index + 1}`,
          dataUrl: item,
        } satisfies PluginSettingImageItem;
      }
      return null;
    })
    .filter((item) => item !== null) as PluginSettingImageItem[];
}

const images = computed(() => normalizeImages(props.value));

function triggerUpload() {
  if (!props.disabled) {
    fileInput.value?.click();
  }
}

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result);
      } else {
        reject(new Error("读取图片失败"));
      }
    };
    reader.onerror = () => reject(new Error("读取图片失败"));
    reader.readAsDataURL(file);
  });
}

function loadImageSize(dataUrl: string): Promise<Pick<PluginSettingImageItem, "width" | "height">> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => resolve({ width: image.naturalWidth, height: image.naturalHeight });
    image.onerror = () => resolve({});
    image.src = dataUrl;
  });
}

async function fileToImageItem(file: File): Promise<PluginSettingImageItem> {
  const dataUrl = await fileToDataUrl(file);
  const size = await loadImageSize(dataUrl);
  return {
    id: `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
    name: file.name,
    dataUrl,
    mime: file.type || undefined,
    size: file.size,
    uploadedAt: Date.now(),
    ...size,
  };
}

async function processFiles(files: FileList | File[]) {
  if (props.disabled) {
    return;
  }
  uploadError.value = "";
  const next = [...images.value];
  const maxImages = props.max ?? DEFAULT_MAX_IMAGES;
  for (const file of Array.from(files)) {
    if (next.length >= maxImages) {
      uploadError.value = `最多上传 ${maxImages} 张图片`;
      break;
    }
    if (!isSupportedImageFile(file)) {
      uploadError.value = "只支持图片文件";
      continue;
    }
    if (file.size > MAX_IMAGE_BYTES) {
      uploadError.value = `${file.name} 超过 8 MB`;
      continue;
    }
    next.push(await fileToImageItem(file));
  }
  emit("update:value", next);
}

function onFileChange(event: Event) {
  const input = event.target as HTMLInputElement;
  if (input.files?.length) {
    void processFiles(input.files);
  }
  input.value = "";
}

function onDragOver(event: DragEvent) {
  if (props.disabled) {
    return;
  }
  event.preventDefault();
  isDragOver.value = true;
}

function onDragLeave() {
  isDragOver.value = false;
}

function onDrop(event: DragEvent) {
  if (props.disabled) {
    return;
  }
  event.preventDefault();
  isDragOver.value = false;
  const files = event.dataTransfer?.files;
  if (files?.length) {
    void processFiles(files);
  }
}

function removeImage(id: string) {
  emit(
    "update:value",
    images.value.filter((image) => image.id !== id),
  );
}

function formatFileSize(size?: number): string {
  if (!size) {
    return "";
  }
  if (size < 1024 * 1024) {
    return `${Math.max(1, Math.round(size / 1024))} KB`;
  }
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

function imageDetail(image: PluginSettingImageItem): string {
  const dimensions = image.width && image.height ? `${image.width} x ${image.height}` : "";
  return [dimensions, formatFileSize(image.size)].filter(Boolean).join(" · ");
}
</script>

<template>
  <div
    class="plugin-image-list"
    :class="{
      'plugin-image-list--dragover': isDragOver,
      'plugin-image-list--disabled': disabled,
    }"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
  >
    <input
      ref="fileInput"
      type="file"
      accept="image/*"
      multiple
      class="plugin-image-list__input"
      @change="onFileChange"
    />

    <div class="plugin-image-list__toolbar">
      <button
        type="button"
        class="plugin-image-list__upload"
        :disabled="disabled"
        @click="triggerUpload"
      >
        <ImagePlus :size="16" />
        <span>选择图片</span>
      </button>
      <span class="plugin-image-list__count">已上传 {{ images.length }}</span>
    </div>

    <div v-if="images.length" class="plugin-image-list__grid">
      <div v-for="image in images" :key="image.id" class="plugin-image-list__item">
        <img class="plugin-image-list__thumb" :src="image.dataUrl" :alt="image.name" />
        <div class="plugin-image-list__meta">
          <span class="plugin-image-list__name" :title="image.name">{{ image.name }}</span>
          <span v-if="imageDetail(image)" class="plugin-image-list__detail">
            {{ imageDetail(image) }}
          </span>
        </div>
        <button
          type="button"
          class="plugin-image-list__delete"
          :disabled="disabled"
          title="删除"
          @click="removeImage(image.id)"
        >
          <Trash2 :size="14" />
        </button>
      </div>
    </div>

    <div v-else class="plugin-image-list__empty">
      {{ placeholder || "支持 PNG、JPG、WebP、GIF、SVG，可拖拽到此上传" }}
    </div>

    <div v-if="uploadError" class="plugin-image-list__error">
      {{ uploadError }}
    </div>
  </div>
</template>

<style scoped>
.plugin-image-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
  border: 1px dashed var(--color-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--color-bg-elevated, #fff) 88%, transparent);
  transition:
    border-color var(--dur-fast, 0.15s) var(--ease-standard, ease),
    background var(--dur-fast, 0.15s) var(--ease-standard, ease);
}

.plugin-image-list--dragover {
  border-color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 8%, transparent);
}

.plugin-image-list--disabled {
  opacity: 0.64;
}

.plugin-image-list__input {
  display: none;
}

.plugin-image-list__toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.plugin-image-list__upload {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 32px;
  padding: 0 12px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-bg-elevated, transparent);
  color: var(--color-text);
  font-size: var(--fs-13, 13px);
  cursor: pointer;
}

.plugin-image-list__upload:disabled {
  cursor: not-allowed;
}

.plugin-image-list__count {
  min-width: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-12, 12px);
}

.plugin-image-list__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
  gap: 8px;
  max-height: 240px;
  overflow: auto;
  padding-right: 2px;
}

.plugin-image-list__item {
  position: relative;
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-bg-elevated, rgba(255, 255, 255, 0.04));
}

.plugin-image-list__thumb {
  display: block;
  width: 100%;
  aspect-ratio: 4 / 3;
  object-fit: cover;
  background: color-mix(in srgb, var(--color-text) 7%, transparent);
}

.plugin-image-list__meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  padding: 6px 30px 7px 8px;
}

.plugin-image-list__name {
  overflow: hidden;
  color: var(--color-text);
  font-size: var(--fs-12, 12px);
  font-weight: var(--fw-semibold, 600);
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.plugin-image-list__detail {
  overflow: hidden;
  color: var(--color-text-muted);
  font-size: 11px;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.plugin-image-list__delete {
  position: absolute;
  right: 6px;
  bottom: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 6px;
  background: color-mix(in srgb, var(--color-danger, #d03050) 14%, transparent);
  color: var(--color-danger, #d03050);
  cursor: pointer;
}

.plugin-image-list__delete:disabled {
  cursor: not-allowed;
}

.plugin-image-list__empty {
  display: flex;
  align-items: center;
  min-height: 72px;
  padding: 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--color-text) 5%, transparent);
  color: var(--color-text-muted);
  font-size: var(--fs-12, 12px);
  line-height: 1.5;
}

.plugin-image-list__error {
  color: var(--color-danger, #d03050);
  font-size: var(--fs-12, 12px);
  line-height: 1.5;
}
</style>
