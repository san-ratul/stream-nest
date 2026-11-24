<script setup>
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { addTorrentFileBase64 } from "../stores/torrents";
import {
  playbackStore,
  friendlyError,
  startRecordPlayback,
} from "../composables/playback";
import { sourceContext } from "../stores/sources";

const route = useRoute();
const router = useRouter();

const TABS = [
  { key: "magnet", label: "Magnet link" },
  { key: "url", label: "Torrent URL" },
  { key: "file", label: "Torrent file" },
  { key: "page", label: "Provider page" },
];

const tab = ref("magnet");
const busy = ref(false);
const stage = ref(null); // fetching | saved | prepared
const error = ref(null);
const record = ref(null);
const prepared = ref(null);
const magnetUri = ref("");
const torrentUrl = ref("");
const pageUrl = ref("");
const fileInput = ref(null);

// Association context from query (?type=movie&id=..&season=..&episode=..)
const assoc = computed(() => {
  const { type, id, season, episode } = route.query;
  if (!type || !id) return null;
  return sourceContext({
    mediaType: type,
    tmdbId: Number(id),
    season: season != null ? Number(season) : null,
    episode: episode != null ? Number(episode) : null,
  });
});
const assocLabel = computed(() => {
  const { type, id, season, episode } = route.query;
  if (!type || !id) return null;
  if (type === "movie") return `linked to movie TMDB #${id}`;
  return `linked to show TMDB #${id}${season != null ? ` · S${String(season).padStart(2, "0")}` : ""}${episode != null ? `E${String(episode).padStart(2, "0")}` : ""}`;
});

const MAGNET_RE = /^magnet:\?xt=urn:btih:[a-zA-Z0-9]{32,40}(&.*)?$/;
const isValidMagnet = computed(() => MAGNET_RE.test(magnetUri.value.trim()));
const isValidUrl = computed(() => /^https?:\/\/\S+$/.test(torrentUrl.value.trim()));
const isValidPage = computed(() => /^https?:\/\/\S+$/.test(pageUrl.value.trim()));

onMounted(() => {
  if (route.query.tab && TABS.some((t) => t.key === route.query.tab)) {
    tab.value = route.query.tab;
  }
});

function reset() {
  record.value = null;
  prepared.value = null;
  stage.value = null;
  error.value = null;
}

async function submit() {
  error.value = null;
  busy.value = true;
  stage.value = "fetching";
  try {
    let rec;
    if (tab.value === "magnet") {
      if (!isValidMagnet.value) {
        error.value = "Enter a valid magnet URI (magnet:?xt=urn:btih:...)";
        return;
      }
      rec = await invoke("source_add_magnet", { uri: magnetUri.value.trim(), context: assoc.value });
    } else if (tab.value === "url") {
      if (!isValidUrl.value) {
        error.value = "Enter a direct http(s) URL to a .torrent file";
        return;
      }
      rec = await invoke("source_add_torrent_url", { url: torrentUrl.value.trim(), context: assoc.value });
    } else if (tab.value === "page") {
      if (!isValidPage.value) {
        error.value = "Enter a provider page URL";
        return;
      }
      const resolved = await invoke("source_resolve_page", { url: pageUrl.value.trim(), context: assoc.value });
      // Resolved page → fetch the torrent through the same safe URL path.
      rec = await invoke("source_add_torrent_url", { url: resolved.url, context: assoc.value });
    } else {
      const file = fileInput.value?.files?.[0];
      if (!file) {
        error.value = "Choose a .torrent file first.";
        return;
      }
      if (!file.name.toLowerCase().endsWith(".torrent")) {
        error.value = "The selected file is not a .torrent file.";
        return;
      }
      const buf = await file.arrayBuffer();
      const bytes = new Uint8Array(buf);
      let binary = "";
      for (let i = 0; i < bytes.length; i += 0x8000) {
        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      }
      rec = await invoke("source_add_file_base64", {
        contentsBase64: btoa(binary),
        name: file.name,
        context: assoc.value,
      });
    }
    record.value = rec;
    stage.value = "saved";
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    busy.value = false;
    stage.value = record.value ? "saved" : null;
  }
}

async function prepare() {
  error.value = null;
  busy.value = true;
  stage.value = "preparing";
  try {
    prepared.value = await invoke("playback_prepare_record", { sourceId: record.value.id });
    stage.value = "prepared";
  } catch (err) {
    error.value = friendlyError(err);
    stage.value = "saved";
  } finally {
    busy.value = false;
  }
}

// File choice (multiple videos / ambiguous episodes)
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

async function startStreaming() {
  error.value = null;
  busy.value = true;
  try {
    const result = await startRecordPlayback(record.value, {
      onNeedFileChoice: fileChooser,
      title: record.value.display_name,
    });
    if (result?.started) {
      router.push("/player");
    }
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    busy.value = false;
    stage.value = "prepared";
  }
}

function fmtSize(n) {
  if (n == null) return "Unknown";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  return `${v.toFixed(v >= 10 || u === 0 ? 0 : 1)} ${units[u]}`;
}

function shortHash(hash) {
  return hash ? `${hash.slice(0, 12)}…` : "Unknown until metadata";
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-4">
      <h1 class="text-2xl font-bold text-slate-100">Add source</h1>
      <p class="mt-1 text-sm text-slate-400">
        Add any torrent source manually: magnet link, direct .torrent URL, local file or a supported provider page.
        Technical validation only — no content classification.
      </p>
      <span v-if="assocLabel" class="mt-2 inline-block rounded bg-sky-500/15 px-2 py-0.5 text-xs text-sky-300">
        {{ assocLabel }}
      </span>
    </header>

    <div class="mb-4 flex flex-wrap gap-2">
      <button
        v-for="t in TABS"
        :key="t.key"
        class="rounded-md px-3 py-1.5 text-sm font-medium"
        :class="tab === t.key ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'"
        @click="tab = t.key; reset()"
      >
        {{ t.label }}
      </button>
    </div>

    <p v-if="error" class="mb-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">{{ error }}</p>

    <div v-if="stage === 'fetching'" class="rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-6 text-sm text-slate-300">
      <span class="inline-flex items-center gap-2">
        <span class="h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-sky-400"></span>
        {{ tab === "url" || tab === "page" ? "Fetching torrent securely…" : "Validating…" }}
      </span>
    </div>

    <template v-else-if="!record">
      <!-- Input forms -->
      <div class="max-w-2xl">
        <div v-if="tab === 'magnet'">
          <input
            v-model="magnetUri"
            placeholder="magnet:?xt=urn:btih:…"
            class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            @keyup.enter="submit"
          />
          <p class="mt-1.5 text-xs" :class="magnetUri ? (isValidMagnet ? 'text-emerald-400' : 'text-red-400') : 'text-slate-500'">
            {{ magnetUri ? (isValidMagnet ? "✓ Valid magnet URI" : "✗ Not a valid magnet URI") : "Paste a magnet URI." }}
          </p>
        </div>

        <div v-else-if="tab === 'url'">
          <input
            v-model="torrentUrl"
            placeholder="https://example.com/file.torrent"
            class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            @keyup.enter="submit"
          />
          <p class="mt-1.5 text-xs" :class="torrentUrl ? (isValidUrl ? 'text-emerald-400' : 'text-red-400') : 'text-slate-500'">
            {{ torrentUrl ? (isValidUrl ? "✓ Will fetch over the network (private/local addresses blocked)" : "✗ Enter an http(s) URL") : "Direct link to a .torrent file. Size- and redirect-bounded." }}
          </p>
        </div>

        <div v-else-if="tab === 'page'">
          <input
            v-model="pageUrl"
            placeholder="https://archive.org/details/identifier"
            class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
            @keyup.enter="submit"
          />
          <p class="mt-1.5 text-xs text-slate-500">
            Supported: Internet Archive item pages. Any other provider:
            “This provider page is not supported. Try a direct Magnet Link or .torrent URL.”
          </p>
        </div>

        <div v-else>
          <label class="flex w-full cursor-pointer items-center justify-center rounded-lg border border-dashed border-slate-600 px-4 py-8 text-sm text-slate-400 hover:border-sky-500/50">
            <input ref="fileInput" type="file" accept=".torrent" class="hidden" />
            Click to choose a .torrent file — your original file is never moved or deleted
          </label>
        </div>

        <div class="mt-4 flex items-center gap-2">
          <button
            class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400 disabled:opacity-50"
            :disabled="busy"
            @click="submit"
          >
            Add source
          </button>
          <button class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600" @click="reset">
            Cancel
          </button>
        </div>
      </div>
    </template>

    <template v-else>
      <!-- Preview + confirm -->
      <div class="max-w-2xl rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <div class="flex items-center justify-between gap-2">
          <div class="min-w-0">
            <div class="truncate text-lg font-semibold text-slate-100">{{ record.display_name }}</div>
            <div class="mt-0.5 text-xs text-slate-500">
              type: {{ record.source_type }} · hash: {{ shortHash(record.info_hash) }}
              <template v-if="record.total_size"> · size: {{ fmtSize(record.total_size) }}</template>
            </div>
          </div>
          <span class="rounded bg-emerald-500/15 px-2 py-0.5 text-xs text-emerald-300">saved</span>
        </div>

        <div v-if="prepared" class="mt-3 rounded-md border border-slate-700/60 bg-slate-900/40 p-3">
          <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">Metadata retrieved</div>
          <div class="mt-1 text-sm text-slate-200">
            {{ prepared.torrent_name }} · {{ prepared.videos.length }} video file{{ prepared.videos.length === 1 ? "" : "s" }}
            <template v-if="prepared.total_files"> of {{ prepared.total_files }} total</template>
          </div>
          <ul class="mt-2 max-h-40 overflow-y-auto text-xs text-slate-400">
            <li v-for="v in prepared.videos" :key="v.file_index" class="truncate">
              <span class="text-sky-400">video</span> · {{ v.path }} ({{ fmtSize(v.size_bytes) }})
            </li>
          </ul>
        </div>

        <p v-if="playbackStore.error" class="mt-3 rounded border border-red-500/40 bg-red-500/10 px-2 py-1 text-xs text-red-300">
          {{ playbackStore.error }}
        </p>

        <div class="mt-4 flex flex-wrap gap-2">
          <button
            v-if="!prepared"
            class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600 disabled:opacity-50"
            :disabled="busy"
            @click="prepare"
          >
            {{ busy ? "Retrieving metadata…" : "Retrieve metadata & files" }}
          </button>
          <button
            class="rounded-md bg-emerald-500 px-4 py-2 text-sm font-semibold text-slate-950 hover:bg-emerald-400 disabled:opacity-50"
            :disabled="busy"
            @click="startStreaming"
          >
            {{ playbackStore.stage === "starting" ? "Starting…" : "▶ Start streaming" }}
          </button>
          <button class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600" @click="router.push('/sources')">
            Go to source library
          </button>
        </div>
        <p class="mt-2 text-[11px] text-slate-500">
          Streaming begins progressively — the full torrent is never required up front.
        </p>
      </div>
    </template>

    <!-- Ambiguous file chooser -->
    <div
      v-if="chooserVideos"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6"
      @click.self="cancelChooser"
    >
      <div class="w-full max-w-xl rounded-lg border border-slate-700 bg-slate-900 p-4">
        <h3 class="text-lg font-semibold text-slate-100">Select a video file</h3>
        <p class="mt-1 text-xs text-slate-400">Multiple video files detected. Choose the one to stream.</p>
        <div class="mt-3 flex max-h-80 flex-col gap-1 overflow-y-auto">
          <button
            v-for="v in chooserVideos"
            :key="v.file_index"
            class="flex items-center justify-between rounded border border-slate-700/60 px-3 py-2 text-left text-sm text-slate-200 hover:border-sky-500/50 hover:bg-slate-800"
            @click="resolveChooser(v.file_index)"
          >
            <span class="min-w-0 truncate">{{ v.path }}</span>
            <span class="shrink-0 text-xs text-slate-500">{{ fmtSize(v.size_bytes) }}</span>
          </button>
        </div>
        <div class="mt-3 flex justify-end">
          <button class="rounded bg-slate-700 px-3 py-1.5 text-sm text-slate-200 hover:bg-slate-600" @click="cancelChooser">
            Cancel
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
