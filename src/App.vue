<script setup>
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import PlayerPage from "./pages/PlayerPage.vue";

const view = ref("home");
const status = ref("checking backend...");
const engineReady = ref(false);
const engineError = ref(null);
const testVideoPath = ref("E:\\streamnest\\.testmedia\\bbb_1080_10s.mp4");

async function checkHealth() {
  try {
    const res = await invoke("health");
    status.value = `backend: ${res}`;
    pollEngine();
  } catch (err) {
    status.value = `backend error: ${err}`;
  }
}

function pollEngine() {
  const tick = async () => {
    try {
      const s = await invoke("engine_status");
      engineReady.value = s.ready;
      engineError.value = s.error;
    } catch {
      engineReady.value = false;
    }
  };
  tick();
  setInterval(tick, 1000);
}

checkHealth();

const playerActive = computed(() => view.value === "player");
</script>

<template>
  <div class="relative h-full w-full">
    <div
      v-if="!playerActive"
      class="absolute inset-0 bg-[#0b0f17]"
    ></div>

    <div v-if="view === 'home'" class="relative flex h-full flex-col items-center justify-center gap-4">
      <h1 class="text-4xl font-bold tracking-tight text-sky-400">StreamNest</h1>
      <p class="text-sm text-slate-400">{{ status }}</p>
      <p class="text-xs" :class="engineReady ? 'text-emerald-400' : 'text-amber-400'">
        torrent engine: {{ engineReady ? "ready" : engineError ? engineError : "initializing..." }}
      </p>
      <button
        class="rounded bg-sky-500 px-5 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400"
        @click="view = 'player'"
      >
        Open player (M3 verification)
      </button>
    </div>

    <div v-else class="relative h-full w-full">
      <PlayerPage :video-path="testVideoPath" />
      <button
        class="absolute top-3 right-4 rounded bg-slate-800/90 px-3 py-1 text-xs text-slate-300 hover:bg-slate-700"
        @click="view = 'home'"
      >
        Back
      </button>
    </div>
  </div>
</template>
