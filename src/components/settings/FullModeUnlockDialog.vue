<!-- FullModeUnlockDialog — 完全体模式 挑战码解锁对话框 -->
<script setup lang="ts">
import { useMessage } from "naive-ui";
import { storeToRefs } from "pinia";
import { ref, watch } from "vue";
import { invokeWithTimeout } from "@/composables/useInvoke";
import { useOverlay } from "@/composables/useOverlay";
import { usePreferencesStore } from "@/stores/preferences";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{
  (e: "update:show", value: boolean): void;
}>();

const message = useMessage();
const prefStore = usePreferencesStore();
const { devTools } = storeToRefs(prefStore);

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
    challenge.value = await invokeWithTimeout<string>("issue_full_mode_challenge");
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
  if (!challenge.value || verifying.value) {
    return;
  }

  verifying.value = true;
  try {
    const verified = await invokeWithTimeout<boolean>("verify_full_mode_challenge", {
      challenge: challenge.value,
      response: inputResponse.value,
    });

    if (verified) {
      prefStore.patchDevTools({ fullModeEnabled: true });
      message.success("完全体模式已激活");
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

function handleRevoke() {
  prefStore.patchDevTools({ fullModeEnabled: false });
  message.info("完全体模式已关闭");
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
    title="解除限制"
    :style="{ width: '400px', maxWidth: '92vw' }"
    :bordered="false"
    :segmented="{ content: true, footer: true }"
    @update:show="(v: boolean) => emit('update:show', v)"
  >
    <template v-if="!devTools.fullModeEnabled">
      <div class="fmu-body">
        <p class="fmu-desc">请使用以下挑战码计算验证码后输入：</p>

        <div class="fmu-challenge-box">
          <span class="fmu-challenge-box__label">挑战码</span>
          <strong class="fmu-challenge-box__value">{{
            loadingChallenge ? "生成中..." : challenge
          }}</strong>
        </div>

        <n-input
          v-model:value="inputResponse"
          placeholder="输入 6 位验证码"
          maxlength="6"
          :status="inputError ? 'error' : undefined"
          :disabled="loadingChallenge || verifying || !challenge"
          class="fmu-input"
          @keydown.enter="handleVerify"
        />
        <p v-if="inputError" class="fmu-error">{{ inputError }}</p>
      </div>
    </template>

    <template v-else>
      <div class="fmu-body">
        <p class="fmu-active">完全体模式已激活，限制已解除。</p>
      </div>
    </template>

    <template #footer>
      <div class="fmu-footer">
        <n-button @click="close">取消</n-button>
        <template v-if="!devTools.fullModeEnabled">
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
          <n-button type="warning" @click="handleRevoke">撤销完全体模式</n-button>
        </template>
      </div>
    </template>
  </n-modal>
</template>

<style scoped>
.fmu-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.fmu-desc {
  margin: 0;
  font-size: 0.88rem;
  color: var(--color-text-soft);
}

.fmu-challenge-box {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: var(--color-fill);
  border-radius: var(--radius-md, 8px);
}

.fmu-challenge-box__label {
  font-size: 0.8rem;
  color: var(--color-text-muted);
  white-space: nowrap;
}

.fmu-challenge-box__value {
  font-family: monospace;
  font-size: 1.5rem;
  letter-spacing: 0.25em;
  color: var(--color-accent, var(--color-primary));
}

.fmu-hint {
  margin: 0;
  font-size: 0.8rem;
  color: var(--color-text-muted);
}

.fmu-formula {
  margin: 0;
  padding: 10px 12px;
  font-size: 0.73rem;
  line-height: 1.6;
  background: var(--color-fill);
  border-radius: 6px;
  overflow-x: auto;
  color: var(--color-text-soft);
  white-space: pre;
}

.fmu-error {
  margin: 0;
  font-size: 0.82rem;
  color: var(--color-error, #e74c3c);
}

.fmu-active {
  margin: 0;
  font-size: 0.9rem;
  color: var(--color-text-soft);
}

.fmu-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
