import { reactive } from "vue";

const KEY = "streamnest.player.current";

export const playerStore = reactive({
  src: null,
  title: null,
  torrentId: null,
  fileId: null,
  infoHash: null,
});

function persist() {
  try {
    if (playerStore.src) {
      sessionStorage.setItem(KEY, JSON.stringify({ ...playerStore }));
    } else {
      sessionStorage.removeItem(KEY);
    }
  } catch {
    // storage unavailable
  }
}

export function restorePlayer() {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (!raw) return;
    const saved = JSON.parse(raw);
    if (saved?.src) Object.assign(playerStore, saved);
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
  persist();
}

export function clearPlayer() {
  playerStore.src = null;
  playerStore.title = null;
  playerStore.torrentId = null;
  playerStore.fileId = null;
  playerStore.infoHash = null;
  persist();
}
