import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";

export const sourceStore = reactive({
  list: [],
  loading: false,
  error: null,
});

export async function loadSources() {
  sourceStore.loading = true;
  sourceStore.error = null;
  try {
    sourceStore.list = (await invoke("source_list", { limit: 200 })) ?? [];
  } catch (err) {
    sourceStore.error = String(err?.message ?? err);
  } finally {
    sourceStore.loading = false;
  }
}

export function sourceTypeLabel(type) {
  switch (type) {
    case "magnet":
      return "Magnet";
    case "torrent_url":
      return "Torrent URL";
    case "torrent_file":
      return "Torrent file";
    case "provider_page":
      return "Provider page";
    case "official":
      return "Official";
    default:
      return type;
  }
}

export function fmtBytes(n) {
  if (n == null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  return `${v.toFixed(v >= 10 || u === 0 ? 0 : 1)} ${units[u]}`;
}

/** Context object for associating a new source with catalog media. */
export function sourceContext({ mediaType = null, tmdbId = null, season = null, episode = null } = {}) {
  if (mediaType == null && tmdbId == null) return null;
  return {
    tmdb_media_type: mediaType,
    tmdb_id: tmdbId,
    season_number: season,
    episode_number: episode,
  };
}
