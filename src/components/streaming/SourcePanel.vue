<script setup>
import { computed, ref } from "vue";

const props = defineProps({
  sources: { type: Array, required: true }, // AuthorizedSource[]
  busySource: { type: String, default: null }, // provider_id currently preparing
  stage: { type: String, default: null }, // playback stage label
});
const emit = defineEmits(["play"]);

const STAGE_LABELS = {
  connecting: "Connecting to source…",
  metadata: "Retrieving metadata…",
  files: "Inspecting files…",
  starting: "Starting stream…",
};

function kindLabel(kind) {
  switch (kind) {
    case "official_torrent":
      return "Official torrent";
    case "dev_fixture":
      return "DEV FIXTURE (local seeder)";
    default:
      return "Official page";
  }
}

function kindClass(kind) {
  switch (kind) {
    case "official_torrent":
      return "bg-emerald-500/15 text-emerald-300";
    case "dev_fixture":
      return "bg-amber-500/15 text-amber-300";
    default:
      return "bg-slate-700/60 text-slate-300";
  }
}

function field(v) {
  return v ? v : null;
}
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">
      Authorized sources
    </div>
    <div
      v-for="s in sources"
      :key="s.provider_id"
      class="rounded-lg border border-slate-700/60 bg-slate-900/50 p-3"
    >
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-sm font-medium text-sky-300">{{ s.source_name }}</span>
            <span class="rounded px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide" :class="kindClass(s.source_kind)">
              {{ kindLabel(s.source_kind) }}
            </span>
            <span v-if="!s.available" class="rounded bg-red-500/15 px-1.5 py-0.5 text-[10px] font-bold uppercase text-red-300">unavailable</span>
          </div>
          <div class="mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-slate-400">
            <span v-if="field(s.quality)">Quality: {{ s.quality }}</span>
            <span v-if="field(s.size_label)">Size: {{ s.size_label }}</span>
            <span v-if="field(s.format)">Format: {{ s.format }}</span>
            <span v-if="field(s.language)">Language: {{ s.language }}</span>
          </div>
          <div class="mt-0.5 text-[11px] text-slate-500">
            License: {{ s.license }} ·
            <a :href="s.source_url" target="_blank" rel="noopener" class="text-sky-400 hover:underline">official page ↗</a>
          </div>
          <p v-if="s.attribution" class="mt-1 text-[11px] text-slate-600">{{ s.attribution }}</p>
        </div>
        <button
          v-if="s.playable_in_app"
          class="shrink-0 rounded-md bg-emerald-500 px-4 py-2 text-sm font-semibold text-slate-950 hover:bg-emerald-400 disabled:opacity-60"
          :disabled="busySource != null"
          @click="emit('play', s)"
        >
          {{ busySource === s.provider_id ? (STAGE_LABELS[stage] ?? "Preparing…") : "▶ Watch" }}
        </button>
        <a
          v-else
          :href="s.source_url"
          target="_blank"
          rel="noopener"
          class="shrink-0 rounded bg-sky-500/20 px-3 py-1.5 text-xs font-medium text-sky-300 hover:bg-sky-500/30"
        >
          Open official page ↗
        </a>
      </div>
    </div>
    <p class="text-[11px] text-slate-500">
      Sources are verified for distribution rights. TMDB metadata alone never makes a title streamable.
    </p>
  </div>
</template>
