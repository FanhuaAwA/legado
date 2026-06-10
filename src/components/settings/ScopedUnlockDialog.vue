<!-- ScopedUnlockDialog — 范围化解锁对话框（通用） -->
<script setup lang="ts">
import { useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import { useCapabilities } from "@/composables/useCapabilities";
import { invokeWithTimeout } from "@/composables/useInvoke";
import { useOverlay } from "@/composables/useOverlay";
import { usePreferencesStore } from "@/stores/preferences";

const props = defineProps<{
  show: boolean;
  /** 解锁范围标识，例如 "booksource" */
  scope: string;
  /** 对话框标题，默认为"解锁" */
  title?: string;
}>();

const emit = defineEmits<{
  (e: "update:show", value: boolean): void;
}>();

const message = useMessage();
const prefStore = usePreferencesStore();
const capabilities = useCapabilities();

/** 当前范围是否已解锁（含完全体模式托底） */
const isUnlocked = computed(
  () =>
    prefStore.devTools.unlockedScopes.includes(props.scope) || prefStore.devTools.fullModeEnabled,
);

const dialogTitle = computed(() => props.title ?? "解锁");

const challenge = ref("");
const inputResponse = ref("");
const inputError = ref("");
const loadingChallenge = ref(false);
const verifying = ref(false);

async function refreshChallenge(errorMessage = "") {
  loadingChallenge.value = true;
  challenge.value = "";
  inputResponse.value = "";
  inputError.value = errorMessage;

  try {
    const state = await capabilities.loadCapabilities();
    if (!state.unlock.supported) {
      inputError.value = state.unlock.reason;
      return;
    }

    challenge.value = await invokeWithTimeout<string>("issue_scoped_unlock_challenge", {
      scope: props.scope,
    });
  } catch (error) {
    inputError.value = "挑战码生成失败，请稍后重试";
    message.error(error instanceof Error ? error.message : "挑战码生成失败，请稍后重试");
  } finally {
    loadingChallenge.value = false;
  }
}

function close() {
  emit("update:show", false);
}

async function handleVerify() {
  if (!challenge.value || verifying.value) return;

  verifying.value = true;
  try {
    await capabilities.requireCapability("unlock");

    const verified = await invokeWithTimeout<boolean>("verify_scoped_unlock_challenge", {
      scope: props.scope,
      challenge: challenge.value,
      response: inputResponse.value,
    });

    if (verified) {
      applyUnlock();
      message.success(`${dialogTitle.value}成功`);
      close();
      return;
    }

    await refreshChallenge("验证码错误，请重新计算");
  } catch (error) {
    message.error(error instanceof Error ? error.message : "验证失败，请稍后重试");
  } finally {
    verifying.value = false;
  }
}

function applyUnlock() {
  const cur = prefStore.devTools.unlockedScopes;
  if (!cur.includes(props.scope)) {
    prefStore.patchDevTools({ unlockedScopes: [...cur, props.scope] });
  }
}

function applyRevoke() {
  prefStore.patchDevTools({
    unlockedScopes: prefStore.devTools.unlockedScopes.filter((s) => s !== props.scope),
  });
}

function handleRevoke() {
  applyRevoke();
  message.info(`${dialogTitle.value}已撤销`);
  close();
}

useOverlay(() => props.show, close);

watch(
  () => props.show,
  (v) => {
    if (v) {
      void refreshChallenge();
      return;
    }
    challenge.value = "";
    inputResponse.value = "";
    inputError.value = "";
  },
);
</script>

<template>
  <n-modal
    :show="show"
    preset="card"
    :title="dialogTitle"
    :style="{ width: '400px', maxWidth: '92vw' }"
    :bordered="false"
    :segmented="{ content: true, footer: true }"
    @update:show="(v: boolean) => emit('update:show', v)"
  >
    <template v-if="!isUnlocked">
      <div class="sud-body">
        <p class="sud-desc">请使用以下挑战码计算验证码后输入：</p>

        <div class="sud-challenge-box">
          <span class="sud-challenge-box__label">挑战码</span>
          <strong class="sud-challenge-box__value">{{
            loadingChallenge ? "生成中..." : challenge
          }}</strong>
        </div>

        <n-input
          v-model:value="inputResponse"
          placeholder="输入 6 位验证码"
          maxlength="6"
          :status="inputError ? 'error' : undefined"
          :disabled="loadingChallenge || verifying || !challenge"
          class="sud-input"
          @keydown.enter="handleVerify"
        />
        <p v-if="inputError" class="sud-error">{{ inputError }}</p>
      </div>
    </template>

    <template v-else>
      <div class="sud-body">
        <p class="sud-active">「{{ dialogTitle }}」已激活。</p>
      </div>
    </template>

    <template #footer>
      <div class="sud-footer">
        <n-button @click="close">取消</n-button>
        <template v-if="!isUnlocked">
          <n-button
            type="primary"
            :loading="verifying"
            :disabled="loadingChallenge || !challenge || !inputResponse.trim()"
            @click="handleVerify"
          >
            验证
          </n-button>
        </template>
        <template v-else>
          <n-button type="warning" @click="handleRevoke">撤销解锁</n-button>
        </template>
      </div>
    </template>
  </n-modal>
</template>

<style scoped>
.sud-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.sud-desc {
  margin: 0;
  font-size: 0.88rem;
  color: var(--color-text-soft);
}

.sud-challenge-box {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: var(--color-fill);
  border-radius: var(--radius-md, 8px);
}

.sud-challenge-box__label {
  font-size: 0.8rem;
  color: var(--color-text-muted);
  white-space: nowrap;
}

.sud-challenge-box__value {
  font-family: monospace;
  font-size: 1.5rem;
  letter-spacing: 0.25em;
  color: var(--color-accent, var(--color-primary));
}

.sud-error {
  margin: 0;
  font-size: 0.82rem;
  color: var(--color-error, #e74c3c);
}

.sud-active {
  margin: 0;
  font-size: 0.9rem;
  color: var(--color-text-soft);
}

.sud-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
