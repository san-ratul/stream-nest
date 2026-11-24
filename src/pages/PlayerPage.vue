<script setup>
import { ref, computed, onMounted, onUnmounted, nextTick } from "vue";
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
  playerGetTrackList,
  playerSetAudioTrack,
  playerSetSubtitleTrack,
  playerSetSpeed,
  playerSetAspect,
  playerSetMute,
  playerAddExternalSubtitle,
  playerSetProperty,
  playerReinit,
  playerSetVideoMarginRatio,
  playerRenderDiagnostics,
} from "../composables/usePlayer";
import { invoke } from "@tauri-apps/api/core";
import { playerStore, mediaKey } from "../stores/player";
import { torrentStore, fmtSpeed, fmtBytes } from "../stores/torrents";
import AppState from "../components/common/AppState.vue";

const volume = ref(100);
const muted = ref(false);
const fullscreen = ref(false);
const speed = ref(1);
const aspect = ref("-1");
const tracks = ref([]);
const subtitleFile = ref(null);
const controlError = ref(null);
const resumeNote = ref(null);
const fixApplied = ref(null);
const scrubbing = ref(false);
const scrubValue = ref(0);
const surfaceRef = ref(null);
let probeTimer = null;
let lastTime = null;
let stallSeconds = 0;
let lastSaveSeconds = 0;
let resumed = false;
let resizeObserver = null;
let marginsTimer = null;

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

const bufferedPct = computed(() => {
  if (!playerState.duration || playerState.cacheTime == null) return 0;
  return Math.min(100, (playerState.cacheTime / playerState.duration) * 100);
});

const timelineValue = computed(() =>
  scrubbing.value ? scrubValue.value : (playerState.timePos ?? 0)
);

const SPEEDS = [0.5, 1, 1.25, 1.5, 2];
const ASPECTS = [
  { label: "auto", value: "-1" },
  { label: "fill", value: "stretch" },
  { label: "16:9", value: "16:9" },
  { label: "4:3", value: "4:3" },
  { label: "2.35:1", value: "2.35" },
];

function currentMediaKey() {
  return mediaKey(playerStore.infoHash, playerStore.fileId);
}

async function updateMargins() {
  const el = surfaceRef.value;
  if (!el || !playerState.initialized) return;
  await nextTick();
  const rect = el.getBoundingClientRect();
  if (rect.width < 50 || rect.height < 50) return;
  const W = window.innerWidth;
  const H = window.innerHeight;
  const ratio = {
    left: Math.max(0, rect.left / W),
    right: Math.max(0, (W - rect.right) / W),
    top: Math.max(0, rect.top / H),
    bottom: Math.max(0, (H - rect.bottom) / H),
  };
  try {
    await playerSetVideoMarginRatio(ratio);
  } catch {
    // margins are cosmetic; retry on next resize
  }
}

async function maybeResume() {
  if (resumed || !currentMediaKey()) return;
  try {
    const rows = await invoke("history_list", { limit: 100 });
    const row = (rows ?? []).find((r) => r.media_key === currentMediaKey());
    if (row && row.position_seconds > 30 && row.duration_seconds > 0 && row.position_seconds < row.duration_seconds * 0.95) {
      await playerSeek(row.position_seconds);
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
    setTimeout(updateMargins, 300);
  } catch (err) {
    controlError.value = `fullscreen failed: ${err}`;
  }
}

async function refreshTracks() {
  tracks.value = await playerGetTrackList();
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

function onScrubStart(e) {
  scrubbing.value = true;
  scrubValue.value = Number(e.target.value);
}

function onScrubMove(e) {
  if (!scrubbing.value) return;
  scrubValue.value = Number(e.target.value);
}

function onScrubEnd() {
  if (!scrubbing.value) return;
  const t = scrubValue.value;
  scrubbing.value = false;
  if (playerState.duration) playerSeek(t);
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
    await updateMargins();
    setTimeout(() => (fixApplied.value = null), 4000);
  } catch (err) {
    controlError.value = String(err?.message ?? err);
  }
}

onMounted(async () => {
  await playerInit();
  if (playerStore.src) await playerLoad(playerStore.src);
  await refreshTracks();
  await updateMargins();
  if (surfaceRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => {
      clearTimeout(marginsTimer);
      marginsTimer = setTimeout(updateMargins, 120);
    });
    resizeObserver.observe(surfaceRef.value);
  }
  window.addEventListener("resize", updateMargins);
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
    const m = diag?.margins ?? {};
    invoke("dev_log", {
      message: `DIAG vo=${diag?.currentVo} configured=${diag?.voConfigured} margins L=${m.left?.toFixed(3)} R=${m.right?.toFixed(3)} T=${m.top?.toFixed(3)} B=${m.bottom?.toFixed(3)} download=${dl}`,
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
  window.removeEventListener("resize", updateMargins);
  if (resizeObserver) resizeObserver.disconnect();
  await saveProgress(true);
  if (fullscreen.value) appWindow.setFullscreen(false).catch(() => {});
  playerDestroy();
});

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

  <div v-else ref="surfaceRef" class="relative flex h-full flex-col justify-end" data-player-surface>
    <div class="absolute inset-x-0 top-0 z-10 bg-gradient-to-b from-black/70 via-black/30 to-transparent px-5 pb-8 pt-3">
      <div class="flex items-center gap-4 text-xs text-slate-200 drop-shadow">
        <span class="max-w-[40%] truncate text-sm font-semibold text-white">{{ playerStore.title ?? "" }}</span>
        <span class="rounded bg-white/10 px-1.5 py-0.5">{{ playerState.hwdec === "no" ? "sw decode" : `hwdec ${playerState.hwdec ?? "?"}` }}</span>
        <span class="rounded bg-white/10 px-1.5 py-0.5">{{ (playerState.videoFormat ?? "?").toUpperCase() }}</span>
        <span v-if="resumeNote" class="rounded bg-sky-500/30 px-2 py-0.5 text-sky-200">{{ resumeNote }}</span>
        <span v-if="speed !== 1" class="rounded bg-violet-500/30 px-1.5 text-violet-200">{{ speed }}×</span>
        <span v-if="playerState.buffering" class="animate-pulse rounded bg-amber-500/30 px-2 py-0.5 text-amber-200">
          buffering…
          <span v-if="downloadSpeed"> ({{ fmtSpeed(downloadSpeed) }})</span>
        </span>
        <span
          v-else-if="playerState.cacheTime != null && playerState.timePos != null && playerState.cacheTime > playerState.timePos + 2"
          class="text-slate-300"
        >
          buffered to {{ fmt(playerState.cacheTime) }}
        </span>
        <span v-if="playerState.error" class="text-red-300">{{ playerState.error }}</span>
      </div>
    </div>

    <div class="absolute right-4 top-3 z-20 flex gap-2">
      <button
        v-if="playerState.videoFormat"
        class="rounded-lg bg-amber-500/80 px-3 py-1 text-xs font-medium text-slate-950 shadow hover:bg-amber-400"
        title="If video stays black, reinitialize with software decode and 8-bit output"
        @click="forceSoftwareDecode"
      >
        fix video
      </button>
      <button
        class="rounded-lg bg-white/10 px-3 py-1 text-xs text-slate-100 shadow backdrop-blur hover:bg-white/20"
        :title="fullscreen ? 'Exit fullscreen (F)' : 'Fullscreen (F)'"
        @click="toggleFullscreen"
      >
        {{ fullscreen ? "Exit fullscreen" : "Fullscreen" }}
      </button>
      <button
        class="rounded-lg bg-white/10 px-3 py-1 text-xs text-slate-100 shadow backdrop-blur hover:bg-white/20"
        title="Close player"
        @click="$router.back()"
      >
        ✕
      </button>
    </div>

    <div class="relative z-10 bg-gradient-to-t from-black/95 via-black/80 to-black/40 px-5 pb-4 pt-2 backdrop-blur-sm" data-player-controls>
      <div class="group relative flex h-4 cursor-pointer items-center">
        <div class="absolute inset-x-0 top-1.5 h-1 overflow-hidden rounded-full bg-white/15">
          <div class="h-full rounded-full bg-white/25" :style="{ width: `${bufferedPct}%` }"></div>
        </div>
        <div
          class="absolute left-0 top-1.5 h-1 rounded-full bg-sky-400 transition-[width] duration-150"
          :style="{ width: `${(timelineValue / (playerState.duration || 1)) * 100}%` }"
        ></div>
        <input
          type="range"
          class="relative z-10 h-4 w-full cursor-pointer appearance-none bg-transparent
                 [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:appearance-none
                 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-sky-400
                 [&::-webkit-slider-thumb]:opacity-0 [&::-webkit-slider-thumb]:shadow
                 [&::-webkit-slider-thumb]:transition-opacity group-hover:[&::-webkit-slider-thumb]:opacity-100"
          min="0"
          :max="playerState.duration ?? 0"
          step="0.1"
          :value="timelineValue"
          @pointerdown="onScrubStart"
          @input="onScrubMove"
          @pointerup="onScrubEnd"
          @change="onScrubEnd"
        />
      </div>

      <div class="mt-1 flex flex-wrap items-center gap-2.5">
        <button
          class="flex h-9 w-9 items-center justify-center rounded-full bg-white text-slate-950 shadow transition hover:bg-sky-300"
          :title="playerState.paused ? 'Play (Space)' : 'Pause (Space)'"
          @click="playerTogglePause"
        >
          <span v-if="playerState.paused" class="text-base leading-none">▶</span>
          <span v-else class="text-base leading-none">⏸</span>
        </button>
        <button
          class="rounded-lg bg-white/10 px-3 py-1.5 text-sm text-slate-100 hover:bg-white/20"
          title="Back 10s (←)"
          @click="playerSeek(Math.max(0, (playerState.timePos ?? 0) - 10))"
        >
          −10s
        </button>
        <button
          class="rounded-lg bg-white/10 px-3 py-1.5 text-sm text-slate-100 hover:bg-white/20"
          title="Forward 10s (→)"
          @click="playerSeek(Math.min(playerState.duration ?? 0, (playerState.timePos ?? 0) + 10))"
        >
          +10s
        </button>
        <span class="font-mono text-xs text-slate-300 tabular-nums">
          {{ fmt(scrubbing ? scrubValue : playerState.timePos) }} / {{ fmt(playerState.duration) }}
        </span>

        <button
          class="rounded-lg px-2 py-1.5 text-sm text-slate-100 hover:bg-white/10"
          :class="muted ? 'text-red-300' : ''"
          title="Mute (M)"
          @click="muted = !muted; playerSetMute(muted)"
        >
          {{ muted ? "🔇" : "🔊" }}
        </button>

        <select
          class="rounded-lg border border-white/10 bg-black/50 px-2 py-1.5 text-xs text-slate-100 focus:outline-none focus:ring-1 focus:ring-sky-400"
          :value="speed"
          title="Playback speed"
          @change="setSpeed(Number($event.target.value))"
        >
          <option v-for="s in SPEEDS" :key="s" :value="s">{{ s }}×</option>
        </select>

        <select
          class="rounded-lg border border-white/10 bg-black/50 px-2 py-1.5 text-xs text-slate-100 focus:outline-none focus:ring-1 focus:ring-sky-400"
          :value="aspect"
          title="Aspect ratio"
          @change="setAspect($event.target.value)"
        >
          <option v-for="a in ASPECTS" :key="a.value" :value="a.value">aspect: {{ a.label }}</option>
        </select>

        <select
          v-if="audioTracks.length"
          class="max-w-[150px] rounded-lg border border-white/10 bg-black/50 px-2 py-1.5 text-xs text-slate-100 focus:outline-none"
          @change="selectAudio($event.target.value)"
        >
          <option value="" disabled selected>audio…</option>
          <option v-for="t in audioTracks" :key="t.id" :value="t.id">
            {{ t.title || t.lang || `audio ${t.id}` }}{{ t.default ? " ✓" : "" }}
          </option>
        </select>

        <select
          class="max-w-[150px] rounded-lg border border-white/10 bg-black/50 px-2 py-1.5 text-xs text-slate-100 focus:outline-none"
          @change="selectSubtitle($event.target.value); $event.target.value = ''"
        >
          <option value="" disabled selected>subtitles…</option>
          <option value="off">off</option>
          <option v-for="t in subtitleTracks" :key="t.id" :value="t.id">
            {{ t.title || t.lang || `sub ${t.id}` }}{{ t.external ? " (ext)" : "" }}
          </option>
        </select>

        <label class="cursor-pointer rounded-lg border border-white/10 px-2.5 py-1.5 text-xs text-slate-100 hover:bg-white/10">
          + load SRT
          <input ref="subtitleFile" type="file" accept=".srt,.ass,.vtt,.ssa" class="hidden" @change="onSubtitleFile" />
        </label>

        <div class="ml-auto flex items-center gap-2">
          <span class="text-xs text-slate-400">Vol</span>
          <input
            type="range"
            class="h-1.5 w-24 cursor-pointer appearance-none rounded-full bg-white/20
                   [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none
                   [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
            min="0"
            max="130"
            :value="volume"
            @input="onVolume"
          />
        </div>
      </div>
      <p v-if="fixApplied || controlError" class="mt-1 text-xs" :class="controlError ? 'text-red-300' : 'text-emerald-300'">
        {{ controlError ?? fixApplied }}
      </p>
    </div>
  </div>
</template>
