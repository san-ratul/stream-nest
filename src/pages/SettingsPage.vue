<script setup>
import { settingsStore, resetSettings } from "../stores/settings";

function onNumber(field, e) {
  const v = Number(e.target.value);
  settingsStore[field] = Number.isFinite(v) ? v : 0;
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
