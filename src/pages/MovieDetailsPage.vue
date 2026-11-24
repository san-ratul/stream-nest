<script setup>
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRoute, useRouter } from "vue-router";
import AppState from "../components/common/AppState.vue";
import {
  imageUrl,
  SIZES,
  loadFavorites,
  isFavorite,
  toggleFavorite,
  tmdbMediaKey,
  toggleLibrary,
  isInLibrary,
} from "../stores/catalog";

const route = useRoute();
const router = useRouter();
const movieId = Number(route.params.id);

const detail = ref(null);
const sources = ref([]);
const state = ref("loading"); // loading | ready | error | notfound
const error = ref(null);
const posterUrl = ref(null);
const backdropUrl = ref(null);
const profileUrls = ref({});
const inLibrary = ref(false);
const historyRow = ref(null);

const mediaKey = tmdbMediaKey("movie", movieId);

async function load() {
  state.value = "loading";
  error.value = null;
  try {
    const res = await invoke("movie_details", { tmdbId: movieId });
    detail.value = res.detail;
    sources.value = res.sources ?? [];
    state.value = "ready";
    await loadFavorites();
    inLibrary.value = await isInLibrary(mediaKey);
    try {
      const rows = await invoke("history_list", { limit: 200 });
      historyRow.value = (rows ?? []).find((r) => r.media_key === mediaKey) ?? null;
    } catch {
      historyRow.value = null;
    }
    posterUrl.value = detail.value.card.poster_path
      ? await imageUrl(detail.value.card.poster_path, "poster", SIZES.large)
      : null;
    backdropUrl.value = detail.value.backdrop_path
      ? await imageUrl(detail.value.backdrop_path, "backdrop", SIZES.backdrop)
      : null;
    const profiles = {};
    for (const c of (detail.value.cast ?? []).slice(0, 12)) {
      if (c.profile_path) {
        profiles[c.id] = await imageUrl(c.profile_path, "profile", SIZES.profile);
      }
    }
    profileUrls.value = profiles;
  } catch (err) {
    const msg = String(err?.message ?? err);
    if (msg.includes("not in the local catalog")) {
      state.value = "notfound";
    } else {
      error.value = msg;
      state.value = "error";
    }
  }
}

onMounted(load);

const playableSources = computed(() => sources.value.filter((s) => s.playable_in_app));
const pageSources = computed(() => sources.value.filter((s) => !s.playable_in_app));
const runtime = computed(() => {
  const m = detail.value?.runtime_minutes;
  if (!m) return null;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
});
const director = computed(
  () => detail.value?.crew?.find((c) => c.job === "Director")?.name ?? null
);

async function onFav() {
  await toggleFavorite(mediaKey, detail.value.card.title);
  await loadFavorites();
}

async function onLibrary() {
  inLibrary.value = await toggleLibrary(mediaKey, detail.value.card.title, "tmdb_movie");
}

function fmtClock(s) {
  const m = Math.floor(s / 60);
  const ss = Math.floor(s % 60);
  return `${m}:${String(ss).padStart(2, "0")}`;
}
</script>

<template>
  <div class="h-full overflow-y-auto">
    <AppState v-if="state === 'loading'" state="loading" title="Loading movie…" />
    <AppState
      v-else-if="state === 'error'"
      state="error"
      title="Failed to load movie details"
      :message="error"
    >
      <button class="rounded-md bg-sky-500/20 px-4 py-2 text-sm text-sky-300 hover:bg-sky-500/30" @click="load">
        Retry
      </button>
    </AppState>
    <AppState
      v-else-if="state === 'notfound'"
      state="empty"
      title="Not in the local catalog"
      message="This title was not synchronized yet. Run a catalog sync from Settings, then search for it."
    >
      <button class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600" @click="router.back()">
        Go back
      </button>
    </AppState>

    <template v-else>
      <div class="relative">
        <div class="absolute inset-0 h-64 overflow-hidden">
          <img v-if="backdropUrl" :src="backdropUrl" alt="" class="h-full w-full object-cover opacity-30" />
          <div class="absolute inset-0 bg-gradient-to-b from-transparent to-[#0b0f17]"></div>
        </div>

        <div class="relative p-6 pt-10">
          <button
            class="mb-4 rounded-md border border-slate-600 bg-slate-900/70 px-3 py-1.5 text-sm text-slate-300 backdrop-blur hover:bg-slate-800"
            @click="router.back()"
          >
            ← Back
          </button>

          <div class="flex flex-col gap-6 lg:flex-row">
            <div class="w-44 shrink-0">
              <div class="overflow-hidden rounded-lg border border-slate-700/60 shadow-xl">
                <img
                  v-if="posterUrl"
                  :src="posterUrl"
                  :alt="detail.card.title"
                  class="w-full object-cover"
                  @error="posterUrl = null"
                />
                <div v-else class="flex aspect-[2/3] items-center justify-center bg-slate-800 px-2 text-center text-slate-500">
                  {{ detail.card.title }}
                </div>
              </div>
            </div>

            <div class="min-w-0 flex-1">
              <h1 class="text-3xl font-bold text-slate-100">{{ detail.card.title }}</h1>
              <p v-if="detail.original_title && detail.original_title !== detail.card.title" class="mt-1 text-sm text-slate-400">
                Original title: {{ detail.original_title }}
              </p>

              <div class="mt-3 flex flex-wrap items-center gap-2 text-sm">
                <span v-if="detail.card.year" class="rounded bg-slate-800 px-2 py-0.5 text-slate-300">{{ detail.card.year }}</span>
                <span v-if="runtime" class="rounded bg-slate-800 px-2 py-0.5 text-slate-300">{{ runtime }}</span>
                <span v-if="detail.card.rating != null" class="rounded bg-amber-500/15 px-2 py-0.5 font-semibold text-amber-300">
                  ★ {{ Number(detail.card.rating).toFixed(1) }}
                  <span class="font-normal text-amber-200/60">({{ detail.vote_count ?? 0 }})</span>
                </span>
                <span v-if="detail.original_language" class="rounded bg-slate-800 px-2 py-0.5 uppercase text-slate-400">{{ detail.original_language }}</span>
                <span class="rounded bg-slate-800 px-2 py-0.5 text-xs uppercase tracking-wide text-sky-300">Movie</span>
              </div>

              <div v-if="detail.genres.length" class="mt-3 flex flex-wrap gap-1">
                <span v-for="g in detail.genres" :key="g" class="rounded bg-slate-800/80 px-2 py-0.5 text-xs text-slate-300">{{ g }}</span>
              </div>

              <div class="mt-4 flex flex-wrap items-center gap-2">
                <button
                  class="rounded-md px-3 py-2 text-sm font-medium"
                  :class="isFavorite(mediaKey) ? 'bg-amber-500/20 text-amber-300 hover:bg-amber-500/30' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'"
                  @click="onFav"
                >
                  {{ isFavorite(mediaKey) ? "★ In favorites" : "☆ Add to favorites" }}
                </button>
                <button
                  class="rounded-md px-3 py-2 text-sm font-medium"
                  :class="inLibrary ? 'bg-sky-500/20 text-sky-300 hover:bg-sky-500/30' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'"
                  @click="onLibrary"
                >
                  {{ inLibrary ? "✓ In my library" : "+ My library" }}
                </button>
              </div>

              <div v-if="detail.release_date" class="mt-4 text-sm text-slate-400">Release date: {{ detail.release_date }}</div>

              <p class="mt-3 max-w-3xl text-sm leading-6 text-slate-300">{{ detail.card.overview || "No overview available." }}</p>

              <div v-if="director" class="mt-4 text-sm text-slate-400">
                Director: <span class="text-slate-200">{{ director }}</span>
              </div>

              <div v-if="playableSources.length" class="mt-5">
                <button
                  v-for="s in playableSources"
                  :key="s.provider_id"
                  class="rounded-md bg-emerald-500/90 px-5 py-2 text-sm font-semibold text-slate-950 hover:bg-emerald-400"
                >
                  ▶ Play ({{ s.source_name }})
                </button>
              </div>
              <div v-else class="mt-5 max-w-2xl rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
                <div class="text-sm font-semibold text-slate-200">No authorized in-app source</div>
                <p class="mt-1 text-xs leading-5 text-slate-400">
                  TMDB provides metadata only — it does not grant streaming or distribution rights.
                  StreamNest never attaches unauthorized torrent sources to catalog titles.
                </p>
                <div v-if="pageSources.length" class="mt-3 flex flex-col gap-2">
                  <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">Verified official sources</div>
                  <div v-for="s in pageSources" :key="s.provider_id" class="rounded-md border border-slate-700/60 bg-slate-900/50 p-3">
                    <div class="flex items-center justify-between gap-2">
                      <div>
                        <div class="text-sm text-sky-300">{{ s.source_name }}</div>
                        <div class="text-xs text-slate-500">License: {{ s.license }}</div>
                      </div>
                      <a
                        :href="s.source_url"
                        target="_blank"
                        rel="noopener"
                        class="rounded bg-sky-500/20 px-3 py-1.5 text-xs font-medium text-sky-300 hover:bg-sky-500/30"
                      >
                        Open official page ↗
                      </a>
                    </div>
                    <p v-if="s.attribution" class="mt-2 text-[11px] text-slate-500">{{ s.attribution }}</p>
                  </div>
                </div>
              </div>

              <div v-if="historyRow" class="mt-5 rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
                <div class="text-sm font-semibold text-slate-200">Playback history</div>
                <div class="mt-2 h-1.5 w-64 overflow-hidden rounded-full bg-slate-700/70">
                  <div
                    class="h-full rounded-full bg-sky-400"
                    :style="{ width: `${Math.min(100, Math.round((historyRow.position_seconds / Math.max(historyRow.duration_seconds, 1)) * 100))}%` }"
                  ></div>
                </div>
                <div class="mt-2 text-xs text-slate-400">
                  Last played {{ historyRow.last_played_at }} · {{ fmtClock(historyRow.position_seconds) }} /
                  {{ fmtClock(historyRow.duration_seconds) }}
                </div>
              </div>
            </div>
          </div>

          <section v-if="detail.cast?.length" class="mt-8">
            <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Top cast</h2>
            <div class="flex gap-3 overflow-x-auto pb-2">
              <div
                v-for="c in detail.cast.slice(0, 12)"
                :key="c.id"
                class="w-28 shrink-0 rounded-lg border border-slate-700/60 bg-slate-800/40 p-2 text-center"
              >
                <img
                  v-if="profileUrls[c.id]"
                  :src="profileUrls[c.id]"
                  :alt="c.name"
                  loading="lazy"
                  class="mx-auto h-24 w-24 rounded-full object-cover"
                  @error="profileUrls = { ...profileUrls, [c.id]: null }"
                />
                <div
                  v-else
                  class="mx-auto flex h-24 w-24 items-center justify-center rounded-full bg-slate-700/60 text-xl text-slate-400"
                >
                  {{ (c.name ?? "?").slice(0, 1) }}
                </div>
                <div class="mt-2 truncate text-xs font-medium text-slate-200" :title="c.name">{{ c.name }}</div>
                <div class="truncate text-[11px] text-slate-500" :title="c.character">{{ c.character }}</div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </template>
  </div>
</template>
