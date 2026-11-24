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
  playerGetPaused,
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
const controlsRef = ref(null);
const subsOpen = ref(false);
const aspectOpen = ref(false);
const audioOpen = ref(false);
let probeTimer = null;
let lastTime = null;
let stallSeconds = 0;
let lastSaveSeconds = 0;
let resumed = false;
let resizeObserver = null;
let marginsTimer = null;
let loadStartedAt = null;
let loadFailNoted = false;

const appWindow = getCurrentWebviewWindow();

const audioTracks = computed(() => tracks.value.filter((t) => t.type === "audio"));
const subtitleTracks = computed(() => tracks.value.filter((t) => t.type === "sub"));
const selectedAudio = computed(() => audioTracks.value.find((t) => t.selected));
const selectedSubId = computed(() => {
  const s = subtitleTracks.value.find((t) => t.selected);
  return s ? String(s.id) : "off";
});

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
  { label: "16:9", value: "16:9" },
  { label: "4:3", value: "4:3" },
  { label: "2.35:1", value: "2.35" },
];

function langCode(t) {
  const raw = t?.lang || t?.title || "";
  const code = String(raw).trim();
  if (!code) return t?.external ? "EXT" : "UND";
  return code.toUpperCase().slice(0, 3);
}

function trackLabel(t) {
  const base = langCode(t);
  const extra = [];
  if (t?.title && t.lang && t.title.toLowerCase() !== t.lang.toLowerCase()) {
    extra.push(t.title);
  }
  if (t?.external) extra.push("external");
  return extra.length ? `${base} · ${extra.join(" · ")}` : base;
}

function currentMediaKey() {
  return mediaKey(playerStore.infoHash, playerStore.fileId);
}

async function updateVideoGeometry() {
  const el = surfaceRef.value;
  if (!el || !playerState.initialized) return;
  await nextTick();
  const rect = el.getBoundingClientRect();
  if (rect.width < 50 || rect.height < 50) return;
  const W = window.innerWidth;
  const H = window.innerHeight;
  let bottom = Math.max(0, (H - rect.bottom) / H);
  // reserve the controls bar height so subtitles (incl. ASS scripts) stay above it
  if (controlsRef.value) {
    const cr = controlsRef.value.getBoundingClientRect();
    bottom = Math.max(bottom, Math.max(0, (H - cr.top) / H));
  }
  const ratio = {
    left: Math.max(0, rect.left / W),
    right: Math.max(0, (W - rect.right) / W),
    top: Math.max(0, rect.top / H),
    bottom,
  };
  try {
    await playerSetVideoMarginRatio(ratio);
  } catch {
    // margins are cosmetic; retry on next resize
  }
  if (controlsRef.value) {
    const controlsHeight = controlsRef.value.offsetHeight;
    const subPos = Math.round(
      Math.min(95, Math.max(50, ((rect.height - controlsHeight) / rect.height) * 100))
    );
    try {
      await playerSetProperty("sub-pos", subPos);
    } catch {
      // subtitle positioning is cosmetic
    }
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
  if (e.key === "Escape") {
    subsOpen.value = false;
    aspectOpen.value = false;
    audioOpen.value = false;
    return;
  }
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
    setTimeout(updateVideoGeometry, 300);
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
    await refreshTracks();
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
    await updateVideoGeometry();
    setTimeout(() => (fixApplied.value = null), 4000);
  } catch (err) {
    controlError.value = String(err?.message ?? err);
  }
}

function pickAudio(id) {
  if (id !== "") {
    playerSetAudioTrack(id).catch((err) => (controlError.value = String(err)));
  }
  audioOpen.value = false;
}

function pickSubtitle(id) {
  const value = id === "off" ? "no" : id;
  playerSetSubtitleTrack(value).catch((err) => (controlError.value = String(err)));
  subsOpen.value = false;
}

onMounted(async () => {
  await playerInit();
  if (playerStore.src) {
    loadStartedAt = Date.now();
    await playerLoad(playerStore.src);
  }
  await refreshTracks();
  await updateVideoGeometry();
  if (surfaceRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => {
      clearTimeout(marginsTimer);
      marginsTimer = setTimeout(updateVideoGeometry, 120);
    });
    resizeObserver.observe(surfaceRef.value);
  }
  window.addEventListener("resize", updateVideoGeometry);
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
    try {
      const p = await playerGetPaused();
      if (p != null && p !== playerState.paused) playerState.paused = p;
    } catch {
      // pause sync is best-effort
    }
    const diag = await playerRenderDiagnostics();
    if (diag?.voConfigured === true && !playerStore.hasVideo) {
      playerStore.hasVideo = true;
      invoke("dev_log", { message: "hasVideo=true (vo configured)" }).catch(() => {});
    }
    if (!playerStore.hasVideo && !loadFailNoted && loadStartedAt && Date.now() - loadStartedAt > 8000) {
      loadFailNoted = true;
      controlError.value = "stream did not start — the source may be offline; reopen it from Downloads.";
    }
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
  window.removeEventListener("resize", updateVideoGeometry);
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
    <div
      class="absolute inset-0 z-0 bg-[#0b0f17] transition-opacity duration-300"
      :class="playerStore.hasVideo ? 'opacity-0' : 'opacity-100'"
    ></div>

    <div class="absolute inset-x-0 top-0 z-10 bg-gradient-to-b from-black/70 via-black/30 to-transparent pl-[4.25rem] pr-5 pb-8 pt-3">
      <div class="flex items-center gap-3 text-xs text-slate-200 drop-shadow">
        <span class="max-w-[30%] truncate text-sm font-semibold text-white">{{ playerStore.title ?? "" }}</span>
        <span class="rounded bg-white/10 px-1.5 py-0.5">{{ playerState.hwdec === "no" ? "sw decode" : `hwdec ${playerState.hwdec ?? "?"}` }}</span>
        <span class="rounded bg-white/10 px-1.5 py-0.5">{{ (playerState.videoFormat ?? "?").toUpperCase() }}</span>
        <span v-if="resumeNote" class="rounded bg-sky-500/30 px-2 py-0.5 text-sky-200">{{ resumeNote }}</span>
        <span v-if="speed !== 1" class="rounded bg-violet-500/30 px-1.5 text-violet-200">{{ speed }}×</span>
        <span v-if="playerState.buffering" class="animate-pulse rounded bg-amber-500/30 px-2 py-0.5 text-amber-200">
          buffering… <span v-if="downloadSpeed">({{ fmtSpeed(downloadSpeed) }})</span>
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

    <div class="absolute right-4 top-12 z-30 flex items-center gap-2">
      <div class="relative">
        <button
          class="rounded-lg border border-white/10 bg-black/50 px-2 py-1 text-xs font-medium tracking-wide text-slate-100 shadow backdrop-blur hover:bg-white/10"
          title="Audio language"
          @click="audioOpen = !audioOpen; subsOpen = false"
        >
          {{ selectedAudio ? langCode(selectedAudio) : "AUD" }} ▾
        </button>
        <div
          v-if="audioOpen"
          class="absolute right-0 top-full z-40 mt-2 w-48 overflow-hidden rounded-xl border border-white/10 bg-black/70 shadow-2xl shadow-black/60 backdrop-blur-xl"
        >
          <button
            v-for="t in audioTracks"
            :key="t.id"
            class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition"
            :class="selectedAudio?.id === t.id ? 'bg-sky-500/30 text-sky-200' : 'text-slate-200 hover:bg-white/10'"
            @click="pickAudio(String(t.id))"
          >
            <span class="inline-flex h-5 min-w-7 items-center justify-center rounded bg-sky-500/20 px-1 text-[10px] font-bold text-sky-200">
              {{ langCode(t) }}
            </span>
            <span class="truncate">
              {{ t.title && t.title.toLowerCase() !== (t.lang ?? "").toLowerCase() ? t.title : (t.lang ? t.lang.toUpperCase() : `track ${t.id}`) }}{{ t.default ? " •" : "" }}
            </span>
          </button>
        </div>
      </div>

      <button
        class="rounded-lg border border-white/10 bg-black/50 px-2 py-1 text-xs font-medium tracking-wide text-slate-100 shadow backdrop-blur hover:bg-white/10"
        title="Subtitles"
        @click="subsOpen = !subsOpen; audioOpen = false"
      >
        CC
        <span v-if="selectedSubId !== 'off'" class="text-sky-300">•</span>
      </button>

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

    <div
      v-if="subsOpen"
      class="nice-scroll absolute right-4 top-20 z-40 flex max-h-[65%] w-80 flex-col overflow-hidden rounded-2xl border border-white/10 bg-black/60 shadow-2xl shadow-black/60 backdrop-blur-xl"
    >
      <div class="flex items-center justify-between border-b border-white/10 px-4 py-3">
        <span class="text-sm font-semibold text-slate-100">Subtitles</span>
        <button class="text-xs text-slate-400 hover:text-slate-200" @click="subsOpen = false">✕</button>
      </div>
      <div class="flex-1 overflow-y-auto p-2">
        <button
          class="mb-1 w-full rounded-lg px-3 py-2 text-left text-sm transition"
          :class="selectedSubId === 'off' ? 'bg-sky-500/30 text-sky-200' : 'text-slate-200 hover:bg-white/10'"
          @click="pickSubtitle('off')"
        >
          Off
        </button>
        <button
          v-for="t in subtitleTracks"
          :key="t.id"
          class="mb-1 w-full truncate rounded-lg px-3 py-2 text-left text-sm transition"
          :class="selectedSubId === String(t.id) ? 'bg-sky-500/30 text-sky-200' : 'text-slate-200 hover:bg-white/10'"
          @click="pickSubtitle(String(t.id))"
        >
          {{ trackLabel(t) }}
        </button>
        <p v-if="!subtitleTracks.length" class="px-3 py-2 text-xs text-slate-400">
          No subtitle tracks in this file. Load an external one below.
        </p>
      </div>
      <div class="border-t border-white/10 p-2">
        <label class="flex cursor-pointer items-center justify-center rounded-lg border border-white/10 px-3 py-2 text-xs text-slate-100 hover:bg-white/10">
          + load external subtitle
          <input ref="subtitleFile" type="file" accept=".srt,.ass,.vtt,.ssa" class="hidden" @change="onSubtitleFile" />
        </label>
      </div>
    </div>

    <div
      ref="controlsRef"
      class="absolute inset-x-4 bottom-3 z-10 rounded-2xl border border-white/10 bg-black/35 px-5 pb-3.5 pt-2 shadow-2xl shadow-black/60 backdrop-blur-xl"
      data-player-controls
    >
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

        <div class="relative">
          <button
            class="flex h-9 w-9 items-center justify-center rounded-lg border border-white/10 bg-black/50 text-base text-slate-100 hover:bg-white/10"
            title="Aspect ratio"
            @click="aspectOpen = !aspectOpen"
          >
            ▭
          </button>
          <div
            v-if="aspectOpen"
            class="absolute bottom-full left-1/2 z-20 mb-2 w-36 -translate-x-1/2 overflow-hidden rounded-xl border border-white/10 bg-black/70 shadow-2xl shadow-black/60 backdrop-blur-xl"
          >
            <button
              v-for="a in ASPECTS"
              :key="a.value"
              class="block w-full px-3 py-2 text-left text-xs transition"
              :class="aspect === a.value ? 'bg-sky-500/30 text-sky-200' : 'text-slate-200 hover:bg-white/10'"
              @click="setAspect(a.value); aspectOpen = false"
            >
              {{ a.label }}
            </button>
          </div>
        </div>

        <div class="ml-auto flex items-center gap-2">
          <span class="text-xs text-slate-400">Vol</span>
          <div class="relative flex h-4 w-24 items-center">
            <div class="absolute inset-x-0 h-1.5 rounded-full bg-white/30"></div>
            <div
              class="absolute left-0 h-1.5 rounded-full bg-sky-400"
              :style="{ width: `${(volume / 130) * 100}%` }"
            ></div>
            <input
              type="range"
              class="relative z-10 h-4 w-full cursor-pointer appearance-none bg-transparent
                     [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none
                     [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white
                     [&::-webkit-slider-thumb]:shadow"
              min="0"
              max="130"
              :value="volume"
              @input="onVolume"
            />
          </div>
        </div>
      </div>
      <p v-if="fixApplied || controlError" class="mt-1 text-xs" :class="controlError ? 'text-red-300' : 'text-emerald-300'">
        {{ controlError ?? fixApplied }}
      </p>
    </div>
  </div>
</template>
