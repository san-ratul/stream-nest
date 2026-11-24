import {
  init,
  destroy,
  command,
  setProperty,
  getProperty,
  observeProperties,
  setVideoMarginRatio,
} from "tauri-plugin-libmpv-api";
import { invoke } from "@tauri-apps/api/core";
import { reactive } from "vue";

const OBSERVED = [
  ["pause", "flag"],
  ["time-pos", "double", "none"],
  ["duration", "double", "none"],
  ["demuxer-cache-time", "double", "none"],
  ["hwdec-current", "string", "none"],
  ["video-format", "string", "none"],
  ["eof-reached", "flag", "none"],
  ["estimated-vf-fps", "double", "none"],
];

export const playerState = reactive({
  initialized: false,
  error: null,
  paused: true,
  timePos: null,
  duration: null,
  cacheTime: null,
  hwdec: null,
  videoFormat: null,
  eof: null,
  loadedSrc: null,
  firstFrameMs: null,
  lastSeekMs: null,
  buffering: false,
  speed: 1,
  muted: false,
  aspect: "auto",
  vfFps: null,
});

let unlisten = null;
let loadT0 = null;
let awaitingFirstFrame = false;
let awaitingSeekResume = false;
let seekTarget = null;
let seekT0 = null;

async function log(msg, level) {
  try {
    await invoke("dev_log", { message: msg, level: level ?? null });
  } catch {
    console.log(`[player] ${msg}`);
  }
}

function onTimePos(v) {
  playerState.timePos = v;
  if (awaitingFirstFrame && v != null && v > 0 && loadT0 != null) {
    awaitingFirstFrame = false;
    playerState.firstFrameMs = Math.round(performance.now() - loadT0);
    log(`FIRST_FRAME_MS=${playerState.firstFrameMs}`);
  }
  if (
    awaitingSeekResume &&
    v != null &&
    seekTarget != null &&
    Math.abs(v - seekTarget) < 3
  ) {
    awaitingSeekResume = false;
    playerState.lastSeekMs = Math.round(performance.now() - seekT0);
    log(`SEEK_RESUME_MS=${playerState.lastSeekMs}`);
  }
}

export async function playerInit(options) {
  if (playerState.initialized) return;
  const cfg = options ?? {
    initialOptions: {
      vo: "gpu",
      hwdec: "auto-safe",
      "keep-open": "yes",
      "force-window": "yes",
      "d3d11-output-format": "bgra8",
    },
    observedProperties: OBSERVED,
  };
  try {
    await init(cfg);
    playerState.initialized = true;
    await log("mpv initialized");

    unlisten = await observeProperties(OBSERVED, ({ name, data }) => {
      switch (name) {
        case "pause":
          playerState.paused = data;
          break;
        case "time-pos":
          onTimePos(data);
          break;
        case "duration":
          playerState.duration = data;
          break;
        case "demuxer-cache-time":
          playerState.cacheTime = data;
          break;
        case "hwdec-current":
          playerState.hwdec = data;
          break;
        case "video-format":
          playerState.videoFormat = data;
          break;
        case "eof-reached":
          playerState.eof = data;
          break;
        case "estimated-vf-fps":
          playerState.vfFps = data;
          break;
      }
    });
  } catch (err) {
    playerState.error = String(err);
    await log(`mpv init failed: ${err}`, "error");
  }
}

export async function playerDestroy() {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  if (playerState.initialized) {
    try {
      // stop playback first so decode/present threads wind down before teardown
      await command("stop", []);
    } catch (err) {
      await log(`mpv stop before destroy failed: ${err}`, "warn");
    }
    try {
      await destroy();
    } catch (err) {
      await log(`mpv destroy failed: ${err}`, "warn");
    }
    playerState.initialized = false;
  }
}

export async function playerReinit(overrides) {
  await playerDestroy();
  await playerInit({
    initialOptions: {
      vo: "gpu",
      hwdec: "no",
      "keep-open": "yes",
      "force-window": "yes",
      "d3d11-output-format": "bgra8",
      ...(overrides ?? {}),
    },
    observedProperties: OBSERVED,
  });
}

export async function playerLoad(src) {
  playerState.loadedSrc = src;
  playerState.firstFrameMs = null;
  playerState.lastSeekMs = null;
  loadT0 = performance.now();
  awaitingFirstFrame = true;
  await command("loadfile", [src]);
  await setProperty("pause", false);
  await log(`STREAM_START src=${src}`);
}

export async function playerTogglePause() {
  const next = !playerState.paused;
  try {
    await setProperty("pause", next);
  } finally {
    try {
      const confirmed = await getProperty("pause", "flag");
      playerState.paused = confirmed === true;
    } catch {
      playerState.paused = next;
    }
  }
}

export async function playerSetVideoMarginRatio(ratio) {
  await setVideoMarginRatio(ratio);
}

export async function playerSeek(seconds) {
  seekTarget = seconds;
  seekT0 = performance.now();
  awaitingSeekResume = true;
  await setProperty("time-pos", seconds);
  await log(`SEEK_START target=${seconds}`);
}

export async function playerSetVolume(percent) {
  await setProperty("volume", percent);
}

export async function playerGetHwdec() {
  try {
    const v = await getProperty("hwdec-current", "string");
    playerState.hwdec = v;
    return v;
  } catch {
    return null;
  }
}

export async function playerGetPaused() {
  try {
    return await getProperty("pause", "flag");
  } catch {
    return null;
  }
}

export async function playerRenderDiagnostics() {
  try {
    const [voConfigured, currentVo, videoFrames, displayRate, hwdecFailed, videoDetected, mL, mR, mT, mB] = await Promise.all([
      getProperty("vo-configured", "flag").catch(() => null),
      getProperty("current-vo", "string").catch(() => null),
      getProperty("video-out-frames", "int64").catch(() => null),
      getProperty("estimated-display-frame-rate", "double").catch(() => null),
      getProperty("hwdec-failed", "flag").catch(() => null),
      getProperty("video-detected", "flag").catch(() => null),
      getProperty("video-margin-ratio-left", "double").catch(() => null),
      getProperty("video-margin-ratio-right", "double").catch(() => null),
      getProperty("video-margin-ratio-top", "double").catch(() => null),
      getProperty("video-margin-ratio-bottom", "double").catch(() => null),
    ]);
    return { voConfigured, currentVo, videoFrames, displayRate, hwdecFailed, videoDetected, margins: { left: mL, right: mR, top: mT, bottom: mB } };
  } catch {
    return null;
  }
}

export async function playerGetTrackList() {
  try {
    // NEVER use the "node" format through the wrapper: its deserializer
    // segfaults (0xc0000005). String format is safe; parse defensively.
    const v = await getProperty("track-list", "string");
    if (!v || typeof v !== "string") return [];
    const cleaned = v.replace(/([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:/g, '$1"$2":');
    const parsed = JSON.parse(cleaned);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export async function playerSetAudioTrack(id) {
  await setProperty("aid", id);
}

export async function playerSetSubtitleTrack(id) {
  await setProperty("sid", id);
}

export async function playerSetSpeed(rate) {
  await setProperty("speed", rate);
  playerState.speed = rate;
}

export async function playerSetAspect(value) {
  await setProperty("video-aspect-override", value);
  playerState.aspect = value;
}

export async function playerSetMute(muted) {
  await setProperty("mute", muted);
  playerState.muted = muted;
}

export async function playerAddExternalSubtitle(path, title) {
  await command("sub-add", [path, "select", title ?? "external"]);
}

export async function playerMpvCommand(name, args) {
  await command(name, args);
}

export async function playerSetProperty(name, value) {
  await setProperty(name, value);
}
