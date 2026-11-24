<script setup>
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { torrentStore, torrentLive } from "../stores/torrents";
import { openInPlayer, mediaKey } from "../stores/player";
import { streamUrl } from "../stores/torrents";
import TorrentRow from "../components/common/TorrentRow.vue";
import AppState from "../components/common/AppState.vue";

const router = useRouter();
const history = ref([]);
const loadingHistory = ref(true);

async function loadHistory() {
  try {
    const rows = await invoke("history_list", { limit: 12 });
    history.value = (rows ?? []).filter(
      (r) => r.position_seconds > 10 && r.duration_seconds > 0 && r.position_seconds < r.duration_seconds * 0.95
    );
  } catch {
    history.value = [];
  } finally {
    loadingHistory.value = false;
  }
}

onMounted(loadHistory);

async function resumeHistory(row) {
  const parts = row.media_key.split(":");
  if (parts.length !== 2) return;
  const [hash, fileIdx] = parts;
  try {
    const url = await streamUrl(hash, Number(fileIdx));
    openInPlayer({
      src: url,
      title: row.title,
      torrentId: hash,
      fileId: Number(fileIdx),
      infoHash: hash,
    });
    router.push("/player");
  } catch (err) {
    console.warn("resume failed", err);
  }
}

function progressOf(row) {
  if (!row.duration_seconds) return 0;
  return Math.min(100, Math.round((row.position_seconds / row.duration_seconds) * 100));
}

function fmtClock(s) {
  const m = Math.floor(s / 60);
  const ss = Math.floor(s % 60);
  return `${m}:${String(ss).padStart(2, "0")}`;
}

const activeTorrents = computed(() => torrentStore.torrents.filter((t) => torrentLive(t)));
const recentTorrents = computed(() => torrentStore.torrents.slice(0, 5));
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-slate-100">Home</h1>
        <p class="mt-1 text-sm text-slate-400">Your media, your device, your control.</p>
      </div>
      <button
        class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400"
        @click="router.push('/add')"
      >
        + Add torrent
      </button>
    </header>

    <AppState
      v-if="torrentStore.engineError"
      state="error"
      title="Torrent engine failed to start"
      :message="torrentStore.engineError"
    />

    <div v-else class="flex flex-1 flex-col gap-6 overflow-y-auto">
      <section>
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Continue watching</h2>
        <div v-if="loadingHistory" class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Loading history...
        </div>
        <div v-else-if="history.length" class="grid grid-cols-2 gap-3">
          <div
            v-for="row in history"
            :key="row.id"
            class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-3"
          >
            <div class="truncate text-sm font-medium text-slate-100">{{ row.title }}</div>
            <div class="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-slate-700/70">
              <div class="h-full rounded-full bg-sky-400" :style="{ width: `${progressOf(row)}%` }"></div>
            </div>
            <div class="mt-2 flex items-center justify-between">
              <span class="text-xs text-slate-400">
                {{ fmtClock(row.position_seconds) }} / {{ fmtClock(row.duration_seconds) }} · {{ progressOf(row) }}%
              </span>
              <button
                class="rounded bg-sky-500/20 px-2 py-0.5 text-xs font-medium text-sky-300 hover:bg-sky-500/30"
                @click="resumeHistory(row)"
              >
                ▶ Resume
              </button>
            </div>
          </div>
        </div>
        <div v-else class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Nothing in progress yet. Positions are saved automatically while you watch.
        </div>
      </section>

      <section>
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Recent torrents
        </h2>
        <div v-if="recentTorrents.length" class="flex flex-col gap-2">
          <TorrentRow
            v-for="t in recentTorrents"
            :key="t.id ?? t.info_hash"
            :torrent="t"
            @open="router.push(`/torrent/${t.id ?? t.info_hash}`)"
          />
        </div>
        <div v-else-if="torrentStore.engineReady" class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          No torrents yet. Paste a magnet link or import a .torrent file to start streaming.
        </div>
        <div v-else class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Waiting for the torrent engine...
        </div>
      </section>

      <section v-if="activeTorrents.length">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Active now ({{ activeTorrents.length }})
        </h2>
        <div class="flex flex-col gap-2">
          <TorrentRow
            v-for="t in activeTorrents"
            :key="`active-${t.id ?? t.info_hash}`"
            :torrent="t"
            @open="router.push(`/torrent/${t.id ?? t.info_hash}`)"
          />
        </div>
      </section>
    </div>
  </div>
</template>
