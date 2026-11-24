<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "vue-router";
import AppState from "../components/common/AppState.vue";

const router = useRouter();
const favorites = ref([]);
const loading = ref(true);
const error = ref(null);

async function load() {
  loading.value = true;
  error.value = null;
  try {
    favorites.value = (await invoke("favorites_list")) ?? [];
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    loading.value = false;
  }
}

onMounted(load);

async function removeFav(mediaKey) {
  await invoke("favorites_remove", { mediaKey });
  await load();
}

function tmdbRoute(mediaKey) {
  // tmdb:movie:123 / tmdb:tv:45
  const parts = mediaKey.split(":");
  if (parts.length === 3 && parts[0] === "tmdb") {
    return parts[1] === "tv" ? `/tv/${parts[2]}` : `/movie/${parts[2]}`;
  }
  return null;
}

function openFav(row) {
  const target = tmdbRoute(row.media_key);
  if (target) router.push(target);
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Favorites</h1>
      <p class="mt-1 text-sm text-slate-400">Media you marked as favorite from catalog pages or torrent details.</p>
    </header>

    <p v-if="error" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ error }}</p>

    <div v-if="loading" class="flex flex-1 items-center justify-center">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-sky-400"></div>
    </div>

    <div v-else-if="favorites.length" class="flex flex-col gap-2 overflow-y-auto">
      <div
        v-for="row in favorites"
        :key="row.id"
        class="flex items-center gap-3 rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3 text-sm"
      >
        <span class="text-amber-300">★</span>
        <span class="min-w-0 flex-1 truncate text-slate-100">{{ row.title }}</span>
        <button
          v-if="tmdbRoute(row.media_key)"
          class="rounded bg-sky-500/20 px-2 py-0.5 text-xs text-sky-300 hover:bg-sky-500/30"
          @click="openFav(row)"
        >
          open
        </button>
        <button
          class="rounded bg-slate-800 px-2 py-0.5 text-xs text-slate-400 hover:bg-slate-700"
          @click="removeFav(row.media_key)"
        >
          remove
        </button>
      </div>
    </div>

    <AppState
      v-else
      state="empty"
      title="No favorites yet"
      message="Use the ☆ button on a movie or TV details page to save it here."
    />
  </div>
</template>
