<script setup>
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import AppState from "../components/common/AppState.vue";

const entries = ref([]);
const sourceNote = ref("");
const query = ref("");
const genre = ref("all");
const loading = ref(true);
const error = ref(null);

const GENRES = ["all", "Animation", "Comedy", "Family", "Fantasy", "Adventure", "Drama", "Sci-Fi", "Action"];

const GRADIENTS = [
  "from-sky-900 via-slate-900 to-slate-800",
  "from-orange-900 via-slate-900 to-slate-800",
  "from-violet-900 via-slate-900 to-slate-800",
  "from-emerald-900 via-slate-900 to-slate-800",
];

function gradient(id) {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return GRADIENTS[h % GRADIENTS.length];
}

async function load() {
  loading.value = true;
  error.value = null;
  try {
    const res = await invoke("catalog_list", {
      query: query.value || null,
      genre: genre.value === "all" ? null : genre.value,
    });
    entries.value = res.entries ?? [];
    sourceNote.value = res.source_note ?? "";
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    loading.value = false;
  }
}

onMounted(load);
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4 flex items-end justify-between">
      <div>
        <h1 class="text-2xl font-bold text-slate-100">Discover</h1>
        <p class="mt-1 text-sm text-slate-400">{{ sourceNote }}</p>
      </div>
    </header>

    <div class="mb-4 flex flex-wrap items-center gap-2">
      <input
        v-model="query"
        placeholder="Search titles…"
        class="w-64 rounded-md border border-slate-700 bg-slate-800/60 px-3 py-1.5 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
        @input="load"
      />
      <button
        v-for="g in GENRES"
        :key="g"
        class="rounded-md px-2.5 py-1 text-xs font-medium capitalize"
        :class="genre === g ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'"
        @click="genre = g; load()"
      >
        {{ g }}
      </button>
    </div>

    <p v-if="error" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ error }}</p>

    <div v-if="loading" class="flex flex-1 items-center justify-center">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-sky-400"></div>
    </div>

    <div v-else-if="entries.length" class="flex-1 overflow-y-auto">
      <div class="grid grid-cols-2 gap-4 pb-4 lg:grid-cols-3">
        <div
          v-for="e in entries"
          :key="e.id"
          class="overflow-hidden rounded-lg border border-slate-700/60 bg-slate-800/40"
        >
          <div
            class="flex h-32 items-end bg-gradient-to-br p-3"
            :class="gradient(e.id)"
          >
            <div class="drop-shadow">
              <div class="text-sm font-bold text-white">{{ e.title }}</div>
              <div class="text-xs text-slate-300">{{ e.year }} · {{ e.duration_minutes }} min</div>
            </div>
          </div>
          <div class="p-3">
            <p class="line-clamp-2 text-xs leading-4 text-slate-300">{{ e.description }}</p>
            <div class="mt-2 flex flex-wrap gap-1">
              <span v-for="g in e.genres" :key="g" class="rounded bg-slate-700/60 px-1.5 py-0.5 text-xs text-slate-300">{{ g }}</span>
            </div>
            <div class="mt-2 flex items-center justify-between text-xs">
              <span class="rounded bg-emerald-500/15 px-1.5 py-0.5 text-emerald-300">{{ e.license }}</span>
              <a :href="e.source_url" class="text-sky-400 hover:text-sky-300" target="_blank" rel="noopener">{{ e.source_name }}</a>
            </div>
          </div>
        </div>
      </div>
      <p class="pb-4 text-xs text-slate-500">
        Torrent acquisition for catalog entries is not bundled: entries link to the official project pages where legal downloads are published. A source provider contract lets authorized providers attach torrents later.
      </p>
    </div>

    <AppState
      v-else
      state="empty"
      title="No catalog entries match"
      message="Try a different search or genre filter."
    />
  </div>
</template>
