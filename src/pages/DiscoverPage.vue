<script setup>
import AppState from "../components/common/AppState.vue";
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Discover</h1>
      <p class="mt-1 text-sm text-slate-400">
        A curated catalog of legally authorized content.
      </p>
    </header>

    <AppState
      state="empty"
      title="Catalog providers arrive in the catalog milestone"
      message="StreamNest ships only rights-verified sources with attribution and license info. No piracy indexes — by design. The provider architecture (source registry, search, filters, posters) is built next."
    >
      <p class="max-w-md text-xs text-slate-500">
        Until then, use Add Torrent with content you are legally authorized to access. The catalog will list each source, its rights documentation, and removal procedures.
      </p>
    </AppState>
  </div>
</template>
