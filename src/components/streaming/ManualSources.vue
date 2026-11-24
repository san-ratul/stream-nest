<script setup>
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { sourceTypeLabel, fmtBytes } from "../../stores/sources";
import { startRecordPlayback, playbackStore } from "../../composables/playback";

const props = defineProps({
  records: { type: Array, required: true }, // SourceRecord[]
  // Optional episode filter: only rows matching season/episode (or unlinked rows)
  season: { type: Number, default: null },
  episode: { type: Number, default: null },
  busy: { type: Boolean, default: false },
});
const emit = defineEmits(["changed", "open-add"]);

const busyId = ref(null);
const rowError = ref(null);
const chooserVideos = ref(null);
let chooserResolve = null;

const visible = computed(() =>
  props.records.filter((r) => {
    if (props.season == null) return true;
    // For an episode context: rows linked to that episode, or rows linked to the
    // show without a specific episode (season packs).
    if (r.season_number == null && r.episode_number == null) return true;
    return r.season_number === props.season && r.episode_number === props.episode;
  })
);

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

async function playRecord(record) {
  busyId.value = record.id;
  rowError.value = null;
  try {
    const result = await startRecordPlayback(record, {
      onNeedFileChoice: fileChooser,
      title: record.display_name,
    });
    if (result?.started) {
      // The caller (details page) navigates to the player via router push — emit event instead.
      emit("changed", { action: "played" });
      return true;
    }
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
  return false;
}

async function retryMetadata(record) {
  busyId.value = record.id;
  rowError.value = null;
  try {
    await invoke("playback_prepare_record", { sourceId: record.id });
    emit("changed", { action: "prepared" });
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
}

async function removeRecord(record) {
  busyId.value = record.id;
  rowError.value = null;
  try {
    await invoke("source_remove", { id: record.id });
    emit("changed", { action: "removed" });
  } catch (err) {
    rowError.value = String(err?.message ?? err);
  } finally {
    busyId.value = null;
  }
}

function fmtSize(n) {
  return n == null ? "" : fmtBytes(n);
}

defineExpose({ playRecord });
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">Your linked sources</div>
    <p v-if="rowError" class="rounded border border-red-500/40 bg-red-500/10 px-2 py-1 text-xs text-red-300">{{ rowError }}</p>
    <div
      v-for="r in visible"
      :key="r.id"
      class="rounded-lg border border-slate-700/60 bg-slate-900/50 p-3"
    >
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-sm font-medium text-slate-200">{{ r.display_name }}</span>
            <span class="rounded bg-slate-700/60 px-1.5 py-0.5 text-[10px] font-bold uppercase text-slate-300">
              {{ sourceTypeLabel(r.source_type) }}
            </span>
            <span v-if="r.season_number != null" class="rounded bg-sky-500/15 px-1.5 py-0.5 text-[10px] text-sky-300">
              S{{ String(r.season_number).padStart(2, "0") }}<template v-if="r.episode_number != null">E{{ String(r.episode_number).padStart(2, "0") }}</template>
            </span>
          </div>
          <div class="mt-0.5 text-xs text-slate-500">
            {{ r.info_hash ? r.info_hash.slice(0, 16) + "…" : "hash unknown" }}
            <template v-if="r.total_size"> · {{ fmtSize(r.total_size) }}</template>
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-1.5">
          <button
            class="rounded bg-emerald-500 px-3 py-1.5 text-xs font-semibold text-slate-950 hover:bg-emerald-400 disabled:opacity-50"
            :disabled="busy || busyId === r.id"
            @click="playRecord(r)"
          >
            {{ busyId === r.id ? (playbackStore.stage === "starting" ? "Starting…" : "Preparing…") : "▶ Play" }}
          </button>
          <button class="rounded bg-slate-800 px-2 py-1 text-[11px] text-slate-400 hover:bg-slate-700" :disabled="busyId === r.id" @click="retryMetadata(r)">
            refresh
          </button>
          <button class="rounded bg-slate-800 px-2 py-1 text-[11px] text-red-300 hover:bg-slate-700" :disabled="busyId === r.id" @click="removeRecord(r)">
            remove
          </button>
        </div>
      </div>
    </div>
    <button
      class="rounded-lg border border-dashed border-slate-600 px-3 py-2 text-xs text-slate-400 hover:border-sky-500/50 hover:text-sky-300"
      @click="emit('open-add')"
    >
      + Add a source manually (magnet, torrent URL or file)
    </button>

    <div
      v-if="chooserVideos"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6"
      @click.self="cancelChooser"
    >
      <div class="w-full max-w-xl rounded-lg border border-slate-700 bg-slate-900 p-4">
        <h3 class="text-lg font-semibold text-slate-100">Select a video file</h3>
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
      </div>
    </div>
  </div>
</template>
