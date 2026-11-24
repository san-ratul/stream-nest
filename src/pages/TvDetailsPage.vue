<script setup>
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRoute, useRouter } from "vue-router";
import AppState from "../components/common/AppState.vue";
import SourcePanel from "../components/streaming/SourcePanel.vue";
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
import {
  playbackStore,
  startAuthorizedPlayback,
  listenPlaybackStages,
} from "../composables/playback";

const route = useRoute();
const router = useRouter();
const showId = Number(route.params.id);

const detail = ref(null);
const sources = ref([]);
const state = ref("loading");
const error = ref(null);
const posterUrl = ref(null);
const backdropUrl = ref(null);
const inLibrary = ref(false);
const expandedSeason = ref(null);
const episodes = ref({}); // season_number -> EpisodeRow[]
const loadingSeason = ref(null);
const historyRows = ref([]);
const seasonPosters = ref({});

// Playback flow state
const busySource = ref(null);
const playbackError = ref(null);
const episodeChooser = ref(null); // { matches, videos, season, episode, prepared, source }
const chooserVideos = ref(null);
let chooserResolve = null;

function fileChooser(videos) {
  return new Promise((resolve) => {
    chooserVideos.value = videos;
    chooserResolve = resolve;
  });
}

function resolveChooser(fileIndex) {
  chooserVideos.value = null;
  chooserResolve?.(fileIndex);
  chooserResolve = null;
}

function cancelChooser() {
  chooserVideos.value = null;
  chooserResolve?.(null);
  chooserResolve = null;
}

function episodeKey(seasonNumber, episodeNumber) {
  return `tmdb:tv:${showId}:${seasonNumber}:${episodeNumber}`;
}

function historyFor(seasonNumber, episodeNumber) {
  const key = episodeKey(seasonNumber, episodeNumber);
  return historyRows.value.find((r) => r.media_key === key) ?? null;
}

function isWatched(seasonNumber, episodeNumber) {
  const row = historyFor(seasonNumber, episodeNumber);
  return Boolean(row && row.duration_seconds > 0 && row.position_seconds >= row.duration_seconds * 0.95);
}

function isNextEpisode(seasonNumber, episodeNumber) {
  // next after the most recent watched/in-progress episode in this season listing
  const current = episodeChooser.value?.season === seasonNumber && episodeChooser.value?.episode === episodeNumber;
  return current === true;
}

async function onEpisodePlay(source, seasonNumber, episodeNumber, episodeName) {
  busySource.value = source.provider_id;
  playbackError.value = null;
  try {
    const result = await startAuthorizedPlayback({
      source,
      tmdbKey: episodeKey(seasonNumber, episodeNumber),
      title: `${detail.value.card.title} S${String(seasonNumber).padStart(2, "0")}E${String(episodeNumber).padStart(2, "0")}${episodeName ? ` · ${episodeName}` : ""}`,
      season: seasonNumber,
      episode: episodeNumber,
      onNeedFileChoice: fileChooser,
    });
    if (result?.started) {
      router.push("/player");
    }
  } catch (err) {
    playbackError.value = String(err?.message ?? err);
  } finally {
    busySource.value = null;
    playbackStore.stage = null;
  }
}

const mediaKey = tmdbMediaKey("tv", showId);

async function load() {
  state.value = "loading";
  error.value = null;
  try {
    const res = await invoke("tv_details", { tmdbId: showId });
    detail.value = res.detail;
    sources.value = res.sources ?? [];
    state.value = "ready";
    await loadFavorites();
    inLibrary.value = await isInLibrary(mediaKey);
    try {
      const rows = await invoke("history_list", { limit: 200 });
      historyRows.value = (rows ?? []).filter((r) => r.media_key.startsWith(`${mediaKey}:`));
    } catch {
      historyRows.value = [];
    }
    posterUrl.value = detail.value.card.poster_path
      ? await imageUrl(detail.value.card.poster_path, "poster", SIZES.large)
      : null;
    backdropUrl.value = detail.value.backdrop_path
      ? await imageUrl(detail.value.backdrop_path, "backdrop", SIZES.backdrop)
      : null;
    for (const s of detail.value.seasons) {
      if (s.poster_path) {
        seasonPosters.value[s.season_number] = await imageUrl(s.poster_path, "poster", SIZES.thumb);
      }
    }
    if (detail.value.seasons.length) {
      await toggleSeason(detail.value.seasons[0].season_number);
    }
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



const playableSources = computed(() => sources.value.filter((s) => s.playable_in_app));
const pageSources = computed(() => sources.value.filter((s) => !s.playable_in_app));

async function toggleSeason(seasonNumber) {
  if (expandedSeason.value === seasonNumber) {
    expandedSeason.value = null;
    return;
  }
  expandedSeason.value = seasonNumber;
  if (!episodes.value[seasonNumber]) {
    loadingSeason.value = seasonNumber;
    try {
      episodes.value = {
        ...episodes.value,
        [seasonNumber]: await invoke("season_episodes", { tmdbId: showId, seasonNumber }),
      };
    } catch (err) {
      console.warn("season episodes failed", err);
      episodes.value = { ...episodes.value, [seasonNumber]: [] };
    } finally {
      loadingSeason.value = null;
    }
  }
}

function stillUrl(path) {
  return imageUrl(path, "still", SIZES.still);
}

function episodeProgress(seasonNumber, episodeNumber) {
  const key = `${mediaKey}:${seasonNumber}:${episodeNumber}`;
  return historyRows.value.find((r) => r.media_key === key) ?? null;
}

function watchedBadge(seasonNumber, episodeNumber) {
  const row = episodeProgress(seasonNumber, episodeNumber);
  if (!row) return null;
  if (row.duration_seconds > 0 && row.position_seconds >= row.duration_seconds * 0.95) {
    return "watched";
  }
  if (row.position_seconds > 10) return "in-progress";
  return null;
}

async function onFav() {
  await toggleFavorite(mediaKey, detail.value.card.title);
  await loadFavorites();
}

async function onLibrary() {
  inLibrary.value = await toggleLibrary(mediaKey, detail.value.card.title, "tmdb_tv");
}

const episodePlayTarget = ref(null);

function openEpisodeSource(seasonNumber, episodeNumber, name) {
  episodePlayTarget.value = { season: seasonNumber, episode: episodeNumber, name };
}

onMounted(() => {
  load();
  listenPlaybackStages();
});
</script>

<template>
  <div class="h-full overflow-y-auto">
    <AppState v-if="state === 'loading'" state="loading" title="Loading show…" />
    <AppState v-else-if="state === 'error'" state="error" title="Failed to load show details" :message="error">
      <button class="rounded-md bg-sky-500/20 px-4 py-2 text-sm text-sky-300 hover:bg-sky-500/30" @click="load">
        Retry
      </button>
    </AppState>
    <AppState
      v-else-if="state === 'notfound'"
      state="empty"
      title="Not in the local catalog"
      message="This show was not synchronized yet. Run a catalog sync from Settings, then search for it."
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
                <img v-if="posterUrl" :src="posterUrl" :alt="detail.card.title" class="w-full object-cover" @error="posterUrl = null" />
                <div v-else class="flex aspect-[2/3] items-center justify-center bg-slate-800 px-2 text-center text-slate-500">
                  {{ detail.card.title }}
                </div>
              </div>
            </div>

            <div class="min-w-0 flex-1">
              <h1 class="text-3xl font-bold text-slate-100">{{ detail.card.title }}</h1>
              <p v-if="detail.original_name && detail.original_name !== detail.card.title" class="mt-1 text-sm text-slate-400">
                Original name: {{ detail.original_name }}
              </p>

              <div class="mt-3 flex flex-wrap items-center gap-2 text-sm">
                <span v-if="detail.card.year" class="rounded bg-slate-800 px-2 py-0.5 text-slate-300">{{ detail.card.year }}</span>
                <span v-if="detail.number_of_seasons" class="rounded bg-slate-800 px-2 py-0.5 text-slate-300">
                  {{ detail.number_of_seasons }} season{{ detail.number_of_seasons === 1 ? "" : "s" }}
                </span>
                <span v-if="detail.card.rating != null" class="rounded bg-amber-500/15 px-2 py-0.5 font-semibold text-amber-300">
                  ★ {{ Number(detail.card.rating).toFixed(1) }}
                  <span class="font-normal text-amber-200/60">({{ detail.vote_count ?? 0 }})</span>
                </span>
                <span class="rounded bg-slate-800 px-2 py-0.5 text-xs uppercase tracking-wide text-violet-300">TV</span>
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

              <p class="mt-3 max-w-3xl text-sm leading-6 text-slate-300">{{ detail.card.overview || "No overview available." }}</p>

              <div v-if="playableSources.length" class="mt-5">
                <div v-if="playbackError" class="mb-2 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
                  {{ playbackError }}
                  <button class="ml-2 underline hover:text-red-200" @click="playbackError = null">dismiss</button>
                </div>
                <SourcePanel
                  v-if="episodePlayTarget"
                  :sources="sources"
                  :busy-source="busySource"
                  :stage="playbackStore.stage"
                  @play="(s) => onEpisodePlay(s, episodePlayTarget.season, episodePlayTarget.episode, episodePlayTarget.name)"
                />
                <p v-else class="text-xs text-slate-500">
                  Authorized sources available — open an episode below and pick a source to stream it.
                </p>
              </div>
              <div v-else class="mt-5 max-w-2xl rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
                <div class="text-sm font-semibold text-slate-200">No authorized in-app source</div>
                <p class="mt-1 text-xs leading-5 text-slate-400">
                  Episode playback activates only when an authorized, rights-verified source exists.
                  TMDB metadata never implies distribution rights.
                </p>
                <div v-if="pageSources.length" class="mt-3 flex flex-col gap-2">
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
                  </div>
                </div>
              </div>
            </div>
          </div>

          <section v-if="detail.seasons.length" class="mt-8">
            <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Seasons</h2>
            <div class="flex max-w-3xl flex-col gap-2">
              <div
                v-for="s in detail.seasons"
                :key="s.season_number"
                class="overflow-hidden rounded-lg border border-slate-700/60 bg-slate-800/40"
              >
                <button
                  class="flex w-full items-center gap-3 p-3 text-left hover:bg-slate-800/70"
                  @click="toggleSeason(s.season_number)"
                >
                  <img
                    v-if="seasonPosters[s.season_number]"
                    :src="seasonPosters[s.season_number]"
                    :alt="s.name"
                    loading="lazy"
                    class="h-16 w-12 shrink-0 rounded object-cover"
                    @error="$event.target.style.display = 'none'"
                  />
                  <div v-else class="flex h-16 w-12 shrink-0 items-center justify-center rounded bg-slate-700/50 text-slate-400">
                    {{ s.season_number }}
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-sm font-medium text-slate-100">{{ s.name || `Season ${s.season_number}` }}</div>
                    <div class="text-xs text-slate-500">
                      {{ s.air_date || "—" }} · {{ s.episode_count ?? "?" }} episodes
                    </div>
                  </div>
                  <span class="text-slate-400">{{ expandedSeason === s.season_number ? "▲" : "▼" }}</span>
                </button>

                <div v-if="expandedSeason === s.season_number" class="border-t border-slate-700/60 p-3">
                  <div v-if="loadingSeason === s.season_number" class="py-4 text-center text-xs text-slate-500">
                    Loading episodes…
                  </div>
                  <div v-else-if="!episodes[s.season_number]?.length" class="py-4 text-center text-xs text-slate-500">
                    No episode data synchronized for this season yet.
                  </div>
                  <div v-else class="flex flex-col gap-2">
                    <div
                      v-for="ep in episodes[s.season_number]"
                      :key="ep.episode_number"
                      class="flex gap-3 rounded-lg border border-slate-700/40 bg-slate-900/40 p-2"
                    >
                      <div class="h-16 w-28 shrink-0 overflow-hidden rounded bg-slate-800">
                        <img
                          v-if="ep.still_path"
                          :src="stillUrl(ep.still_path)"
                          :alt="ep.name"
                          loading="lazy"
                          class="h-full w-full object-cover"
                          @error="$event.target.style.display = 'none'"
                        />
                        <div v-else class="flex h-full items-center justify-center text-xs text-slate-600">
                          E{{ ep.episode_number }}
                        </div>
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-2">
                          <span class="text-sm font-medium text-slate-100">
                            E{{ ep.episode_number }} · {{ ep.name || "Untitled" }}
                          </span>
                          <span v-if="watchedBadge(s.season_number, ep.episode_number) === 'watched'"
                            class="rounded bg-emerald-500/15 px-1.5 text-[10px] font-bold uppercase text-emerald-300">Watched</span>
                          <span v-else-if="watchedBadge(s.season_number, ep.episode_number) === 'in-progress'"
                            class="rounded bg-sky-500/15 px-1.5 text-[10px] font-bold uppercase text-sky-300">In progress</span>
                          <span v-if="ep.vote_average != null" class="text-xs text-amber-300/80">★ {{ Number(ep.vote_average).toFixed(1) }}</span>
                          <button
                            v-if="playableSources.length"
                            class="ml-auto shrink-0 rounded bg-emerald-500/20 px-2 py-0.5 text-[11px] font-medium text-emerald-300 hover:bg-emerald-500/30"
                            :disabled="busySource != null"
                            @click="openEpisodeSource(s.season_number, ep.episode_number, ep.name)"
                          >
                            ▶ Play
                          </button>
                        </div>
                        <div class="text-xs text-slate-500">{{ ep.air_date || "No air date" }}</div>
                        <p class="mt-1 line-clamp-2 text-xs leading-4 text-slate-400">{{ ep.overview || "No overview." }}</p>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </section>

          <section v-if="detail.cast?.length" class="mt-8">
            <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Cast</h2>
            <div class="flex flex-wrap gap-2 pb-4">
              <span
                v-for="c in detail.cast.slice(0, 20)"
                :key="c.id"
                class="rounded-full border border-slate-700/60 bg-slate-800/40 px-3 py-1 text-xs text-slate-300"
                :title="c.character"
              >
                {{ c.name }}<span v-if="c.character" class="text-slate-500"> — {{ c.character }}</span>
              </span>
            </div>
          </section>
        </div>
      </div>
    </template>
  </div>
</template>
