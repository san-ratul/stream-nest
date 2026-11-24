<script setup>
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import PlayerPage from "./pages/PlayerPage.vue";
import { playerState, playerSeek } from "./composables/usePlayer";

const view = ref("home");
const status = ref("checking backend...");
const engineReady = ref(false);
const engineError = ref(null);
const localVideoPath = ref("E:\\streamnest\\.testmedia\\bbb_1080_10s.mp4");
const e2eSrc = ref(null);
const e2eBusy = ref(false);
const e2eLog = ref([]);
const isDev = import.meta.env.DEV;

if (isDev) {
  invoke("dev_log", { message: "APP_MOUNTED isDev=true" }).catch(() => {});
}

let ranE2e = false;
const e2eAutoEnabled = ref(isDev);
checkHealth();

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
      if (s.ready && isDev && !ranE2e && e2eAutoEnabled.value) {
        ranE2e = true;
        setTimeout(() => {
          runE2e();
        }, 2000);
      }
    } catch {
      engineReady.value = false;
    }
  };
  tick();
  setInterval(tick, 1000);
}

const playerActive = computed(() => view.value === "player");

function addLog(line) {
  const ts = new Date().toISOString().slice(11, 23);
  e2eLog.value.unshift(`[${ts}] ${line}`);
  invoke("dev_log", { message: `E2E ${line}` }).catch(() => {});
}

async function runE2e() {
  e2eBusy.value = true;
  try {
    addLog("reading test torrent");
    const b64 = await invoke("dev_read_test_torrent", {
      name: "Sintel.2010.1080p.mkv.torrent",
    });
    addLog("adding torrent with initial peer 127.0.0.1:6900");
    const res = await invoke("torrent_add_file_base64", {
      contents: b64,
      peers: ["127.0.0.1:6900"],
    });
    const id = res.id;
    addLog(`torrent added id=${id}`);
    const details = await invoke("torrent_details", { id });
    const files = details?.files ?? details?.meta?.files ?? [];
    addLog(`files=${files.length ? JSON.stringify(files.map((f) => f.name)) : "metadata pending"}`);
    const url = await invoke("stream_url", { id, fileId: 0 });
    addLog(`stream url ready`);
    e2eSrc.value = url;
    view.value = "player";
  } catch (err) {
    addLog(`ERROR ${err}`);
  } finally {
    e2eBusy.value = false;
  }
}

function playLocal() {
  e2eSrc.value = null;
  view.value = "player";
}

let seeksDone = 0;
watch(
  () => playerState.firstFrameMs,
  (v) => {
    if (v != null && e2eSrc.value && seeksDone === 0) {
      seeksDone = 1;
      const dur = playerState.duration ?? 900;
      setTimeout(() => {
        const t1 = Math.floor(dur * 0.35);
        addLog(`auto-seek 1 -> ${t1}s (35%)`);
        playerSeek(t1);
        setTimeout(() => {
          const t2 = Math.floor(dur * 0.7);
          addLog(`auto-seek 2 -> ${t2}s (70%)`);
          playerSeek(t2);
        }, 15000);
      }, 8000);
    }
  }
);
</script>

<template>
  <div class="relative h-full w-full">
    <div v-if="!playerActive" class="absolute inset-0 bg-[#0b0f17]"></div>

    <div v-if="view === 'home'" class="relative flex h-full flex-col items-center justify-center gap-4">
      <h1 class="text-4xl font-bold tracking-tight text-sky-400">StreamNest</h1>
      <p class="text-sm text-slate-400">{{ status }}</p>
      <p class="text-xs" :class="engineReady ? 'text-emerald-400' : 'text-amber-400'">
        torrent engine: {{ engineReady ? "ready" : engineError ? engineError : "initializing..." }}
      </p>
      <div class="flex gap-3">
        <button
          class="rounded bg-sky-500 px-5 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400"
          @click="playLocal"
        >
          Play local test video
        </button>
        <button
          class="rounded bg-slate-700 px-5 py-2 text-sm font-medium text-slate-200 hover:bg-slate-600"
          @click="view = 'player'"
        >
          Open empty player
        </button>
      </div>

      <div v-if="isDev" class="mt-6 w-[520px] rounded-lg border border-slate-700 bg-slate-800/60 p-4">
        <div class="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-400">
          Dev E2E harness (local seeder :6900)
        </div>
        <button
          class="rounded bg-emerald-500 px-4 py-1.5 text-sm font-medium text-slate-950 hover:bg-emerald-400 disabled:opacity-50"
          :disabled="e2eBusy || !engineReady"
          @click="runE2e"
        >
          {{ e2eBusy ? "Running..." : "Run torrent streaming E2E" }}
        </button>
        <div v-if="e2eLog.length" class="mt-3 max-h-32 overflow-y-auto font-mono text-[11px] leading-4 text-slate-300">
          <div v-for="(line, i) in e2eLog" :key="i">{{ line }}</div>
        </div>
      </div>
    </div>

    <div v-else class="relative h-full w-full">
      <PlayerPage :src="e2eSrc ?? localVideoPath" />
      <button
        class="absolute top-3 right-4 rounded bg-slate-800/90 px-3 py-1 text-xs text-slate-300 hover:bg-slate-700"
        @click="view = 'home'"
      >
        Back
      </button>
    </div>
  </div>
</template>
