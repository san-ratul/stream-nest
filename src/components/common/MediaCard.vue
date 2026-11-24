<script setup>
import { onMounted, ref, watch } from "vue";
import { imageUrl, SIZES, tmdbMediaKey, isFavorite, toggleFavorite } from "../../stores/catalog";

const props = defineProps({
  card: { type: Object, required: true }, // { media_type, tmdb_id, title, year, poster_path, rating, overview }
  showType: { type: Boolean, default: true },
});
const emit = defineEmits(["open"]);

const posterUrl = ref(null);
const failed = ref(false);
const fav = ref(false);
let mediaKey = tmdbMediaKey(props.card.media_type, props.card.tmdb_id);

async function loadPoster(retry = true) {
  posterUrl.value = null;
  failed.value = false;
  if (!props.card.poster_path) {
    failed.value = true;
    return;
  }
  try {
    const url = await imageUrl(props.card.poster_path, "poster", SIZES.card);
    if (url) {
      posterUrl.value = url;
    } else if (retry) {
      // Image proxy may still be starting right after app boot; retry once.
      setTimeout(() => loadPoster(false), 1500);
    } else {
      failed.value = true;
    }
  } catch {
    failed.value = true;
  }
}

function syncFav() {
  fav.value = isFavorite(mediaKey);
}

onMounted(() => {
  loadPoster();
  syncFav();
});
watch(() => props.card.tmdb_id, () => {
  mediaKey = tmdbMediaKey(props.card.media_type, props.card.tmdb_id);
  loadPoster();
  syncFav();
});

async function onFavToggle(e) {
  e.stopPropagation();
  try {
    await toggleFavorite(mediaKey, props.card.title);
    syncFav();
  } catch (err) {
    console.warn("favorite toggle failed", err);
  }
}

const GRADIENTS = [
  "from-sky-900 via-slate-900 to-slate-800",
  "from-orange-900 via-slate-900 to-slate-800",
  "from-violet-900 via-slate-900 to-slate-800",
  "from-emerald-900 via-slate-900 to-slate-800",
];
function gradient() {
  let h = 0;
  for (const c of String(props.card.tmdb_id)) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return GRADIENTS[h % GRADIENTS.length];
}
</script>

<template>
  <div
    class="group cursor-pointer overflow-hidden rounded-lg border border-slate-700/60 bg-slate-800/40 transition hover:border-sky-500/50 hover:shadow-lg hover:shadow-sky-500/10"
    @click="emit('open', card)"
  >
    <div class="relative aspect-[2/3] w-full overflow-hidden">
      <img
        v-if="posterUrl && !failed"
        :src="posterUrl"
        :alt="card.title"
        loading="lazy"
        class="h-full w-full object-cover transition group-hover:scale-[1.03]"
        @error="failed = true"
      />
      <div
        v-else
        class="flex h-full w-full items-center justify-center bg-gradient-to-br"
        :class="gradient()"
      >
        <span class="px-2 text-center text-lg font-black text-white/70 drop-shadow">{{ card.title }}</span>
      </div>

      <button
        class="absolute right-1.5 top-1.5 flex h-7 w-7 items-center justify-center rounded-full border border-white/10 bg-black/50 text-sm backdrop-blur transition hover:bg-black/70"
        :class="fav ? 'text-amber-300' : 'text-slate-300'"
        :title="fav ? 'Remove from favorites' : 'Add to favorites'"
        @click="onFavToggle"
      >
        {{ fav ? "★" : "☆" }}
      </button>

      <span
        v-if="card.rating != null"
        class="absolute bottom-1.5 left-1.5 rounded bg-black/60 px-1.5 py-0.5 text-xs font-semibold text-amber-300 backdrop-blur"
      >
        ★ {{ Number(card.rating).toFixed(1) }}
      </span>
      <span
        v-if="showType"
        class="absolute bottom-1.5 right-1.5 rounded bg-black/60 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-sky-300 backdrop-blur"
      >
        {{ card.media_type === "tv" ? "TV" : "Movie" }}
      </span>
    </div>

    <div class="p-2">
      <div class="truncate text-sm font-medium text-slate-100" :title="card.title">{{ card.title }}</div>
      <div class="mt-0.5 text-xs text-slate-400">{{ card.year || "—" }}</div>
    </div>
  </div>
</template>
