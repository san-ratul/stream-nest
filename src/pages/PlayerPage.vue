<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  playerState,
  playerInit,
  playerDestroy,
  playerLoad,
  playerTogglePause,
  playerSeek,
  playerSetVolume,
  playerGetHwdec,
  playerRenderDiagnostics,
  playerGetTrackList,
  playerSetAudioTrack,
  playerSetSubtitleTrack,
  playerSetSpeed,
  playerSetAspect,
  playerSetMute,
  playerAddExternalSubtitle,
  playerSetProperty,
  playerReinit,
} from "../composables/usePlayer";
import { invoke } from "@tauri-apps/api/core";
import { playerStore, mediaKey } from "../stores/player";
import { torrentStore, fmtSpeed, fmtBytes } from "../stores/torrents";
import AppState from "../components/common/AppState.vue";

const volume = ref(100);
const muted = ref(false);
const fullscreen = ref(false);
const speed = ref(1);
const aspect = ref("auto");
const tracks = ref([]);
const subtitleFile = ref(null);
const controlError = ref(null);
const resumeNote = ref(null);
const fixApplied = ref(null);
let probeTimer = null;
let lastTime = null;
let stallSeconds = 0;
let lastSaveSeconds = 0;
let resumed = false;

const appWindow = getCurrentWebviewWindow();

const audioTracks = computed(() => tracks.value.filter((t) => t.type === "audio"));
const subtitleTracks = computed(() => tracks.value.filter((t) => t.type === "sub"));

const sourceTorrent = computed(
  () =>
    torrentStore.torrents.find(
      (t) => String(t.id) === String(playerStore.torrentId) || t.info_hash === playerStore.torrentId
    ) ?? null
);
const downloadSpeed = computed(() => sourceTorrent.value?.stats?.live?.downloaded ?? 0);

const SPEEDS = [0.5, 1, 1.25, 1.5, 2];
const ASPECTS = [
  { label: "auto", value: "-1" },
  { label: "16:9", value: "16:9" },
  { label: "4:3", value: "4:3" },
  { label: "2.35:1", value: "2.35" },
];

function currentMediaKey() {
  return mediaKey(playerStore.infoHash, playerStore.fileId);
}

async function maybeResume() {
  if (resumed || !currentMediaKey()) return;
  try {
    const rows = await invoke("history_list", { limit: 100 });
    const row = (rows ?? []).find((r) => r.media_key === currentMediaKey());
    if (row && row.position_seconds > 30 && row.duration_seconds > 0 && row.position_seconds < row.duration_seconds * 0.95) {
      playerSeek(row.position_seconds);
      resumeNote.value = `Resumed from ${fmt(row.position_seconds)}`;
      setTimeout(() => (resumeNote.value = null), 5000);
    }
    resumed = true;
  } catch {
    resumed = true;
  }
}

async function saveProgress(final = false) {
  const key = currentMediaKey();
  const pos = playerState.timePos;
  if (!key || pos == null || pos < 1 || playerState.duration == null) return;
  if (!final && Math.abs(pos - lastSaveSeconds) < 5) return;
  lastSaveSeconds = pos;
  try {
    await invoke("history_upsert", {
      mediaKey: key,
      title: playerStore.title ?? "video",
      positionSeconds: pos,
      durationSeconds: playerState.duration,
    });
  } catch {
    // progress persistence is best-effort during playback
  }
}

function onKeydown(e) {
  const tag = document.activeElement?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
  switch (e.key) {
    case " ":
      e.preventDefault();
      playerTogglePause();
      break;
    case "ArrowLeft":
      e.preventDefault();
      playerSeek(Math.max(0, (playerState.timePos ?? 0) - 10));
      break;
    case "ArrowRight":
      e.preventDefault();
      playerSeek(Math.min(playerState.duration ?? Infinity, (playerState.timePos ?? 0) + 10));
      break;
    case "ArrowUp":
      e.preventDefault();
      volume.value = Math.min(130, volume.value + 5);
      playerSetVolume(volume.value);
      break;
    case "ArrowDown":
      e.preventDefault();
      volume.value = Math.max(0, volume.value - 5);
      playerSetVolume(volume.value);
      break;
    case "m":
    case "M":
      muted.value = !muted.value;
      playerSetMute(muted.value);
      break;
    case "f":
    case "F":
      toggleFullscreen();
      break;
  }
}

async function toggleFullscreen() {
  try {
    fullscreen.value = !fullscreen.value;
    await appWindow.setFullscreen(fullscreen.value);
  } catch (err) {
    controlError.value = `fullscreen failed: ${err}`;
  }
}

async function refreshTracks() {
  tracks.value = await playerGetTrackList();
}

async function seekWhenReady(pos) {
  for (let i = 0; i < 24; i++) {
    if (playerState.duration != null) {
      try {
        await playerSeek(pos);
        return;
      } catch {
        // property not ready yet; retry
      }
    }
    await new Promise((r) => setTimeout(r, 250));
  }
}

async function forceSoftwareDecode() {
  try {
    await invoke("dev_log", { message: "FIX v2: reinit hwdec=no d3d11-output-format=bgra8 + reload" });
    fixApplied.value = "re-initializing with 8-bit output…";
    const pos = playerState.timePos ?? 0;
    await playerReinit();
    if (playerStore.src) {
      await playerLoad(playerStore.src);
      if (pos > 2) await seekWhenReady(pos);
    }
    setTimeout(() => (fixApplied.value = null), 4000);
  } catch (err) {
    controlError.value = String(err?.message ?? err);
  }
}

async function onSubtitleFile() {
  controlError.value = null;
  const file = subtitleFile.value?.files?.[0];
  if (!file) return;
  try {
    const buf = await file.arrayBuffer();
    const bytes = new Uint8Array(buf);
    let binary = "";
    for (let i = 0; i < bytes.length; i += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    }
    const path = await invoke("import_subtitle", {
      name: file.name,
      contentsBase64: btoa(binary),
    });
    await playerAddExternalSubtitle(path, file.name);
  } catch (err) {
    controlError.value = String(err?.message ?? err);
  }
}

onMounted(async () => {
  await playerInit();
  if (playerStore.src) await playerLoad(playerStore.src);
  await refreshTracks();
  if (currentMediaKey()) {
    invoke("library_add", {
      mediaKey: currentMediaKey(),
      title: playerStore.title ?? "video",
      path: null,
    }).catch(() => {});
  }
  probeTimer = setInterval(async () => {
    playerGetHwdec();
    refreshTracks();
    const diag = await playerRenderDiagnostics();
    const src = sourceTorrent.value;
    const dl = src ? `${fmtBytes(src.stats?.downloaded_bytes ?? 0)}/${fmtBytes(src.stats?.total_bytes ?? 0)}` : "n/a";
    invoke("dev_log", {
      message: `DIAG vo=${diag?.currentVo} configured=${diag?.voConfigured} frames=${diag?.videoFrames} dispRate=${diag?.displayRate} hwdecFailed=${diag?.hwdecFailed} download=${dl}`,
    }).catch(() => {});
    const t = playerState.timePos;
    if (!playerState.paused && playerState.duration != null) {
      if (t != null && lastTime != null && Math.abs(t - lastTime) > 0.01) {
        stallSeconds = 0;
        playerState.buffering = false;
      } else {
        stallSeconds += 1;
        playerState.buffering = stallSeconds >= 2;
      }
    } else {
      stallSeconds = 0;
      playerState.buffering = false;
    }
    lastTime = t;
    if (playerState.duration != null && playerState.duration > 0) await maybeResume();
    await saveProgress(false);
  }, 1000);
  window.addEventListener("keydown", onKeydown);
});

onUnmounted(async () => {
  clearInterval(probeTimer);
  window.removeEventListener("keydown", onKeydown);
  await saveProgress(true);
  if (fullscreen.value) appWindow.setFullscreen(false).catch(() => {});
  playerDestroy();
});

function onSeek(e) {
  const t = Number(e.target.value);
  if (playerState.duration) playerSeek(t);
}

function onVolume(e) {
  volume.value = Number(e.target.value);
  playerSetVolume(volume.value);
  muted.value = false;
  playerSetMute(false);
}

function setSpeed(rate) {
  speed.value = rate;
  playerSetSpeed(rate).catch((err) => (controlError.value = String(err)));
}

function setAspect(value) {
  aspect.value = value;
  playerSetAspect(value).catch((err) => (controlError.value = String(err)));
}

function selectAudio(id) {
  playerSetAudioTrack(id).catch((err) => (controlError.value = String(err)));
}

function selectSubtitle(id) {
  playerSetSubtitleTrack(id === "off" ? "no" : id).catch((err) => (controlError.value = String(err)));
}

function fmt(t) {
  if (t == null) return "--:--";
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = Math.floor(t % 60);
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}
</script>

<template>
  <div v-if="!playerStore.src" class="flex h-full flex-col">
    <div class="absolute inset-0 bg-[#0b0f17]"></div>
    <div class="relative flex flex-1 flex-col">
      <AppState
        state="empty"
        title="Nothing playing"
        message="Open a video from torrent details, or use the dev harness."
      />
    </div>
  </div>

  <div v-else class="flex h-full flex-col justify-end" data-player-surface>
    <div class="absolute top-3 left-4 flex items-center gap-4 text-xs text-slate-300 drop-shadow">
      <span class="max-w-[360px] truncate font-medium text-slate-100">{{ playerStore.title ?? "" }}</span>
      <span>hwdec: {{ playerState.hwdec ?? "?" }}</span>
      <span>codec: {{ playerState.videoFormat ?? "?" }}</span>
      <span v-if="resumeNote" class="rounded bg-sky-500/20 px-2 py-0.5 text-sky-300">{{ resumeNote }}</span>
      <span v-if="speed !== 1" class="rounded bg-violet-500/20 px-1.5 text-violet-300">{{ speed }}×</span>
      <span v-if="playerState.buffering" class="animate-pulse rounded bg-amber-500/20 px-2 py-0.5 text-amber-300">
        buffering…
        <span v-if="downloadSpeed"> ({{ fmtSpeed(downloadSpeed) }})</span>
      </span>
      <span
        v-else-if="playerState.cacheTime != null && playerState.timePos != null && playerState.cacheTime > playerState.timePos + 2"
        class="text-slate-400"
      >
        buffered to {{ fmt(playerState.cacheTime) }}
      </span>
      <span v-if="playerState.error" class="text-red-400">{{ playerState.error }}</span>
    </div>

    <div class="absolute top-3 right-4 z-10 flex gap-2">
      <button
        v-if="playerState.buffering || playerState.videoFormat"
        class="rounded bg-amber-500/80 px-3 py-1 text-xs font-medium text-slate-950 hover:bg-amber-400"
        title="If video stays black, switch to software decoding"
        @click="forceSoftwareDecode"
      >
        fix video (software decode)
      </button>
      <button
        class="rounded bg-slate-800/90 px-3 py-1 text-xs text-slate-300 hover:bg-slate-700"
        @click="toggleFullscreen"
      >
        {{ fullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)" }}
      </button>
      <button
        class="rounded bg-slate-800/90 px-3 py-1 text-xs text-slate-300 hover:bg-slate-700"
        @click="$router.back()"
      >
        ✕ Close
      </button>
    </div>

    <div class="bg-slate-900/85 px-4 py-3 backdrop-blur-sm" data-player-controls>
      <input
        type="range"
        class="w-full accent-sky-400"
        min="0"
        :max="playerState.duration ?? 0"
        step="0.5"
        :value="playerState.timePos ?? 0"
        @change="onSeek"
      />
      <div class="mt-2 flex flex-wrap items-center gap-3">
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

        <button
          class="rounded bg-slate-700 px-2 py-1 text-xs text-slate-300 hover:bg-slate-600"
          :class="{ 'bg-violet-500/30 text-violet-200': muted }"
          @click="muted = !muted; playerSetMute(muted)"
        >
          {{ muted ? "unmute" : "mute" }}
        </button>

        <select
          class="rounded bg-slate-800 px-2 py-1 text-xs text-slate-200"
          :value="speed"
          @change="setSpeed(Number($event.target.value))"
        >
          <option v-for="s in SPEEDS" :key="s" :value="s">{{ s }}×</option>
        </select>

        <select
          class="rounded bg-slate-800 px-2 py-1 text-xs text-slate-200"
          :value="aspect"
          @change="setAspect($event.target.value)"
        >
          <option v-for="a in ASPECTS" :key="a.value" :value="a.value">aspect: {{ a.label }}</option>
        </select>

        <select
          v-if="audioTracks.length"
          class="max-w-[160px] rounded bg-slate-800 px-2 py-1 text-xs text-slate-200"
          @change="selectAudio($event.target.value)"
        >
          <option value="" disabled selected>audio…</option>
          <option v-for="t in audioTracks" :key="t.id" :value="t.id">
            {{ t.title || t.lang || `audio ${t.id}` }}{{ t.default ? " (default)" : "" }}
          </option>
        </select>

        <select
          class="max-w-[160px] rounded bg-slate-800 px-2 py-1 text-xs text-slate-200"
          @change="selectSubtitle($event.target.value); $event.target.value = ''"
        >
          <option value="" disabled selected>subtitles…</option>
          <option value="off">off</option>
          <option v-for="t in subtitleTracks" :key="t.id" :value="t.id">
            {{ t.title || t.lang || `sub ${t.id}` }}{{ t.external ? " (external)" : "" }}
          </option>
        </select>

        <label class="cursor-pointer rounded bg-slate-800 px-2 py-1 text-xs text-slate-300 hover:bg-slate-700">
          + load SRT
          <input ref="subtitleFile" type="file" accept=".srt,.ass,.vtt,.ssa" class="hidden" @change="onSubtitleFile" />
        </label>

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
      <p v-if="controlError" class="mt-1 text-xs text-red-300">{{ controlError }}</p>
    </div>
  </div>
</template>
