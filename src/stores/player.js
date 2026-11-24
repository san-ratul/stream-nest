import { reactive } from "vue";

export const playerStore = reactive({
  src: null,
  title: null,
  torrentId: null,
  fileId: null,
  infoHash: null,
});

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
}

export function clearPlayer() {
  playerStore.src = null;
  playerStore.title = null;
  playerStore.torrentId = null;
  playerStore.fileId = null;
  playerStore.infoHash = null;
}
