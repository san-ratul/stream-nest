<script setup>
import { ref, onMounted, onUnmounted } from "vue";
import {
  playerState,
  playerInit,
  playerDestroy,
  playerLoad,
  playerTogglePause,
  playerSeek,
  playerSetVolume,
  playerGetHwdec,
} from "../composables/usePlayer";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps({
  videoPath: { type: String, default: "" },
});

const volume = ref(100);
let probeTimer = null;

onMounted(() => {
  playerInit().then(() => {
    if (props.videoPath) playerLoad(props.videoPath);
  });
  probeTimer = setInterval(() => {
    playerGetHwdec();
    invoke("dev_log", {
      message: `probe time=${playerState.timePos} dur=${playerState.duration} hwdec=${playerState.hwdec} fmt=${playerState.videoFormat}`,
    }).catch(() => {});
  }, 1000);
});

onUnmounted(() => {
  clearInterval(probeTimer);
  playerDestroy();
});

function onSeek(e) {
  const t = Number(e.target.value);
  if (playerState.duration) playerSeek(t);
}

function onVolume(e) {
  volume.value = Number(e.target.value);
  playerSetVolume(volume.value);
}

function fmt(t) {
  if (t == null) return "--:--";
  const m = Math.floor(t / 60);
  const s = Math.floor(t % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
</script>

<template>
  <div class="flex h-full flex-col justify-end" data-player-surface>
    <div class="absolute top-3 left-4 flex gap-4 text-xs text-slate-300 drop-shadow">
      <span>hwdec: {{ playerState.hwdec ?? "?" }}</span>
      <span>codec: {{ playerState.videoFormat ?? "?" }}</span>
      <span v-if="playerState.error" class="text-red-400">{{ playerState.error }}</span>
    </div>

    <div
      class="bg-slate-900/85 px-4 py-3 backdrop-blur-sm"
      data-player-controls
    >
      <input
        type="range"
        class="w-full accent-sky-400"
        min="0"
        :max="playerState.duration ?? 0"
        step="0.5"
        :value="playerState.timePos ?? 0"
        @change="onSeek"
      />
      <div class="mt-2 flex items-center gap-4">
        <button
          class="rounded bg-sky-500 px-4 py-1 text-sm font-medium text-slate-950 hover:bg-sky-400"
          @click="playerTogglePause"
        >
          {{ playerState.paused ? "Play" : "Pause" }}
        </button>
        <button
          class="rounded bg-slate-700 px-3 py-1 text-sm text-slate-200 hover:bg-slate-600"
          @click="playerSeek(Math.max(0, (playerState.timePos ?? 0) - 10))"
        >
          -10s
        </button>
        <button
          class="rounded bg-slate-700 px-3 py-1 text-sm text-slate-200 hover:bg-slate-600"
          @click="playerSeek(Math.min(playerState.duration ?? 0, (playerState.timePos ?? 0) + 10))"
        >
          +10s
        </button>
        <span class="text-sm text-slate-300">
          {{ fmt(playerState.timePos) }} / {{ fmt(playerState.duration) }}
        </span>
        <div class="ml-auto flex items-center gap-2">
          <span class="text-xs text-slate-400">Vol</span>
          <input
            type="range"
            class="w-24 accent-sky-400"
            min="0"
            max="130"
            :value="volume"
            @input="onVolume"
          />
        </div>
      </div>
    </div>
  </div>
</template>
