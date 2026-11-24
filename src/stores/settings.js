import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

const DEFAULTS = {
  downloadDir: "",
  cacheDir: "",
  cacheSizeLimitMb: 2048,
  downloadSpeedLimitKbps: 0,
  uploadSpeedLimitKbps: 0,
  connectionLimit: 100,
  preferredAudioLanguage: "",
  preferredSubtitleLanguage: "",
  subtitleSize: 100,
  theme: "dark",
  launchOnStartup: false,
  diagnosticsOptIn: false,
  pauseTorrentsOnExit: true,
};

export const settingsStore = reactive({ ...DEFAULTS });

let hydrated = false;
let saveTimer = null;

export async function hydrateSettings() {
  if (hydrated) return;
  hydrated = true;
  try {
    const rows = await invoke("settings_get_all");
    for (const [key, value] of rows ?? []) {
      if (key in settingsStore) {
        const current = settingsStore[key];
        if (typeof current === "number") {
          const n = Number(value);
          settingsStore[key] = Number.isFinite(n) ? n : current;
        } else if (typeof current === "boolean") {
          settingsStore[key] = value === "true";
        } else {
          settingsStore[key] = value;
        }
      }
    }
  } catch (err) {
    console.warn("settings hydration failed", err);
  }

  watch(
    settingsStore,
    () => {
      if (!hydrated) return;
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(persistAll, 400);
    },
    { deep: true }
  );
}

async function persistAll() {
  for (const [key, value] of Object.entries(settingsStore)) {
    try {
      await invoke("settings_set", { key, value: String(value) });
    } catch (err) {
      console.warn(`settings save failed for ${key}`, err);
    }
  }
  try {
    await invoke("apply_engine_limits", {
      downloadLimitKbps: settingsStore.downloadSpeedLimitKbps,
      uploadLimitKbps: settingsStore.uploadSpeedLimitKbps,
    });
  } catch (err) {
    console.warn("engine limit application failed (engine may not be ready yet)", err);
  }
}

export function resetSettings() {
  Object.assign(settingsStore, DEFAULTS);
}
