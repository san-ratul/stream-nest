import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";

const KEY = "streamnest.player.current";

export const playerStore = reactive({
  src: null,
  title: null,
  torrentId: null,
  fileId: null,
  infoHash: null,
  hasVideo: false,
});

function persist() {
  try {
    if (playerStore.src) {
      sessionStorage.setItem(
        KEY,
        JSON.stringify({
          src: playerStore.src,
          title: playerStore.title,
          torrentId: playerStore.torrentId,
          fileId: playerStore.fileId,
          infoHash: playerStore.infoHash,
        })
      );
    } else {
      sessionStorage.removeItem(KEY);
    }
  } catch {
    // storage unavailable
  }
}

export async function restorePlayer() {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (!raw) return;
    const saved = JSON.parse(raw);
    if (!saved?.src) return;

    // Torrent stream URLs contain a per-run port + token: never replay them.
    // Rebuild from the torrent identity against the current streaming server.
    if (saved.src.startsWith("http") && saved.infoHash != null && saved.fileId != null) {
      try {
        const url = await invoke("stream_url", {
          id: saved.infoHash,
          fileId: saved.fileId,
        });
        Object.assign(playerStore, saved, { src: url });
        return;
      } catch {
        sessionStorage.removeItem(KEY);
        return;
      }
    }
    Object.assign(playerStore, saved);
  } catch {
    // ignore malformed state
  }
}

export function mediaKey(infoHash, fileId) {
  if (infoHash != null && fileId != null) return `${infoHash}:${fileId}`;
  return null;
}

export function openInPlayer({ src, title, torrentId, fileId, infoHash }) {
  playerStore.src = src;
  playerStore.title = title ?? null;
  playerStore.torrentId = torrentId ?? null;
  playerStore.fileId = fileId ?? null;
  playerStore.infoHash = infoHash ?? null;
  playerStore.hasVideo = false;
  persist();
}

export function clearPlayer() {
  playerStore.src = null;
  playerStore.title = null;
  playerStore.torrentId = null;
  playerStore.fileId = null;
  playerStore.infoHash = null;
  playerStore.hasVideo = false;
  persist();
}
