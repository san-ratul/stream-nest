import {
  init,
  destroy,
  command,
  setProperty,
  getProperty,
  observeProperties,
} from "tauri-plugin-libmpv-api";
import { invoke } from "@tauri-apps/api/core";
import { reactive } from "vue";

const OBSERVED = [
  ["pause", "flag"],
  ["time-pos", "double", "none"],
  ["duration", "double", "none"],
  ["hwdec-current", "string", "none"],
  ["video-format", "string", "none"],
  ["eof-reached", "flag", "none"],
];

export const playerState = reactive({
  initialized: false,
  error: null,
  paused: true,
  timePos: null,
  duration: null,
  hwdec: null,
  videoFormat: null,
  eof: null,
  loadedFile: null,
});

let unlisten = null;

async function log(msg, level) {
  try {
    await invoke("dev_log", { message: msg, level: level ?? null });
  } catch {
    console.log(`[player] ${msg}`);
  }
}

export async function playerInit() {
  if (playerState.initialized) return;
  try {
    await init({
      initialOptions: {
        vo: "gpu-next",
        hwdec: "auto-safe",
        "keep-open": "yes",
        "force-window": "yes",
      },
      observedProperties: OBSERVED,
    });
    playerState.initialized = true;
    await log("mpv initialized");

    unlisten = await observeProperties(OBSERVED, ({ name, data }) => {
      switch (name) {
        case "pause":
          playerState.paused = data;
          break;
        case "time-pos":
          playerState.timePos = data;
          break;
        case "duration":
          playerState.duration = data;
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
      await destroy();
    } catch (err) {
      await log(`mpv destroy failed: ${err}`, "warn");
    }
    playerState.initialized = false;
  }
}

export async function playerLoad(path) {
  playerState.loadedFile = path;
  await command("loadfile", [path]);
  await setProperty("pause", false);
  await log(`loadfile ${path}`);
}

export async function playerTogglePause() {
  const next = !playerState.paused;
  await setProperty("pause", next);
  playerState.paused = next;
}

export async function playerSeek(seconds) {
  await setProperty("time-pos", seconds);
}

export async function playerSeekBy(delta) {
  const current = playerState.timePos ?? 0;
  await playerSeek(Math.max(0, current + delta));
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
