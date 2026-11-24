<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "vue-router";
import AppState from "../components/common/AppState.vue";
import MediaCard from "../components/common/MediaCard.vue";
import { loadFavorites } from "../stores/catalog";

const router = useRouter();
const query = ref("");
const filter = ref("all");
const results = ref(null); // { movies, tv, documentaries, legal_entries, source_note }
const state = ref("idle"); // idle | loading | results | empty | error
const error = ref(null);

const FILTERS = [
  { key: "all", label: "All" },
  { key: "movies", label: "Movies" },
  { key: "tv", label: "TV Shows" },
  { key: "documentaries", label: "Documentaries" },
  { key: "legal", label: "Legal catalog" },
];

let debounceTimer = null;
let lastQuery = "";

async function runSearch() {
  const q = query.value.trim();
  lastQuery = q;
  if (!q) {
    state.value = "idle";
    results.value = null;
    return;
  }
  state.value = "loading";
  error.value = null;
  try {
    const res = await invoke("catalog_search", {
      query: q,
      filter: filter.value === "all" ? "all" : filter.value,
      limit: 30,
    });
    if (query.value.trim() !== lastQuery) return; // stale response
    results.value = res;
    const total =
      (res.movies?.length ?? 0) +
      (res.tv?.length ?? 0) +
      (res.documentaries?.length ?? 0) +
      (res.legal_entries?.length ?? 0);
    state.value = total === 0 ? "empty" : "results";
  } catch (err) {
    error.value = String(err?.message ?? err);
    state.value = "error";
  }
}

function onInput() {
  // Debounce: never fire a request per keystroke.
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(runSearch, 350);
}

function setFilter(f) {
  filter.value = f;
  runSearch();
}

function openCard(card) {
  router.push(card.media_type === "tv" ? `/tv/${card.tmdb_id}` : `/movie/${card.tmdb_id}`);
}

function openLegal(entry) {
  // Legal catalog entries keep their existing behavior: open the official source page.
  if (entry.source_url) window.open(entry.source_url, "_blank", "noopener");
}

onMounted(loadFavorites);

function cardList(key) {
  if (filter.value === "all") return results.value?.[key] ?? [];
  return filter.value === key ? results.value?.[key] ?? [] : [];
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4">
      <h1 class="text-2xl font-bold text-slate-100">Search</h1>
      <p class="mt-1 text-sm text-slate-400">
        Local catalog first. Metadata from TMDB; playback only for authorized sources.
      </p>
    </header>

    <div class="mb-4 flex flex-wrap items-center gap-2">
      <input
        v-model="query"
        placeholder="Search movies, TV shows…"
        class="w-72 rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
        @input="onInput"
        @keyup.enter="runSearch"
      />
      <button
        v-for="f in FILTERS"
        :key="f.key"
        class="rounded-md px-2.5 py-1 text-xs font-medium"
        :class="filter === f.key ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'"
        @click="setFilter(f.key)"
      >
        {{ f.label }}
      </button>
    </div>

    <div v-if="state === 'idle'" class="flex flex-1 items-center justify-center">
      <p class="text-sm text-slate-500">Type to search the synchronized catalog.</p>
    </div>

    <div v-else-if="state === 'loading'" class="flex flex-1 flex-col gap-4 overflow-hidden">
      <div class="grid grid-cols-3 gap-4 lg:grid-cols-6">
        <div v-for="i in 6" :key="i" class="animate-pulse">
          <div class="aspect-[2/3] w-full rounded-lg bg-slate-800/60"></div>
          <div class="mt-2 h-3 w-3/4 rounded bg-slate-800/60"></div>
        </div>
      </div>
    </div>

    <div v-else-if="state === 'error'" class="flex flex-1 flex-col">
      <AppState state="error" title="Search failed" :message="error" />
    </div>

    <div v-else-if="state === 'empty'" class="flex flex-1 flex-col">
      <AppState
        state="empty"
        title="No results"
        :message="`Nothing in the local catalog matches “${query}”. Run a TMDB sync from Settings for wider coverage.`"
      />
    </div>

    <div v-else class="flex flex-1 flex-col gap-6 overflow-y-auto pb-4">
      <template v-for="key in ['movies', 'tv', 'documentaries']" :key="key">
        <section v-if="cardList(key).length">
          <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
            {{ key === "tv" ? "TV Shows" : key === "movies" ? "Movies" : "Documentaries" }}
            <span class="ml-1 text-slate-500">({{ cardList(key).length }})</span>
          </h2>
          <div class="grid grid-cols-3 gap-4 lg:grid-cols-6">
            <MediaCard
              v-for="card in cardList(key)"
              :key="`${card.media_type}-${card.tmdb_id}`"
              :card="card"
              @open="openCard"
            />
          </div>
        </section>
      </template>

      <section v-if="results?.legal_entries?.length">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Rights-verified catalog
          <span class="ml-1 text-slate-500">({{ results.legal_entries.length }})</span>
        </h2>
        <div class="flex flex-col gap-2">
          <div
            v-for="e in results.legal_entries"
            :key="e.id"
            class="cursor-pointer rounded-lg border border-slate-700/60 bg-slate-800/40 p-3 hover:border-sky-500/40"
            @click="openLegal(e)"
          >
            <div class="flex items-center justify-between">
              <div class="truncate text-sm font-medium text-slate-100">{{ e.title }} ({{ e.year }})</div>
              <span class="rounded bg-emerald-500/15 px-1.5 py-0.5 text-xs text-emerald-300">{{ e.license }}</span>
            </div>
            <p class="mt-1 line-clamp-2 text-xs text-slate-400">{{ e.description }}</p>
            <div class="mt-1 text-xs text-sky-400">{{ e.source_name }} ↗</div>
          </div>
        </div>
        <p class="mt-2 text-xs text-slate-500">{{ results.source_note }}</p>
      </section>
    </div>
  </div>
</template>
