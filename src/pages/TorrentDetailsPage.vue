<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import {
  torrentDetails,
  torrentPause,
  torrentStart,
  torrentDelete,
  torrentForget,
  torrentUpdateOnlyFiles,
  streamUrl,
  fmtBytes,
} from "../stores/torrents";
import { openInPlayer } from "../stores/player";
import AppState from "../components/common/AppState.vue";

const route = useRoute();
const router = useRouter();

const details = ref(null);
const stats = ref(null);
const peers = ref({});
const error = ref(null);
const busy = ref(false);
const selected = ref(new Set());
const streamTarget = ref(null);
const isFavorite = ref(false);
let pollTimer = null;

const VIDEO_EXTS = ["mp4", "mkv", "avi", "webm", "mov", "m4v", "ts"];

const id = computed(() => route.params.id);

const files = computed(() => details.value?.files ?? []);
const hasMetadata = computed(() => files.value.length > 0 || details.value?.name);

function fileExt(name) {
  const m = String(name ?? "").toLowerCase().match(/\.([a-z0-9]+)$/);
  return m ? m[1] : "";
}
function isVideo(file) {
  return VIDEO_EXTS.includes(fileExt(file?.name));
}

async function refresh() {
  try {
    details.value = await torrentDetails(id.value);
    if (selected.value.size === 0 && files.value.length) {
      files.value.forEach((f, i) => {
        if (isVideo(f)) selected.value.add(i);
      });
    }
    const hash = details.value?.info_hash;
    if (hash && !registered) {
      registered = true;
      invoke("db_upsert_torrent", {
        infoHash: hash,
        name: details.value?.name ?? "unknown",
        outputFolder: details.value?.output_folder ?? null,
      }).catch(() => {});
      const favs = await invoke("favorites_list");
      isFavorite.value = (favs ?? []).some((f) => f.media_key === hash);
    }
  } catch (err) {
    error.value = String(err?.message ?? err);
  }
}

let registered = false;

async function toggleFavorite() {
  const hash = details.value?.info_hash;
  if (!hash) return;
  try {
    if (isFavorite.value) {
      await invoke("favorites_remove", { mediaKey: hash });
      isFavorite.value = false;
    } else {
      await invoke("favorites_add", {
        mediaKey: hash,
        title: details.value?.name ?? "video",
      });
      isFavorite.value = true;
    }
  } catch (err) {
    error.value = String(err?.message ?? err);
  }
}

async function refreshStats() {
  try {
    stats.value = await invoke("torrent_stats", { id: id.value });
    peers.value = await invoke("torrent_peer_stats", {
      id: id.value,
      filter: { state: "live" },
    });
  } catch {
    stats.value = null;
  }
}

onMounted(() => {
  refresh();
  pollTimer = setInterval(() => {
    refresh();
    refreshStats();
  }, 2000);
});
onUnmounted(() => clearInterval(pollTimer));

async function doAction(fn, successNote) {
  busy.value = true;
  error.value = null;
  try {
    await fn();
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    busy.value = false;
  }
}

const pauseTorrent = () => doAction(() => torrentPause(id.value));
const startTorrent = () => doAction(() => torrentStart(id.value));
const removeTorrent = () =>
  doAction(async () => {
    await torrentDelete(id.value);
    router.push("/downloads");
  }, "removed with files");
const forgetTorrent = () =>
  doAction(async () => {
    await torrentForget(id.value);
    router.push("/downloads");
  });

function toggleFile(index) {
  if (selected.value.has(index)) selected.value.delete(index);
  else selected.value.add(index);
  selected.value = new Set(selected.value);
}

const saveSelection = () =>
  doAction(() => torrentUpdateOnlyFiles(id.value, [...selected.value].sort((a, b) => a - b)));

async function streamSelected() {
  const index = [...selected.value].sort((a, b) => a - b).find((i) => isVideo(files.value[i]));
  if (index == null) {
    error.value = "Select a video file to stream.";
    return;
  }
  busy.value = true;
  error.value = null;
  try {
    const url = await streamUrl(id.value, index);
    openInPlayer({
      src: url,
      title: files.value[index]?.name ?? details.value?.name ?? "stream",
      torrentId: id.value,
      fileId: index,
      infoHash: details.value?.info_hash ?? null,
    });
    router.push("/player");
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4 flex items-center justify-between">
      <div class="min-w-0">
        <h1 class="truncate text-2xl font-bold text-slate-100">
          {{ details?.name || (hasMetadata ? "Torrent" : "Retrieving metadata...") }}
        </h1>
        <p class="mt-1 font-mono text-xs text-slate-500">id: {{ id }}</p>
      </div>
      <button class="rounded-md bg-slate-700 px-3 py-1.5 text-sm text-slate-200 hover:bg-slate-600" @click="router.back()">
        Back
      </button>
    </header>

    <AppState v-if="error && !details" state="error" title="Failed to load torrent" :message="error" />

    <div v-else class="flex flex-1 flex-col gap-4 overflow-y-auto">
      <div class="flex flex-wrap gap-2">
        <button class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400 disabled:opacity-50" :disabled="busy" @click="startTorrent">
          Start
        </button>
        <button class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600 disabled:opacity-50" :disabled="busy" @click="pauseTorrent">
          Pause
        </button>
        <button
          class="rounded-md bg-sky-500/90 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400 disabled:opacity-50"
          :disabled="busy || !hasMetadata"
          @click="streamSelected"
        >
          ▶ Stream selected video
        </button>
        <button class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600 disabled:opacity-50" :disabled="busy" @click="saveSelection">
          Save file selection
        </button>
        <button
          class="rounded-md px-4 py-2 text-sm hover:bg-slate-600 disabled:opacity-50"
          :class="isFavorite ? 'bg-amber-500/25 text-amber-200' : 'bg-slate-700 text-slate-200'"
          :disabled="busy"
          @click="toggleFavorite"
        >
          {{ isFavorite ? "★ Favorited" : "☆ Favorite" }}
        </button>
        <button class="rounded-md bg-red-500/20 px-4 py-2 text-sm text-red-300 hover:bg-red-500/30 disabled:opacity-50" :disabled="busy" @click="removeTorrent">
          Delete (with files)
        </button>
        <button class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-600 disabled:opacity-50" :disabled="busy" @click="forgetTorrent">
          Forget (keep files)
        </button>
      </div>

      <p v-if="error" class="rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ error }}</p>

      <section v-if="Object.keys(peers).length">
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Peers ({{ Object.keys(peers).length }} live)
        </h2>
        <div class="overflow-hidden rounded-lg border border-slate-700/60">
          <div class="grid grid-cols-[1fr_1fr_90px_110px_110px_70px] gap-2 border-b border-slate-800 bg-slate-800/60 px-4 py-2 text-xs uppercase tracking-wide text-slate-400">
            <span>Address</span><span>Client</span><span>State</span><span>Kind</span><span>Down / Up</span><span>Err</span>
          </div>
          <div
            v-for="(p, addr) in peers"
            :key="addr"
            class="grid grid-cols-[1fr_1fr_90px_110px_110px_70px] gap-2 border-b border-slate-800 px-4 py-2 text-sm text-slate-300 last:border-b-0"
          >
            <span class="truncate font-mono text-xs">{{ addr }}</span>
            <span class="truncate text-slate-400">{{ p.client_name ?? "unknown" }}</span>
            <span class="text-xs">{{ p.state ?? "?" }}</span>
            <span class="text-xs">{{ p.conn_kind ?? "?" }}</span>
            <span class="text-xs text-slate-400">
              ↓{{ fmtBytes(p.counters?.fetched_bytes) }} ↑{{ fmtBytes(p.counters?.uploaded_bytes) }}
            </span>
            <span class="text-xs" :class="p.counters?.errors ? 'text-red-300' : 'text-slate-500'">{{ p.counters?.errors ?? 0 }}</span>
          </div>
        </div>
      </section>

      <section v-if="hasMetadata">
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">Files</h2>
        <AppState
          v-if="!hasMetadata"
          state="loading"
          title="Retrieving metadata"
          message="Fetching info from peers. Large swarms can take a while; the UI stays responsive."
        />
        <div v-else class="overflow-hidden rounded-lg border border-slate-700/60">
          <label
            v-for="(file, index) in files"
            :key="index"
            class="flex cursor-pointer items-center gap-3 border-b border-slate-800 px-4 py-2.5 text-sm last:border-b-0 hover:bg-slate-800/50"
          >
            <input
              type="checkbox"
              class="h-4 w-4 accent-sky-500"
              :checked="selected.has(index)"
              @change="toggleFile(index)"
            />
            <span class="min-w-0 flex-1 truncate text-slate-200">{{ file.name }}</span>
            <span
              v-if="isVideo(file)"
              class="rounded bg-sky-500/15 px-1.5 py-0.5 text-xs text-sky-300"
            >video</span>
            <span class="shrink-0 text-xs text-slate-400">{{ fmtBytes(file.length ?? file.size) }}</span>
          </label>
        </div>
      </section>
    </div>
  </div>
</template>
