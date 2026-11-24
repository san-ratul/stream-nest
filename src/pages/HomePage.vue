<script setup>
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { torrentStore, torrentLive } from "../stores/torrents";
import { openInPlayer } from "../stores/player";
import { streamUrl } from "../stores/torrents";
import TorrentRow from "../components/common/TorrentRow.vue";
import AppState from "../components/common/AppState.vue";
import MediaCard from "../components/common/MediaCard.vue";
import {
  catalogStore,
  loadHomeSections,
  loadCatalogStatus,
  loadFavorites,
  startSync,
  isSyncRunning,
  imageUrl,
  SIZES,
} from "../stores/catalog";

const router = useRouter();
const history = ref([]);
const loadingHistory = ref(true);
const cwPosters = ref({}); // media_key -> poster URL

function parseHistoryKey(mediaKey) {
  const parts = mediaKey.split(":");
  if (parts[0] === "tmdb") {
    if (parts.length === 3) return { tmdb: true, mediaType: parts[1], id: Number(parts[2]) };
    if (parts.length === 5) {
      return {
        tmdb: true,
        mediaType: "tv",
        id: Number(parts[1] ? parts[2] : parts[2]),
        season: Number(parts[3]),
        episode: Number(parts[4]),
      };
    }
  }
  return { tmdb: false };
}

function cwLabel(row) {
  const parsed = parseHistoryKey(row.media_key);
  if (parsed.tmdb && parsed.season != null) {
    return `S${String(parsed.season).padStart(2, "0")}E${String(parsed.episode).padStart(2, "0")}`;
  }
  return null;
}

async function loadHistory() {
  try {
    const rows = await invoke("history_list", { limit: 12 });
    history.value = (rows ?? []).filter(
      (r) => r.position_seconds > 10 && r.duration_seconds > 0 && r.position_seconds < r.duration_seconds * 0.95
    );
    // Posters for TMDB-backed rows from the local catalog.
    for (const row of history.value) {
      const parsed = parseHistoryKey(row.media_key);
      if (parsed.tmdb && cwPosters.value[row.media_key] == null) {
        try {
          const card = await invoke("catalog_card_lookup", {
            mediaType: parsed.mediaType,
            tmdbId: parsed.id,
          });
          cwPosters.value = {
            ...cwPosters.value,
            [row.media_key]: card?.poster_path
              ? await imageUrl(card.poster_path, "poster", SIZES.thumb)
              : null,
          };
        } catch {
          cwPosters.value = { ...cwPosters.value, [row.media_key]: null };
        }
      }
    }
  } catch {
    history.value = [];
  } finally {
    loadingHistory.value = false;
  }
}

onMounted(async () => {
  loadHistory();
  loadFavorites();
  await loadCatalogStatus();
  if (catalogStore.status?.db?.movies > 0 || catalogStore.status?.db?.tv_shows > 0) {
    loadHomeSections();
  }
});

function openCard(card) {
  router.push(card.media_type === "tv" ? `/tv/${card.tmdb_id}` : `/movie/${card.tmdb_id}`);
}

async function resumeHistory(row) {
  // TMDB keys route back to the details page: source availability is re-checked
  // there and the player resumes from the saved position automatically.
  const parsed = parseHistoryKey(row.media_key);
  if (parsed.tmdb) {
    router.push(parsed.mediaType === "tv" ? `/tv/${parsed.id}` : `/movie/${parsed.id}`);
    return;
  }
  // Torrent keys: "{info_hash}:{file_index}" — resume the existing stream.
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

function remainingOf(row) {
  if (!row.duration_seconds) return null;
  const left = Math.max(0, Math.round(row.duration_seconds - row.position_seconds));
  if (left < 60) return `${left}s`;
  if (left < 3600) return `${Math.round(left / 60)} min`;
  return `${Math.floor(left / 3600)}h ${Math.round((left % 3600) / 60)}m`;
}

function fmtClock(s) {
  const m = Math.floor(s / 60);
  const ss = Math.floor(s % 60);
  return `${m}:${String(ss).padStart(2, "0")}`;
}

const activeTorrents = computed(() => torrentStore.torrents.filter((t) => torrentLive(t)));
const recentTorrents = computed(() => torrentStore.torrents.slice(0, 5));
const orderedSections = computed(() =>
  [...catalogStore.sections].sort((a, b) => a.sort_order - b.sort_order)
);
const catalogEmpty = computed(
  () =>
    !catalogStore.loadingSections &&
    orderedSections.value.length === 0
);

async function triggerSync() {
  try {
    await startSync();
    // Poll briefly so the button reflects state; full progress lives in Settings.
    const poll = setInterval(async () => {
      await loadCatalogStatus();
      if (!isSyncRunning()) {
        clearInterval(poll);
        loadHomeSections();
      }
    }, 2000);
  } catch (err) {
    console.warn("sync start failed", err);
  }
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-slate-100">Home</h1>
        <p class="mt-1 text-sm text-slate-400">Discover metadata. Stream what you legally can.</p>
      </div>
      <div class="flex items-center gap-2">
        <button
          class="rounded-md border border-slate-600 bg-slate-800/60 px-3 py-2 text-sm font-medium text-slate-200 hover:bg-slate-700"
          @click="router.push('/search')"
        >
          ⌕ Search
        </button>
        <button
          class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400"
          @click="router.push('/add')"
        >
          + Add torrent
        </button>
      </div>
    </header>

    <AppState
      v-if="torrentStore.engineError"
      state="error"
      title="Torrent engine failed to start"
      :message="torrentStore.engineError"
    />

    <div v-else class="flex flex-1 flex-col gap-6 overflow-y-auto pb-4">
      <section>
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Continue watching</h2>
        <div v-if="loadingHistory" class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Loading history...
        </div>
        <div v-else-if="history.length" class="grid grid-cols-2 gap-3">
          <div
            v-for="row in history"
            :key="row.id"
            class="flex gap-3 rounded-lg border border-slate-700/60 bg-slate-800/40 p-3"
          >
            <div class="h-20 w-14 shrink-0 overflow-hidden rounded bg-slate-800">
              <img
                v-if="cwPosters[row.media_key]"
                :src="cwPosters[row.media_key]"
                :alt="row.title"
                class="h-full w-full object-cover"
                @error="$event.target.style.display = 'none'"
              />
              <div v-else class="flex h-full items-center justify-center text-slate-600">🎬</div>
            </div>
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium text-slate-100">{{ row.title }}</div>
              <div v-if="cwLabel(row)" class="text-xs text-sky-300">{{ cwLabel(row) }}</div>
              <div class="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-slate-700/70">
                <div class="h-full rounded-full bg-sky-400" :style="{ width: `${progressOf(row)}%` }"></div>
              </div>
              <div class="mt-2 flex items-center justify-between">
                <span class="text-xs text-slate-400">
                  {{ fmtClock(row.position_seconds) }} / {{ fmtClock(row.duration_seconds) }}
                  <span v-if="remainingOf(row)"> · {{ remainingOf(row) }} left</span>
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
        </div>
        <div v-else class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Nothing in progress yet. Positions are saved automatically while you watch.
        </div>
      </section>

      <div v-if="catalogStore.status && !catalogStore.status.has_api_key"
        class="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-200">
        Metadata discovery is inactive: no TMDB API key configured.
        Create a <code class="rounded bg-black/30 px-1">.env</code> file at one of these locations containing
        <code class="rounded bg-black/30 px-1">TMDB_API_KEY=your_key</code>, then restart the app:
        <div class="mt-1.5 flex flex-col gap-0.5 text-xs text-amber-200/80">
          <code v-for="p in catalogStore.status.env_paths ?? []" :key="p" class="truncate">{{ p }}</code>
        </div>
        Then run a sync from Settings.
      </div>
      <div v-else-if="catalogStore.status && catalogStore.status.db.movies === 0 && catalogStore.status.db.tv_shows === 0 && !isSyncRunning()"
        class="flex items-center justify-between rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3 text-sm text-slate-300">
        <span>The local catalog is empty. Synchronize with TMDB to populate discovery sections.</span>
        <button class="rounded-md bg-sky-500/20 px-3 py-1.5 text-xs font-medium text-sky-300 hover:bg-sky-500/30" @click="triggerSync">
          Sync now
        </button>
      </div>
      <div v-else-if="isSyncRunning()"
        class="rounded-lg border border-sky-500/30 bg-sky-500/10 px-4 py-3 text-sm text-sky-200">
        Catalog synchronization in progress: {{ catalogStore.status.phase.detail?.step || "working" }}…
        Track it in Settings.
      </div>

      <AppState
        v-if="catalogStore.sectionsError"
        state="error"
        title="Discovery sections failed to load"
        :message="catalogStore.sectionsError"
      />
      <template v-else>
        <section v-if="catalogStore.loadingSections" class="grid grid-cols-3 gap-4 lg:grid-cols-6">
          <div v-for="i in 6" :key="i" class="animate-pulse">
            <div class="aspect-[2/3] w-full rounded-lg bg-slate-800/60"></div>
            <div class="mt-2 h-3 w-3/4 rounded bg-slate-800/60"></div>
            <div class="mt-1 h-2 w-1/2 rounded bg-slate-800/40"></div>
          </div>
        </section>

        <template v-else>
          <section v-for="section in orderedSections" :key="section.key">
            <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">{{ section.title }}</h2>
            <div class="flex gap-3 overflow-x-auto pb-2">
              <div class="w-[150px] shrink-0" v-for="card in section.items" :key="`${card.media_type}-${card.tmdb_id}`">
                <MediaCard :card="card" @open="openCard" />
              </div>
            </div>
          </section>
        </template>
      </template>

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
