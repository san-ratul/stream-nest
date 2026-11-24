<script setup>
import { ref } from "vue";

const version = "0.1.0";

const licenses = [
  { name: "librqbit", license: "MIT", role: "BitTorrent engine" },
  { name: "tauri-plugin-libmpv", license: "MPL-2.0", role: "mpv embedding plugin" },
  { name: "libmpv-2.dll", license: "LGPL (build: mpv-dev-lgpl)", role: "media playback" },
  { name: "Tauri", license: "MIT / Apache-2.0", role: "desktop framework" },
  { name: "Vue 3 / Vite / Tailwind CSS 4", license: "MIT", role: "frontend stack" },
  { name: "Test media", license: "CC-BY (Blender Foundation)", role: "Sintel, Big Buck Bunny" },
];

const showLicenses = ref(true);
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">About StreamNest</h1>
      <p class="mt-1 text-sm text-slate-400">Version {{ version }} — open-source core, privacy-first.</p>
    </header>

    <div class="flex max-w-2xl flex-col gap-6">
      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4 text-sm text-slate-300">
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">Product</h2>
        <p>
          StreamNest streams video through BitTorrent progressively — start watching before the download finishes —
          with an integrated mpv-based player and a local-first media library. No account required, no telemetry,
          no torrent history collection.
        </p>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4 text-sm text-slate-300">
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">Content policy</h2>
        <p>
          StreamNest does not index, host, or recommend content. You are responsible for using only content you are
          legally authorized to access. The built-in catalog lists only rights-verified sources with attribution.
        </p>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4 text-sm text-slate-300">
        <button
          class="mb-3 flex w-full items-center justify-between text-sm font-semibold uppercase tracking-wide text-slate-400"
          @click="showLicenses = !showLicenses"
        >
          <span>Open-source licenses</span>
          <span>{{ showLicenses ? "−" : "+" }}</span>
        </button>
        <div v-if="showLicenses" class="overflow-hidden rounded-md border border-slate-700/60">
          <div
            v-for="l in licenses"
            :key="l.name"
            class="flex items-center justify-between border-b border-slate-800 px-3 py-2 text-sm last:border-b-0"
          >
            <span class="font-medium text-slate-200">{{ l.name }}</span>
            <span class="text-xs text-slate-400">{{ l.role }}</span>
            <span class="rounded bg-slate-700/60 px-2 py-0.5 text-xs text-slate-300">{{ l.license }}</span>
          </div>
        </div>
        <p class="mt-2 text-xs text-slate-500">
          Full third-party notices ship with the installer (BUILD_AND_RELEASE.md).
        </p>
      </section>

      <section class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4 text-sm text-slate-300">
        <h2 class="mb-2 text-sm font-semibold uppercase tracking-wide text-slate-400">Keyboard shortcuts</h2>
        <ul class="space-y-1 text-slate-400">
          <li><kbd class="rounded bg-slate-700 px-1.5 text-xs">Alt+1..8</kbd> — navigate screens</li>
          <li><kbd class="rounded bg-slate-700 px-1.5 text-xs">Space</kbd> — play/pause (in player)</li>
          <li><kbd class="rounded bg-slate-700 px-1.5 text-xs">←/→</kbd> — seek ±10 s (in player)</li>
        </ul>
      </section>
    </div>
  </div>
</template>
