import { reactive } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export const torrentStore = reactive({
  started: false,
  engineReady: false,
  engineError: null,
  torrents: [],
});

let statusPoll = null;

export function torrentName(t) {
  return t?.name || t?.info_hash || `torrent ${t?.id}`;
}

export function torrentProgress(t) {
  const downloaded = t?.stats?.downloaded_bytes ?? t?.downloaded_bytes ?? 0;
  const total = t?.stats?.total_bytes ?? t?.total_bytes ?? 0;
  if (!total) return 0;
  return Math.min(100, Math.round((downloaded / total) * 100));
}

export function torrentLive(t) {
  return Boolean(t?.stats?.live || t?.live);
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

export function fmtSpeed(n) {
  return n ? `${fmtBytes(n)}/s` : "—";
}

export async function startTorrentStore() {
  if (torrentStore.started) return;
  torrentStore.started = true;

  await listen("engine:torrents", (e) => {
    torrentStore.torrents = e.payload?.torrents ?? [];
  });
  await listen("engine:ready", () => {
    torrentStore.engineReady = true;
    torrentStore.engineError = null;
  });
  await listen("engine:error", (e) => {
    torrentStore.engineError = String(e.payload ?? "engine error");
  });

  const poll = async () => {
    try {
      const s = await invoke("engine_status");
      torrentStore.engineReady = s.ready;
      torrentStore.engineError = s.error;
    } catch {
      torrentStore.engineReady = false;
    }
  };
  poll();
  statusPoll = setInterval(poll, 1500);
}

export async function addTorrentUri(uri, peers) {
  return await invoke("torrent_add_uri", { uri, peers: peers ?? null });
}

export async function addTorrentFileBase64(contents, peers) {
  return await invoke("torrent_add_file_base64", {
    contents,
    peers: peers ?? null,
  });
}

export async function torrentDetails(id) {
  return await invoke("torrent_details", { id });
}

export async function torrentPause(id) {
  return await invoke("torrent_pause", { id });
}

export async function torrentStart(id) {
  return await invoke("torrent_start", { id });
}

export async function torrentDelete(id) {
  return await invoke("torrent_delete", { id });
}

export async function torrentForget(id) {
  return await invoke("torrent_forget", { id });
}

export async function torrentUpdateOnlyFiles(id, onlyFiles) {
  return await invoke("torrent_update_only_files", { id, onlyFiles });
}

export async function streamUrl(id, fileId) {
  return await invoke("stream_url", { id, fileId });
}
