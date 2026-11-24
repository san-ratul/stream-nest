<script setup>
import { computed } from "vue";
import { useRouter } from "vue-router";
import { torrentStore, torrentLive } from "../stores/torrents";
import TorrentRow from "../components/common/TorrentRow.vue";
import AppState from "../components/common/AppState.vue";

const router = useRouter();

const activeTorrents = computed(() => torrentStore.torrents.filter((t) => torrentLive(t)));
const recentTorrents = computed(() => torrentStore.torrents.slice(0, 5));
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-slate-100">Home</h1>
        <p class="mt-1 text-sm text-slate-400">Your media, your device, your control.</p>
      </div>
      <button
        class="rounded-md bg-sky-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400"
        @click="router.push('/add')"
      >
        + Add torrent
      </button>
    </header>

    <AppState
      v-if="torrentStore.engineError"
      state="error"
      title="Torrent engine failed to start"
      :message="torrentStore.engineError"
    />

    <div v-else class="flex flex-1 flex-col gap-6 overflow-y-auto">
      <section>
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">Continue watching</h2>
        <div class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Playback history and resume points arrive with the local library milestone (Phase 6).
        </div>
      </section>

      <section>
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Recent torrents
        </h2>
        <div v-if="recentTorrents.length" class="flex flex-col gap-2">
          <TorrentRow
            v-for="t in recentTorrents"
            :key="t.id ?? t.info_hash"
            :torrent="t"
            @open="router.push(`/torrent/${t.id ?? t.info_hash}`)"
          />
        </div>
        <div v-else-if="torrentStore.engineReady" class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          No torrents yet. Paste a magnet link or import a .torrent file to start streaming.
        </div>
        <div v-else class="rounded-lg border border-dashed border-slate-700 px-4 py-6 text-sm text-slate-400">
          Waiting for the torrent engine...
        </div>
      </section>

      <section v-if="activeTorrents.length">
        <h2 class="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Active now ({{ activeTorrents.length }})
        </h2>
        <div class="flex flex-col gap-2">
          <TorrentRow
            v-for="t in activeTorrents"
            :key="`active-${t.id ?? t.info_hash}`"
            :torrent="t"
            @open="router.push(`/torrent/${t.id ?? t.info_hash}`)"
          />
        </div>
      </section>
    </div>
  </div>
</template>
