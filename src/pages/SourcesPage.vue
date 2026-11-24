<script setup>
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "vue-router";
import AppState from "../components/common/AppState.vue";
import { sourceStore, loadSources, sourceTypeLabel, fmtBytes } from "../stores/sources";
import { startRecordPlayback, playbackStore } from "../composables/playback";

const router = useRouter();
const filter = ref("all"); // all | recently_added | recently_used | associated
const expanded = ref(null);
const busyId = ref(null);
const rowError = ref(null);
const associateFor = ref(null); // record being associated
const assocQuery = ref("");
const assocResults = ref(null);
const copied = ref(null);

const rows = computed(() => {
  const list = sourceStore.list ?? [];
  if (filter.value === "recently_added") {
    return [...list].sort((a, b) => (b.date_added ?? "").localeCompare(a.date_added ?? "")).slice(0, 20);
  }
  if (filter.value === "recently_used") {
    return list.filter((r) => r.last_used_at).slice(0, 20);
  }
  if (filter.value === "associated") {
    return list.filter((r) => r.tmdb_id);
  }
  return list;
});

onMounted(loadSources);

async function playRecord(record) {
  busyId.value = record.id;
  rowError.value = null;
  try {
    const result = await startRecordPlayback(record, {
      title: record.display_name,
      onNeedFileChoice: fileChooser,
    });
    if (result?.started) router.push("/player");
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
}

const chooserVideos = ref(null);
let chooserResolve = null;
function fileChooser(videos) {
  return new Promise((resolve) => {
    chooserVideos.value = videos;
    chooserResolve = resolve;
  });
}
function resolveChooser(idx) {
  chooserVideos.value = null;
  chooserResolve?.(idx);
  chooserResolve = null;
}
function cancelChooser() {
  chooserVideos.value = null;
  chooserResolve?.(null);
  chooserResolve = null;
}

async function removeRecord(record) {
  busyId.value = record.id;
  rowError.value = null;
  try {
    await invoke("source_remove", { id: record.id });
    await loadSources();
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
}

async function copyMagnet(record) {
  if (!record.magnet_uri) return;
  try {
    await navigator.clipboard.writeText(record.magnet_uri);
    copied.value = record.id;
    setTimeout(() => (copied.value = null), 2000);
  } catch {
    rowError.value = "Clipboard unavailable";
  }
}

async function retryMetadata(record) {
  busyId.value = record.id;
  rowError.value = null;
  try {
    await invoke("playback_prepare_record", { sourceId: record.id });
    await loadSources();
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
}

function openDetails(record) {
  expanded.value = expanded.value === record.id ? null : record.id;
}

function openAssociation(record) {
  if (record.tmdb_id) {
    router.push(record.tmdb_media_type === "tv" ? `/tv/${record.tmdb_id}` : `/movie/${record.tmdb_id}`);
    return;
  }
  associateFor.value = record;
  assocResults.value = null;
  assocQuery.value = "";
}

async function searchForAssociation() {
  const q = assocQuery.value.trim();
  if (!q) return;
  try {
    assocResults.value = await invoke("catalog_search", { query: q, filter: "all", limit: 8 });
  } catch {
    assocResults.value = null;
  }
}

async function confirmAssociation(record, card) {
  try {
    await invoke("source_associate", {
      id: record.id,
      tmdbMediaType: card.media_type,
      tmdbId: card.tmdb_id,
      seasonNumber: null,
      episodeNumber: null,
    });
    associateFor.value = null;
    await loadSources();
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  }
}

async function clearAssociation(record) {
  try {
    await invoke("source_associate", {
      id: record.id,
      tmdbMediaType: null,
      tmdbId: null,
      seasonNumber: null,
      episodeNumber: null,
    });
    await loadSources();
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  }
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-slate-100">Sources</h1>
        <p class="mt-1 text-sm text-slate-400">Torrent sources you added manually or from authorized providers.</p>
      </div>
      <button class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400" @click="router.push('/add')">
        + Add source
      </button>
    </header>

    <div class="mb-4 flex flex-wrap gap-2">
      <button
        v-for="f in [
          { key: 'all', label: 'All' },
          { key: 'recently_added', label: 'Recently added' },
          { key: 'recently_used', label: 'Recently used' },
          { key: 'associated', label: 'Linked to catalog' },
        ]"
        :key="f.key"
        class="rounded-md px-2.5 py-1 text-xs font-medium"
        :class="filter === f.key ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'"
        @click="filter = f.key"
      >
        {{ f.label }}
      </button>
    </div>

    <p v-if="rowError" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ rowError }}</p>

    <div v-if="sourceStore.loading" class="flex flex-1 items-center justify-center">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-sky-400"></div>
    </div>

    <AppState
      v-else-if="!rows.length"
      state="empty"
      title="No sources yet"
      message="Add a magnet link, torrent URL, local .torrent file or supported provider page."
    >
      <button class="rounded-md bg-sky-500/20 px-4 py-2 text-sm text-sky-300 hover:bg-sky-500/30" @click="router.push('/add')">
        Add your first source
      </button>
    </AppState>

    <div v-else class="flex flex-1 flex-col gap-2 overflow-y-auto pb-4">
      <div
        v-for="row in rows"
        :key="row.id"
        class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-3"
      >
        <div class="flex items-center gap-3">
          <span class="rounded bg-slate-700/60 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-slate-300">
            {{ sourceTypeLabel(row.source_type) }}
          </span>
          <div class="min-w-0 flex-1">
            <div class="truncate text-sm font-medium text-slate-100">{{ row.display_name }}</div>
            <div class="text-xs text-slate-500">
              {{ row.info_hash ? row.info_hash.slice(0, 16) + "…" : "hash unknown" }}
              <template v-if="row.total_size"> · {{ fmtBytes(row.total_size) }}</template>
              <template v-if="row.tmdb_id">
                ·
                <button class="text-sky-400 hover:underline" @click="router.push(row.tmdb_media_type === 'tv' ? `/tv/${row.tmdb_id}` : `/movie/${row.tmdb_id}`)">
                  linked to {{ row.tmdb_media_type === "tv" ? "show" : "movie" }} #{{ row.tmdb_id
                  }}<template v-if="row.season_number != null"> S{{ String(row.season_number).padStart(2, "0") }}<template v-if="row.episode_number != null">E{{ String(row.episode_number).padStart(2, "0") }}</template></template>
                </button>
              </template>
            </div>
            <div class="text-[11px] text-slate-600">
              added {{ row.date_added }}<template v-if="row.last_used_at"> · last used {{ row.last_used_at }}</template>
            </div>
          </div>
          <div class="flex shrink-0 items-center gap-1.5">
            <button
              class="rounded bg-emerald-500/20 px-2 py-1 text-xs font-medium text-emerald-300 hover:bg-emerald-500/30 disabled:opacity-50"
              :disabled="busyId === row.id || playbackStore.stage === 'starting'"
              @click="playRecord(row)"
            >
              ▶ Play
            </button>
            <button class="rounded bg-slate-800 px-2 py-1 text-xs text-slate-400 hover:bg-slate-700" @click="openDetails(row)">
              {{ expanded === row.id ? "hide" : "details" }}
            </button>
          </div>
        </div>

        <div v-if="expanded === row.id" class="mt-2 border-t border-slate-700/60 pt-2">
          <div class="flex flex-wrap gap-2 text-xs">
            <button v-if="row.magnet_uri" class="rounded bg-slate-800 px-2 py-1 text-slate-300 hover:bg-slate-700" @click="copyMagnet(row)">
              {{ copied === row.id ? "✓ copied" : "Copy magnet URI" }}
            </button>
            <button class="rounded bg-slate-800 px-2 py-1 text-slate-300 hover:bg-slate-700" :disabled="busyId === row.id" @click="retryMetadata(row)">
              Retry metadata retrieval
            </button>
            <button class="rounded bg-slate-800 px-2 py-1 text-slate-300 hover:bg-slate-700" @click="openAssociation(row)">
              {{ row.tmdb_id ? "Open linked media" : "Associate with a movie/show" }}
            </button>
            <button v-if="row.tmdb_id" class="rounded bg-slate-800 px-2 py-1 text-slate-400 hover:bg-slate-700" @click="clearAssociation(row)">
              Clear link
            </button>
            <button class="rounded bg-red-500/10 px-2 py-1 text-red-300 hover:bg-red-500/20" :disabled="busyId === row.id" @click="removeRecord(row)">
              Remove source
            </button>
          </div>
          <div class="mt-2 text-[11px] text-slate-600">
            <template v-if="row.torrent_url">source URL: {{ row.torrent_url }}</template>
            <template v-else-if="row.magnet_uri">magnet: {{ row.magnet_uri.slice(0, 60) }}…</template>
            <template v-else>imported torrent file</template>
          </div>
        </div>
      </div>
    </div>

    <!-- Association picker (local catalog search only) -->
    <div
      v-if="associateFor"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6"
      @click.self="associateFor = null"
    >
      <div class="w-full max-w-xl rounded-lg border border-slate-700 bg-slate-900 p-4">
        <h3 class="text-lg font-semibold text-slate-100">Link source to catalog media</h3>
        <p class="mt-1 text-xs text-slate-400">Search your synchronized local catalog and pick the matching title.</p>
        <input
          v-model="assocQuery"
          placeholder="Search movies & shows…"
          class="mt-2 w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
          @input="searchForAssociation"
        />
        <div class="mt-2 flex max-h-72 flex-col gap-1 overflow-y-auto">
          <template v-if="assocResults">
            <button
              v-for="card in [...(assocResults.movies ?? []), ...(assocResults.tv ?? [])]"
              :key="`${card.media_type}-${card.tmdb_id}`"
              class="flex items-center justify-between rounded border border-slate-700/60 px-3 py-2 text-left text-sm text-slate-200 hover:border-sky-500/50 hover:bg-slate-800"
              @click="confirmAssociation(associateFor, card)"
            >
              <span class="truncate">{{ card.title }}</span>
              <span class="shrink-0 text-xs text-slate-500">{{ card.year || "" }} · {{ card.media_type }}</span>
            </button>
            <p v-if="!(assocResults.movies?.length || assocResults.tv?.length)" class="px-2 py-3 text-center text-xs text-slate-500">
              No local catalog matches. Sync the catalog from Settings first.
            </p>
          </template>
        </div>
        <div class="mt-3 flex justify-end">
          <button class="rounded bg-slate-700 px-3 py-1.5 text-sm text-slate-200 hover:bg-slate-600" @click="associateFor = null">
            Cancel
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
