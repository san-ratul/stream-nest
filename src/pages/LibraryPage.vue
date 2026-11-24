<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "vue-router";
import AppState from "../components/common/AppState.vue";

const router = useRouter();
const library = ref([]);
const history = ref([]);
const loading = ref(true);
const error = ref(null);
const query = ref("");

async function load() {
  loading.value = true;
  error.value = null;
  try {
    library.value = (await invoke("library_list")) ?? [];
    history.value = (await invoke("history_list", { limit: 30 })) ?? [];
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    loading.value = false;
  }
}

onMounted(load);

async function removeLibrary(mediaKey) {
  await invoke("library_remove", { mediaKey });
  await load();
}

async function removeHistory(id) {
  await invoke("history_remove", { id });
  await load();
}

function tmdbRoute(mediaKey) {
  const parts = mediaKey.split(":");
  if (parts.length === 3 && parts[0] === "tmdb") {
    return parts[1] === "tv" ? `/tv/${parts[2]}` : `/movie/${parts[2]}`;
  }
  return null;
}

function openRow(mediaKey) {
  const target = tmdbRoute(mediaKey);
  if (target) router.push(target);
}

function matches(title) {
  if (!query.value) return true;
  return title.toLowerCase().includes(query.value.toLowerCase());
}

function fmtClock(s) {
  const m = Math.floor(s / 60);
  const ss = Math.floor(s % 60);
  return `${m}:${String(ss).padStart(2, "0")}`;
}

function progressOf(row) {
  if (!row.duration_seconds) return 0;
  return Math.min(100, Math.round((row.position_seconds / row.duration_seconds) * 100));
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4 flex items-center justify-between">
      <h1 class="text-2xl font-bold text-slate-100">Media Library</h1>
      <input
        v-model="query"
        placeholder="Search…"
        class="w-56 rounded-md border border-slate-700 bg-slate-800/60 px-3 py-1.5 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
      />
    </header>

    <p v-if="error" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ error }}</p>

    <div v-if="loading" class="flex flex-1 items-center justify-center">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-sky-400"></div>
    </div>

    <div v-else class="flex flex-1 flex-col gap-6 overflow-y-auto">
      <section>
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">Recently played</h2>
        <div v-if="history.filter((r) => matches(r.title)).length" class="overflow-hidden rounded-lg border border-slate-700/60">
          <div
            v-for="row in history.filter((r) => matches(r.title))"
            :key="row.id"
            class="flex items-center gap-3 border-b border-slate-800 px-4 py-2.5 text-sm last:border-b-0"
          >
            <span class="min-w-0 flex-1 truncate text-slate-200">{{ row.title }}</span>
            <span class="w-40 shrink-0">
              <span class="block h-1 w-full overflow-hidden rounded-full bg-slate-700/70">
                <span class="block h-full rounded-full bg-sky-400" :style="{ width: `${progressOf(row)}%` }"></span>
              </span>
            </span>
            <span class="w-24 shrink-0 text-right text-xs text-slate-400">
              {{ fmtClock(row.position_seconds) }} / {{ fmtClock(row.duration_seconds) }}
            </span>
            <span class="w-36 shrink-0 text-right text-xs text-slate-500">{{ row.last_played_at }}</span>
            <button
              v-if="tmdbRoute(row.media_key)"
              class="shrink-0 rounded bg-sky-500/20 px-2 py-0.5 text-xs text-sky-300 hover:bg-sky-500/30"
              @click="openRow(row.media_key)"
            >
              open
            </button>
            <button
              class="shrink-0 rounded bg-slate-800 px-2 py-0.5 text-xs text-slate-400 hover:bg-slate-700"
              @click="removeHistory(row.id)"
            >
              remove
            </button>
          </div>
        </div>
        <div v-else class="rounded-lg border border-dashed border-slate-700 px-4 py-5 text-sm text-slate-400">
          No playback history yet.
        </div>
      </section>

      <section>
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">Saved media</h2>
        <div v-if="library.filter((r) => matches(r.title)).length" class="overflow-hidden rounded-lg border border-slate-700/60">
          <div
            v-for="row in library.filter((r) => matches(r.title))"
            :key="row.id"
            class="flex items-center gap-3 border-b border-slate-800 px-4 py-2.5 text-sm last:border-b-0"
          >
            <span class="min-w-0 flex-1 truncate text-slate-200">{{ row.title }}</span>
            <span class="rounded bg-slate-700/60 px-1.5 py-0.5 text-xs text-slate-300">{{ row.media_type }}</span>
            <span class="w-36 shrink-0 text-right text-xs text-slate-500">{{ row.added_at }}</span>
            <button
              v-if="tmdbRoute(row.media_key)"
              class="shrink-0 rounded bg-sky-500/20 px-2 py-0.5 text-xs text-sky-300 hover:bg-sky-500/30"
              @click="openRow(row.media_key)"
            >
              open
            </button>
            <button
              class="shrink-0 rounded bg-slate-800 px-2 py-0.5 text-xs text-slate-400 hover:bg-slate-700"
              @click="removeLibrary(row.media_key)"
            >
              remove
            </button>
          </div>
        </div>
        <div v-else class="rounded-lg border border-dashed border-slate-700 px-4 py-5 text-sm text-slate-400">
          Media you stream is added here automatically.
        </div>
      </section>
    </div>
  </div>
</template>
