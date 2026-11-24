<script setup>
import AppState from "../components/common/AppState.vue";
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Media Library</h1>
      <p class="mt-1 text-sm text-slate-400">Recently played, history, and local organization.</p>
    </header>

    <AppState
      state="empty"
      title="Library arrives with the local library milestone"
      message="Playback history, continue-watching positions, and torrent media organization are stored in SQLite from Phase 6 onward. Nothing is recorded until then."
    />
  </div>
</template>
