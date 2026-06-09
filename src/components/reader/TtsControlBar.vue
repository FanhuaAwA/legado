<script setup lang="ts">
import { Loader2, Pause, Play, SkipBack, SkipForward, Volume2, X } from "lucide-vue-next";
import { computed, onMounted, watch } from "vue";
import { useTts } from "@/composables/useTts";

const props = defineProps<{
  visible: boolean;
  progressText?: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const tts = useTts();

const RATE_OPTIONS = [
  { label: "0.75x", value: 0.75 },
  { label: "1x", value: 1 },
  { label: "1.25x", value: 1.25 },
  { label: "1.5x", value: 1.5 },
  { label: "2x", value: 2 },
];

const progressValue = computed({
  get: () => tts.currentSegmentIndex.value,
  set: (value: number) => tts.seekToIndex(value),
});

const progressMax = computed(() => Math.max(tts.totalSegmentsKnown.value - 1, 0));
const progressPercent = computed(() => Math.round(tts.progressRatio.value * 100));
const progressCountText = computed(() => {
  if (tts.totalSegmentsKnown.value <= 0) {
    return "0/0";
  }
  const suffix = tts.totalFinalized.value ? "" : "+";
  return `${tts.currentSegmentOrdinal.value}/${tts.totalSegmentsKnown.value}${suffix}`;
});
const engineTitle = computed(
  () => tts.selectedEngine.value.description || tts.selectedEngine.value.name,
);

function handleClose() {
  tts.stop();
  emit("close");
}

function togglePlayPause() {
  if (tts.isPlaying.value) {
    tts.pause();
  } else {
    tts.play();
  }
}

function handleEngineChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  void tts.setEngineId(target.value);
}

function handleVoiceChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  tts.setVoiceId(target.value);
}

onMounted(() => {
  void tts.refreshEngines();
  void tts.loadVoices();
});

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      void tts.refreshEngines();
      void tts.loadVoices();
    }
  },
);
</script>

<template>
  <Transition name="tts-bar">
    <section v-if="visible" class="tts-control-bar" aria-label="朗读控制">
      <div class="tts-control-bar__main">
        <button
          class="tts-control-bar__icon-btn"
          type="button"
          title="上一段"
          :disabled="!tts.hasSession.value || tts.currentSegmentIndex.value <= 0"
          @click="tts.prevSegment()"
        >
          <SkipBack :size="18" aria-hidden="true" />
        </button>

        <button
          class="tts-control-bar__play-btn"
          type="button"
          :title="tts.isPlaying.value ? '暂停' : '播放'"
          :disabled="!tts.hasSession.value"
          @click="togglePlayPause"
        >
          <Loader2 v-if="tts.isLoading.value" class="tts-control-bar__spin" :size="20" />
          <Pause v-else-if="tts.isPlaying.value" :size="20" aria-hidden="true" />
          <Play v-else :size="20" aria-hidden="true" />
        </button>

        <button
          class="tts-control-bar__icon-btn"
          type="button"
          title="下一段"
          :disabled="
            !tts.hasSession.value ||
            (tts.totalFinalized.value && tts.currentSegmentIndex.value >= progressMax)
          "
          @click="tts.nextSegment()"
        >
          <SkipForward :size="18" aria-hidden="true" />
        </button>

        <div class="tts-control-bar__progress-block">
          <div class="tts-control-bar__progress-meta">
            <span class="tts-control-bar__location">{{ progressText || "—" }}</span>
            <span class="tts-control-bar__count"
              >{{ progressCountText }} · {{ progressPercent }}%</span
            >
          </div>
          <input
            v-model.number="progressValue"
            class="tts-control-bar__progress"
            type="range"
            min="0"
            :max="progressMax"
            step="1"
            :disabled="!tts.hasSession.value || tts.totalSegmentsKnown.value <= 1"
            aria-label="朗读进度"
          />
        </div>

        <button
          class="tts-control-bar__icon-btn tts-control-bar__close"
          type="button"
          title="关闭朗读"
          @click="handleClose"
        >
          <X :size="18" aria-hidden="true" />
        </button>
      </div>

      <div class="tts-control-bar__options">
        <label class="tts-control-bar__select-wrap" :title="engineTitle">
          <Volume2 :size="15" aria-hidden="true" />
          <select
            class="tts-control-bar__select"
            :value="tts.selectedEngineId.value"
            aria-label="朗读引擎"
            @change="handleEngineChange"
          >
            <option
              v-for="engine in tts.availableEngines.value"
              :key="engine.id"
              :value="engine.id"
            >
              {{ engine.name }}
            </option>
          </select>
        </label>

        <select
          class="tts-control-bar__select tts-control-bar__voice"
          :value="tts.selectedVoiceId.value"
          :disabled="tts.voicesLoading.value || tts.availableVoices.value.length === 0"
          aria-label="朗读语音"
          @change="handleVoiceChange"
        >
          <option value="">默认语音</option>
          <option v-for="voice in tts.availableVoices.value" :key="voice.id" :value="voice.id">
            {{ voice.name }}{{ voice.language ? ` · ${voice.language}` : "" }}
          </option>
        </select>

        <div class="tts-control-bar__rates" role="group" aria-label="朗读速度">
          <button
            v-for="option in RATE_OPTIONS"
            :key="option.value"
            class="tts-control-bar__rate-btn"
            type="button"
            :class="{
              'tts-control-bar__rate-btn--active': tts.playbackRate.value === option.value,
            }"
            @click="tts.setPlaybackRate(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
      </div>

      <div v-if="tts.error.value" class="tts-control-bar__error" role="status">
        {{ tts.error.value }}
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.tts-control-bar {
  position: fixed;
  left: 50%;
  bottom: calc(72px + max(12px, var(--safe-area-inset-bottom, env(safe-area-inset-bottom, 0px))));
  z-index: 80;
  width: min(680px, calc(100vw - 24px));
  display: grid;
  gap: 8px;
  padding: 10px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  background: rgba(24, 26, 30, 0.94);
  color: #f3f4f6;
  box-shadow: 0 12px 34px rgba(0, 0, 0, 0.34);
  backdrop-filter: blur(18px);
  transform: translateX(-50%);
}

.tts-control-bar__main,
.tts-control-bar__options {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.tts-control-bar__main {
  display: grid;
  grid-template-columns: 34px 42px 34px minmax(0, 1fr) 34px;
}

.tts-control-bar__icon-btn,
.tts-control-bar__play-btn,
.tts-control-bar__rate-btn {
  border: 0;
  color: inherit;
  cursor: pointer;
  transition:
    background 0.15s ease,
    opacity 0.15s ease,
    transform 0.15s ease;
}

.tts-control-bar__icon-btn,
.tts-control-bar__play-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.08);
}

.tts-control-bar__play-btn {
  width: 42px;
  height: 42px;
  background: #39c98d;
  color: #071913;
}

.tts-control-bar__icon-btn:not(:disabled):hover,
.tts-control-bar__rate-btn:not(:disabled):hover {
  background: rgba(255, 255, 255, 0.16);
}

.tts-control-bar__play-btn:not(:disabled):hover {
  background: #5ae0a9;
  transform: translateY(-1px);
}

.tts-control-bar__icon-btn:disabled,
.tts-control-bar__play-btn:disabled {
  opacity: 0.38;
  cursor: default;
}

.tts-control-bar__close {
  color: #d1d5db;
}

.tts-control-bar__progress-block {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.tts-control-bar__progress-meta {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
  font-size: 12px;
  line-height: 1.2;
}

.tts-control-bar__location {
  overflow: hidden;
  color: #f9fafb;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tts-control-bar__count {
  flex: 0 0 auto;
  color: #a7f3d0;
  font-variant-numeric: tabular-nums;
}

.tts-control-bar__progress {
  width: 100%;
  height: 18px;
  margin: 0;
  accent-color: #39c98d;
}

.tts-control-bar__options {
  flex-wrap: wrap;
}

.tts-control-bar__select-wrap {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 150px;
  height: 32px;
  padding: 0 8px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.08);
}

.tts-control-bar__select {
  min-width: 0;
  height: 32px;
  border: 0;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.08);
  color: #f3f4f6;
  font-size: 12px;
  outline: none;
}

.tts-control-bar__select-wrap .tts-control-bar__select {
  flex: 1;
  height: auto;
  background: transparent;
}

.tts-control-bar__select option {
  color: #111827;
}

.tts-control-bar__voice {
  flex: 1 1 180px;
  padding: 0 8px;
}

.tts-control-bar__rates {
  display: inline-flex;
  gap: 2px;
  height: 32px;
  padding: 2px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.08);
}

.tts-control-bar__rate-btn {
  min-width: 44px;
  padding: 0 8px;
  border-radius: 4px;
  background: transparent;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}

.tts-control-bar__rate-btn--active {
  background: #39c98d;
  color: #071913;
}

.tts-control-bar__error {
  overflow: hidden;
  padding: 6px 8px;
  border-radius: 6px;
  background: rgba(220, 38, 38, 0.18);
  color: #fecaca;
  font-size: 12px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tts-control-bar__spin {
  animation: tts-spin 0.9s linear infinite;
}

.tts-bar-enter-active,
.tts-bar-leave-active {
  transition:
    opacity 0.18s ease,
    transform 0.18s ease;
}

.tts-bar-enter-from,
.tts-bar-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(10px);
}

@keyframes tts-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 620px) {
  .tts-control-bar {
    bottom: calc(76px + max(8px, var(--safe-area-inset-bottom, env(safe-area-inset-bottom, 0px))));
    width: calc(100vw - 16px);
    padding: 8px;
  }

  .tts-control-bar__main {
    grid-template-columns: 32px 40px 32px minmax(0, 1fr) 32px;
    gap: 6px;
  }

  .tts-control-bar__select-wrap,
  .tts-control-bar__voice,
  .tts-control-bar__rates {
    flex: 1 1 100%;
  }

  .tts-control-bar__rates {
    justify-content: space-between;
  }

  .tts-control-bar__rate-btn {
    flex: 1;
  }
}
</style>
