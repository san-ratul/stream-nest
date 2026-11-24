<script setup>
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import {
  torrentStore,
  torrentLive,
  torrentName,
  torrentProgress,
  fmtBytes,
  fmtSpeed,
  torrentPause,
  torrentStart,
  torrentDelete,
  torrentForget,
} from "../stores/torrents";
import TorrentRow from "../components/common/TorrentRow.vue";
import AppState from "../components/common/AppState.vue";

const router = useRouter();
const busyId = ref(null);
const error = ref(null);
const filter = ref("all"); // all | active | paused | complete

const filtered = computed(() =>
  torrentStore.torrents.filter((t) => {
    if (filter.value === "active") return torrentLive(t);
    if (filter.value === "paused") return !torrentLive(t) && torrentProgress(t) < 100;
    if (filter.value === "complete") return torrentProgress(t) >= 100;
    return true;
  })
);

const totals = computed(() => {
  let down = 0;
  let up = 0;
  let totalBytes = 0;
  let haveBytes = 0;
  for (const t of torrentStore.torrents) {
    down += Number(t?.stats?.live?.downloaded ?? 0) || 0;
    up += Number(t?.stats?.live?.uploaded ?? 0) || 0;
    totalBytes += Number(t?.stats?.total_bytes ?? 0) || 0;
    haveBytes += Number(t?.stats?.downloaded_bytes ?? 0) || 0;
  }
  return { down, up, totalBytes, haveBytes };
});

async function act(id, fn) {
  busyId.value = id;
  error.value = null;
  try {
    await fn(id);
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4 flex items-center justify-between">
      <h1 class="text-2xl font-bold text-slate-100">Download Manager</h1>
      <button class="rounded-md bg-slate-700 px-3 py-1.5 text-sm text-slate-200 hover:bg-slate-600" @click="router.push('/add')">
        + Add
      </button>
    </header>

    <div class="mb-4 grid grid-cols-4 gap-3">
      <div class="rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
        <div class="text-xs uppercase tracking-wide text-slate-400">Download</div>
        <div class="mt-1 text-lg font-semibold text-sky-300">{{ fmtSpeed(totals.down) }}</div>
      </div>
      <div class="rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
        <div class="text-xs uppercase tracking-wide text-slate-400">Upload</div>
        <div class="mt-1 text-lg font-semibold text-violet-300">{{ fmtSpeed(totals.up) }}</div>
      </div>
      <div class="rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
        <div class="text-xs uppercase tracking-wide text-slate-400">Downloaded</div>
        <div class="mt-1 text-lg font-semibold text-slate-200">{{ fmtBytes(totals.haveBytes) }}</div>
      </div>
      <div class="rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
        <div class="text-xs uppercase tracking-wide text-slate-400">Torrents</div>
        <div class="mt-1 text-lg font-semibold text-slate-200">{{ torrentStore.torrents.length }}</div>
      </div>
    </div>

    <div class="mb-3 flex gap-2">
      <button
        v-for="f in ['all', 'active', 'paused', 'complete']"
        :key="f"
        class="rounded-md px-3 py-1 text-xs font-medium capitalize"
        :class="filter === f ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'"
        @click="filter = f"
      >
        {{ f }}
      </button>
    </div>

    <p v-if="error" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ error }}</p>

    <div class="flex flex-1 flex-col gap-2 overflow-y-auto">
      <template v-if="filtered.length">
        <div v-for="t in filtered" :key="t.id ?? t.info_hash" class="flex flex-col gap-1">
          <TorrentRow :torrent="t" @open="router.push(`/torrent/${t.id ?? t.info_hash}`)" />
          <div class="flex gap-2 pl-2">
            <button
              v-if="!torrentLive(t)"
              class="rounded bg-slate-800 px-2 py-0.5 text-xs text-slate-300 hover:bg-slate-700 disabled:opacity-50"
              :disabled="busyId === (t.id ?? t.info_hash)"
              @click="act(t.id ?? t.info_hash, torrentStart)"
            >start</button>
            <button
              v-else
              class="rounded bg-slate-800 px-2 py-0.5 text-xs text-slate-300 hover:bg-slate-700 disabled:opacity-50"
              :disabled="busyId === (t.id ?? t.info_hash)"
              @click="act(t.id ?? t.info_hash, torrentPause)"
            >pause</button>
            <button
              class="rounded bg-slate-800 px-2 py-0.5 text-xs text-slate-400 hover:bg-slate-700 disabled:opacity-50"
              :disabled="busyId === (t.id ?? t.info_hash)"
              @click="act(t.id ?? t.info_hash, torrentForget)"
            >forget</button>
            <button
              class="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-300 hover:bg-red-500/25 disabled:opacity-50"
              :disabled="busyId === (t.id ?? t.info_hash)"
              @click="act(t.id ?? t.info_hash, torrentDelete)"
            >delete + files</button>
          </div>
        </div>
      </template>
      <AppState
        v-else
        state="empty"
        title="No downloads"
        message="Add a magnet link or .torrent file to start downloading or streaming."
      >
        <button class="rounded-md bg-sky-500 px-4 py-1.5 text-sm font-medium text-slate-950 hover:bg-sky-400" @click="router.push('/add')">
          Add torrent
        </button>
      </AppState>
    </div>

    <p class="pt-3 text-xs text-slate-500">
      Cache usage and cleanup controls arrive with the cache management milestone (Phase 7). Torrents download to the configured download directory.
    </p>
  </div>
</template>
