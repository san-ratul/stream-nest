<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { settingsStore, resetSettings } from "../stores/settings";
import {
  catalogStore,
  loadCatalogStatus,
  startSync,
  cancelSync,
  clearCatalog,
  setSyncPages,
  isSyncRunning,
} from "../stores/catalog";

function onNumber(field, e) {
  const v = Number(e.target.value);
  settingsStore[field] = Number.isFinite(v) ? v : 0;
}

const clearing = ref(false);
const clearMessage = ref(null);
const syncError = ref(null);
const syncPages = ref(2);
let pollTimer = null;

const phase = computed(() => catalogStore.status?.phase ?? { phase: "Idle" });
const running = computed(() => isSyncRunning());
const db = computed(() => catalogStore.status?.db ?? null);

onMounted(async () => {
  await loadCatalogStatus();
  try {
    const rows = await invoke("settings_get_all");
    const saved = Object.fromEntries(rows ?? []);
    if (saved["catalog.sync.pages"]) syncPages.value = Number(saved["catalog.sync.pages"]) || 2;
  } catch {
    // defaults apply
  }
});

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});

function startPolling() {
  if (pollTimer) clearInterval(pollTimer);
  pollTimer = setInterval(async () => {
    await loadCatalogStatus();
    if (!isSyncRunning()) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }, 1500);
}

async function onSync() {
  syncError.value = null;
  try {
    await startSync();
    startPolling();
  } catch (err) {
    syncError.value = String(err?.message ?? err);
  }
}

async function onCancel() {
  try {
    await cancelSync();
    await loadCatalogStatus();
  } catch (err) {
    syncError.value = String(err?.message ?? err);
  }
}

async function onClear() {
  clearing.value = true;
  clearMessage.value = null;
  try {
    const removed = await clearCatalog();
    clearMessage.value = `Catalog cleared (${removed} records removed). Personal library, history and favorites were kept.`;
  } catch (err) {
    clearMessage.value = `Clear failed: ${String(err?.message ?? err)}`;
  } finally {
    clearing.value = false;
  }
}

async function onPages(e) {
  const v = Math.min(5, Math.max(1, Number(e.target.value) || 2));
  syncPages.value = v;
  try {
    await setSyncPages(v);
  } catch (err) {
    console.warn("pages setting failed", err);
  }
}

function fmtTime(t) {
  return t ?? "never";
}
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Settings</h1>
      <p class="mt-1 text-sm text-slate-400">
        Persisted locally. Engine-side limits (speed caps, connection limit) apply from the engine settings milestone (Phase 8); storage currently uses local device settings.
      </p>
    </header>

    <div class="flex max-w-2xl flex-col gap-6 pb-10">
      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Catalog &amp; TMDB</h2>

        <div class="mb-3 flex items-center gap-2 text-sm">
          <span
            class="h-2 w-2 rounded-full"
            :class="catalogStore.status?.has_api_key ? 'bg-emerald-400' : 'bg-red-400'"
          ></span>
          <span :class="catalogStore.status?.has_api_key ? 'text-slate-300' : 'text-red-300'">
            {{ catalogStore.status?.has_api_key ? "TMDB configured" : "TMDB API key missing" }}
          </span>
          <span class="ml-auto text-xs text-slate-500">
            {{ db ? `${db.movies} movies · ${db.tv_shows} shows` : "catalog unavailable" }}
          </span>
        </div>

        <div v-if="catalogStore.status && !catalogStore.status.has_api_key"
          class="mb-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-200">
          Place a <code class="rounded bg-black/30 px-1">.env</code> file with
          <code class="rounded bg-black/30 px-1">TMDB_API_KEY=your_key</code> in one of:
          <div class="mt-1 flex flex-col gap-0.5">
            <code v-for="p in catalogStore.status.env_paths ?? []" :key="p" class="truncate">{{ p }}</code>
          </div>
          Restart the app afterwards.
        </div>

        <div v-if="db?.last_sync" class="mb-3 rounded-md border border-slate-700/60 bg-slate-900/40 px-3 py-2 text-xs text-slate-400">
          Last sync: <span class="text-slate-200">{{ db.last_sync.status }}</span>
          · started {{ fmtTime(db.last_sync.started_at) }}
          · finished {{ fmtTime(db.last_sync.completed_at) }}
          · {{ db.last_sync.records_processed }} records
          <div v-if="db.last_sync.error_message" class="mt-1 text-red-300">
            Errors: {{ db.last_sync.error_message }}
          </div>
        </div>

        <div v-if="running" class="mb-3 rounded-md border border-sky-500/30 bg-sky-500/10 px-3 py-2 text-xs text-sky-200">
          <div class="flex items-center justify-between">
            <span>
              Syncing: {{ phase.detail?.step || "working" }}
              ({{ (phase.detail?.current ?? 0) + 1 }}/{{ phase.detail?.total ?? "?" }})
            </span>
            <button class="rounded bg-red-500/20 px-2 py-1 text-[11px] text-red-300 hover:bg-red-500/30" @click="onCancel">
              Cancel
            </button>
          </div>
          <div class="mt-2 flex flex-col gap-1">
            <div
              v-for="c in phase.detail?.collections ?? []"
              :key="c.key"
              class="flex items-center justify-between gap-2"
            >
              <span class="truncate" :class="c.status === 'failed' ? 'text-red-300' : 'text-slate-300'">{{ c.title }}</span>
              <span class="text-slate-500">
                {{ c.status === "done" ? `✓ ${c.items}` : c.status === "running" ? "…" : c.status === "failed" ? "✗" : "·" }}
              </span>
            </div>
          </div>
        </div>

        <div v-if="syncError" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
          {{ syncError }}
        </div>

        <div class="flex flex-wrap items-center gap-2">
          <button
            class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400 disabled:opacity-50"
            :disabled="running || !catalogStore.status?.has_api_key"
            @click="onSync"
          >
            {{ running ? "Syncing…" : "Synchronize catalog" }}
          </button>
          <label class="flex items-center gap-2 text-xs text-slate-400">
            Pages per section
            <input
              type="number"
              min="1"
              max="5"
              :value="syncPages"
              class="w-16 rounded-md border border-slate-700 bg-slate-800/60 px-2 py-1 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
              @change="onPages"
            />
            <span class="text-slate-500">(1–5, 20 items/page)</span>
          </label>
          <button
            class="ml-auto rounded-md bg-red-500/15 px-4 py-2 text-sm text-red-300 hover:bg-red-500/25 disabled:opacity-50"
            :disabled="clearing || running"
            @click="onClear"
          >
            {{ clearing ? "Clearing…" : "Clear catalog data" }}
          </button>
        </div>
        <p v-if="clearMessage" class="mt-2 text-xs text-slate-400">{{ clearMessage }}</p>
        <p class="mt-2 text-[11px] leading-4 text-slate-500">
          Clearing removes synchronized TMDB metadata and cached images only — history, favorites,
          library and torrents are preserved. The API key is never displayed or stored in the UI.
        </p>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Storage</h2>
        <div class="grid grid-cols-1 gap-3">
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Download directory</span>
            <input
              v-model="settingsStore.downloadDir"
              placeholder="default: app data /downloads"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            />
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Cache directory</span>
            <input
              v-model="settingsStore.cacheDir"
              placeholder="default: app data /cache"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            />
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Cache size limit (MB)</span>
            <input
              type="number"
              min="64"
              :value="settingsStore.cacheSizeLimitMb"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
              @input="onNumber('cacheSizeLimitMb', $event)"
            />
          </label>
        </div>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Network</h2>
        <div class="grid grid-cols-3 gap-3">
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Download limit (KB/s)</span>
            <input
              type="number"
              min="0"
              :value="settingsStore.downloadSpeedLimitKbps"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
              @input="onNumber('downloadSpeedLimitKbps', $event)"
            />
            <span class="mt-1 block text-xs text-slate-500">0 = unlimited</span>
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Upload limit (KB/s)</span>
            <input
              type="number"
              min="0"
              :value="settingsStore.uploadSpeedLimitKbps"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
              @input="onNumber('uploadSpeedLimitKbps', $event)"
            />
            <span class="mt-1 block text-xs text-slate-500">0 = unlimited</span>
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Connection limit</span>
            <input
              type="number"
              min="10"
              :value="settingsStore.connectionLimit"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
              @input="onNumber('connectionLimit', $event)"
            />
          </label>
        </div>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Player & subtitles</h2>
        <div class="grid grid-cols-2 gap-3">
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Preferred audio language</span>
            <input
              v-model="settingsStore.preferredAudioLanguage"
              placeholder="e.g. eng"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            />
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Preferred subtitle language</span>
            <input
              v-model="settingsStore.preferredSubtitleLanguage"
              placeholder="e.g. eng"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            />
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-slate-300">Subtitle size (%)</span>
            <input
              type="number"
              min="50"
              max="200"
              :value="settingsStore.subtitleSize"
              class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
              @input="onNumber('subtitleSize', $event)"
            />
          </label>
        </div>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Privacy & behavior</h2>
        <div class="flex flex-col gap-2 text-sm text-slate-300">
          <label class="flex items-center gap-3">
            <input v-model="settingsStore.launchOnStartup" type="checkbox" class="h-4 w-4 accent-sky-500" />
            Launch on system startup
          </label>
          <label class="flex items-center gap-3">
            <input v-model="settingsStore.pauseTorrentsOnExit" type="checkbox" class="h-4 w-4 accent-sky-500" />
            Pause torrents when closing the app
          </label>
          <label class="flex items-center gap-3">
            <input v-model="settingsStore.diagnosticsOptIn" type="checkbox" class="h-4 w-4 accent-sky-500" />
            Share opt-in diagnostics (off by default; StreamNest never collects torrent history)
          </label>
        </div>
      </section>

      <button
        class="w-fit rounded-md bg-red-500/15 px-4 py-2 text-sm text-red-300 hover:bg-red-500/25"
        @click="resetSettings"
      >
        Reset all settings
      </button>
    </div>
  </div>
</template>
