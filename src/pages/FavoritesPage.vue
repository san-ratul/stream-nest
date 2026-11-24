<script setup>
import AppState from "../components/common/AppState.vue";
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Favorites</h1>
      <p class="mt-1 text-sm text-slate-400">Media you marked as favorite.</p>
    </header>

    <AppState
      state="empty"
      title="Favorites arrive with the local library milestone"
      message="Favorites are stored in SQLite alongside playback history from Phase 6 onward."
    />
  </div>
</template>
