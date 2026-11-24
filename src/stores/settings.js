import { reactive, watch } from "vue";

const KEY = "streamnest.settings.v1";

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

function load() {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    return { ...DEFAULTS, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULTS };
  }
}

export const settingsStore = reactive(load());

watch(
  settingsStore,
  () => {
    try {
      localStorage.setItem(KEY, JSON.stringify(settingsStore));
    } catch {
      // storage unavailable; settings stay in memory
    }
  },
  { deep: true }
);

export function resetSettings() {
  Object.assign(settingsStore, DEFAULTS);
}
