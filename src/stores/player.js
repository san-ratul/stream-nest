import { reactive } from "vue";

export const playerStore = reactive({
  src: null,
  title: null,
  torrentId: null,
  fileId: null,
});

export function openInPlayer({ src, title, torrentId, fileId }) {
  playerStore.src = src;
  playerStore.title = title ?? null;
  playerStore.torrentId = torrentId ?? null;
  playerStore.fileId = fileId ?? null;
}

export function clearPlayer() {
  playerStore.src = null;
  playerStore.title = null;
  playerStore.torrentId = null;
  playerStore.fileId = null;
}
