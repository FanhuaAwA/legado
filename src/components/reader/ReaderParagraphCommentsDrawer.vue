<script setup lang="ts">
import { NAvatar, NButton, NEmpty, NInput, NSpin, NTag, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import AppDrawer from "@/components/base/AppDrawer.vue";
import {
  getParagraphCommentCapabilities,
  normalizeParagraphCommentDetailPage,
  type ParagraphCommentActionCapabilities,
  type ParagraphCommentDetail,
  type ParagraphCommentTarget,
} from "@/features/reader/services/readerParagraphComments";
import { useBookSourceStore, useScriptBridgeStore } from "@/stores";

const props = defineProps<{
  show: boolean;
  target: ParagraphCommentTarget | null;
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
}>();

const message = useMessage();
const bookSourceStore = useBookSourceStore();
const scriptBridgeStore = useScriptBridgeStore();

const loading = ref(false);
const submittingReply = ref(false);
const error = ref("");
const comments = ref<ParagraphCommentDetail[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 20;
const replyContent = ref("");
const capabilities = ref<ParagraphCommentActionCapabilities>({
  counts: false,
  details: false,
  like: false,
  reply: false,
});

const title = computed(() => {
  const count = props.target?.count ?? total.value;
  return count > 0 ? `段评 ${count}` : "段评";
});

const hasMore = computed(() => comments.value.length < total.value);
const canLike = computed(() => capabilities.value.like);
const canReply = computed(() => capabilities.value.reply);

function onUpdateShow(value: boolean) {
  emit("update:show", value);
}

async function refreshCapabilities() {
  const target = props.target;
  if (!target) {
    capabilities.value = {
      counts: false,
      details: false,
      like: false,
      reply: false,
    };
    return;
  }
  await bookSourceStore.ensureCapsLoaded();
  const sourceCapabilities =
    bookSourceStore.getCachedCapabilities(target.fileName, target.sourceDir) ??
    (await bookSourceStore.detectCapabilities(target.fileName, target.sourceDir));
  capabilities.value = getParagraphCommentCapabilities(sourceCapabilities);
}

async function loadComments(reset = true) {
  const target = props.target;
  if (!target) {
    comments.value = [];
    total.value = 0;
    return;
  }
  if (reset) {
    page.value = 1;
    comments.value = [];
  }
  loading.value = true;
  error.value = "";
  try {
    await refreshCapabilities();
    if (!capabilities.value.details) {
      comments.value = [];
      total.value = target.count;
      return;
    }
    const raw = await scriptBridgeStore.runChapterParagraphComments(
      target.fileName,
      target.chapterUrl,
      target.key,
      { page: page.value, pageSize },
      target.sourceDir,
    );
    const detailPage = normalizeParagraphCommentDetailPage(raw);
    total.value = detailPage.total || target.count;
    comments.value = reset ? detailPage.comments : [...comments.value, ...detailPage.comments];
  } catch (loadError) {
    error.value = loadError instanceof Error ? loadError.message : String(loadError);
  } finally {
    loading.value = false;
  }
}

async function loadMore() {
  if (loading.value || !hasMore.value) {
    return;
  }
  page.value += 1;
  await loadComments(false);
}

async function toggleLike(comment: ParagraphCommentDetail) {
  const target = props.target;
  if (!target || !canLike.value) {
    return;
  }
  const nextLiked = !comment.liked;
  try {
    await scriptBridgeStore.likeParagraphComment(
      target.fileName,
      target.chapterUrl,
      target.key,
      comment.id,
      nextLiked,
      target.sourceDir,
    );
    comment.liked = nextLiked;
    comment.likeCount = Math.max(0, comment.likeCount + (nextLiked ? 1 : -1));
  } catch (likeError) {
    message.error(likeError instanceof Error ? likeError.message : String(likeError));
  }
}

async function submitReply() {
  const target = props.target;
  const content = replyContent.value.trim();
  const rootComment = comments.value[0];
  if (!target || !rootComment || !content || !canReply.value) {
    return;
  }
  submittingReply.value = true;
  try {
    await scriptBridgeStore.replyParagraphComment(
      target.fileName,
      target.chapterUrl,
      target.key,
      rootComment.id,
      content,
      target.sourceDir,
    );
    replyContent.value = "";
    await loadComments(true);
  } catch (replyError) {
    message.error(replyError instanceof Error ? replyError.message : String(replyError));
  } finally {
    submittingReply.value = false;
  }
}

function formatCommentTime(value?: string): string {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function normalizeTagType(type?: string) {
  const allowed = ["default", "primary", "info", "success", "warning", "error"] as const;
  return allowed.includes(type as (typeof allowed)[number])
    ? (type as (typeof allowed)[number])
    : "default";
}

watch(
  () => [props.show, props.target?.key, props.target?.chapterUrl] as const,
  ([show]) => {
    if (!show || !props.target) {
      return;
    }
    void loadComments(true);
  },
);
</script>

<template>
  <AppDrawer
    :show="show"
    :title="title"
    placement="bottom"
    height="min(72vh, 620px)"
    @update:show="onUpdateShow"
  >
    <div class="paragraph-comments">
      <div class="paragraph-comments__meta">
        <span>{{ target?.chapterName }}</span>
        <span v-if="target">第 {{ target.start + 1 }} 段</span>
      </div>

      <div v-if="loading && comments.length === 0" class="paragraph-comments__center">
        <n-spin size="small" />
      </div>
      <n-empty v-else-if="error" :description="error" />
      <n-empty v-else-if="comments.length === 0" description="暂无段评" />

      <div v-else class="paragraph-comments__list">
        <article v-for="comment in comments" :key="comment.id" class="paragraph-comments__item">
          <n-avatar round :size="36" :src="comment.avatarUrl">
            {{ comment.nickname.slice(0, 1) }}
          </n-avatar>
          <div class="paragraph-comments__content">
            <div class="paragraph-comments__author-row">
              <span class="paragraph-comments__nickname">{{ comment.nickname }}</span>
              <n-tag
                v-for="tag in comment.tags"
                :key="`${comment.id}:${tag.label}`"
                size="small"
                :bordered="false"
                :type="normalizeTagType(tag.type)"
                :color="tag.color ? { color: tag.color, textColor: '#fff' } : undefined"
              >
                {{ tag.label }}
              </n-tag>
            </div>
            <p class="paragraph-comments__text">{{ comment.content }}</p>
            <div class="paragraph-comments__footer-row">
              <span>{{ formatCommentTime(comment.createdAt) }}</span>
              <button
                class="paragraph-comments__like"
                type="button"
                :disabled="!canLike"
                :class="{ 'paragraph-comments__like--active': comment.liked }"
                @click="toggleLike(comment)"
              >
                {{ comment.liked ? "已赞" : "点赞" }} {{ comment.likeCount }}
              </button>
            </div>
          </div>
        </article>
        <n-button v-if="hasMore" text :loading="loading" @click="loadMore">加载更多</n-button>
      </div>
    </div>

    <template #footer>
      <div class="paragraph-comments__reply">
        <n-input
          v-model:value="replyContent"
          type="textarea"
          size="small"
          :autosize="{ minRows: 1, maxRows: 3 }"
          :disabled="!canReply"
          :placeholder="canReply ? '写下回复' : '书源未提供回复接口'"
        />
        <n-button
          type="primary"
          size="small"
          :loading="submittingReply"
          :disabled="!canReply || !replyContent.trim()"
          @click="submitReply"
        >
          回复
        </n-button>
      </div>
    </template>
  </AppDrawer>
</template>

<style scoped>
.paragraph-comments {
  min-height: 180px;
}

.paragraph-comments__meta {
  display: flex;
  gap: 10px;
  margin-bottom: 12px;
  color: var(--text-color-3);
  font-size: 12px;
}

.paragraph-comments__center {
  display: flex;
  min-height: 160px;
  align-items: center;
  justify-content: center;
}

.paragraph-comments__list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.paragraph-comments__item {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr);
  gap: 10px;
}

.paragraph-comments__content {
  min-width: 0;
}

.paragraph-comments__author-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.paragraph-comments__nickname {
  font-weight: 600;
}

.paragraph-comments__text {
  margin: 6px 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.paragraph-comments__footer-row {
  display: flex;
  gap: 12px;
  align-items: center;
  color: var(--text-color-3);
  font-size: 12px;
}

.paragraph-comments__like {
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.paragraph-comments__like:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.paragraph-comments__like--active {
  color: var(--primary-color);
}

.paragraph-comments__reply {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  align-items: end;
}
</style>
