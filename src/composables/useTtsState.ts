import { readonly, ref } from "vue";

export const ttsIsPlaying = ref(false);

export function useTtsPlaybackState() {
  return {
    isPlaying: readonly(ttsIsPlaying),
  };
}
