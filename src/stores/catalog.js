import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";

export const catalogStore = reactive({
  status: null, // { has_api_key, db: { movies, tv_shows, collections, last_sync }, phase, provider_note }
  sections: [], // home discovery sections
  loadingSections: false,
  sectionsError: null,
});

export async function loadCatalogStatus() {
  try {
    catalogStore.status = await invoke("catalog_status");
  } catch (err) {
    console.warn("catalog status failed", err);
    catalogStore.status = null;
  }
  return catalogStore.status;
}

export async function loadHomeSections(perSection = 20) {
  catalogStore.loadingSections = true;
  catalogStore.sectionsError = null;
  try {
    catalogStore.sections = await invoke("catalog_home", { perSection });
  } catch (err) {
    catalogStore.sectionsError = String(err?.message ?? err);
    catalogStore.sections = [];
  } finally {
    catalogStore.loadingSections = false;
  }
}

export async function startSync() {
  const phase = await invoke("catalog_sync_start");
  catalogStore.status = await loadCatalogStatus();
  return phase;
}

export async function cancelSync() {
  await invoke("catalog_sync_cancel");
}

export async function clearCatalog() {
  const removed = await invoke("catalog_clear");
  await loadCatalogStatus();
  catalogStore.sections = [];
  return removed;
}

export async function setSyncPages(pages) {
  await invoke("catalog_set_pages", { pages });
}

export function isSyncRunning() {
  return catalogStore.status?.phase?.phase === "Running";
}

/** Media key convention for TMDB items (favourites/library/history). */
export function tmdbMediaKey(mediaType, tmdbId) {
  return `tmdb:${mediaType}:${tmdbId}`;
}

/** Shared favourites set for card hearts and details pages. */
export const favoritesState = reactive({ set: new Set(), loaded: false });

export async function loadFavorites() {
  try {
    const rows = await invoke("favorites_list");
    favoritesState.set = new Set((rows ?? []).map((r) => r.media_key));
    favoritesState.loaded = true;
  } catch (err) {
    console.warn("favorites load failed", err);
  }
}

export function isFavorite(mediaKey) {
  return favoritesState.set.has(mediaKey);
}

export async function toggleFavorite(mediaKey, title) {
  if (favoritesState.set.has(mediaKey)) {
    await invoke("favorites_remove", { mediaKey });
    favoritesState.set.delete(mediaKey);
    return false;
  }
  await invoke("favorites_add", { mediaKey, title });
  favoritesState.set.add(mediaKey);
  return true;
}

export async function toggleLibrary(mediaKey, title, mediaType) {
  const rows = await invoke("library_list");
  const exists = (rows ?? []).some((r) => r.media_key === mediaKey);
  if (exists) {
    await invoke("library_remove", { mediaKey });
    return false;
  }
  await invoke("library_add", { mediaKey, title, path: null, mediaType });
  return true;
}

export async function isInLibrary(mediaKey) {
  try {
    const rows = await invoke("library_list");
    return (rows ?? []).some((r) => r.media_key === mediaKey);
  } catch {
    return false;
  }
}

/**
 * Builds the local proxy URL for a TMDB poster/backdrop/still path.
 * The frontend never touches image.tmdb.org (CSP + privacy).
 */
export function imageUrl(path, kind = "poster", size = "w342") {
  if (!path) return null;
  return invoke("catalog_image_url", { path, kind, size });
}

/** Poster size per layout density. */
export const SIZES = {
  thumb: "w154",
  card: "w342",
  large: "w500",
  backdrop: "w780",
  still: "w300",
  profile: "w185",
};
