<script setup>
import { computed, onMounted, onUnmounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import { torrentStore, torrentLive } from "../stores/torrents";

const route = useRoute();
const router = useRouter();

const nav = [
  { path: "/", label: "Home", key: "1" },
  { path: "/discover", label: "Discover", key: "2" },
  { path: "/add", label: "Add Torrent", key: "3" },
  { path: "/downloads", label: "Downloads", key: "4" },
  { path: "/library", label: "Library", key: "5" },
  { path: "/favorites", label: "Favorites", key: "6" },
  { path: "/settings", label: "Settings", key: "7" },
  { path: "/about", label: "About", key: "8" },
];

const activeCount = computed(() => torrentStore.torrents.filter((t) => torrentLive(t)).length);

function onKeydown(e) {
  if (!e.altKey || e.ctrlKey || e.shiftKey) return;
  const item = nav.find((n) => n.key === e.key);
  if (item) {
    e.preventDefault();
    router.push(item.path);
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="flex h-full w-full">
    <aside class="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-[#0d1220]">
      <div class="flex items-center gap-2 px-4 py-4">
        <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-sky-500/15 text-lg font-bold text-sky-400">S</div>
        <span class="text-sm font-bold tracking-wide text-slate-100">StreamNest</span>
      </div>
      <nav class="flex-1 space-y-1 px-2">
        <RouterLink
          v-for="item in nav"
          :key="item.path"
          :to="item.path"
          class="flex items-center justify-between rounded-md px-3 py-2 text-sm transition-colors"
          :class="route.path === item.path ? 'bg-sky-500/15 text-sky-300' : 'text-slate-300 hover:bg-slate-800/70'"
        >
          <span>{{ item.label }}</span>
          <span v-if="item.path === '/downloads' && activeCount" class="rounded bg-sky-500/20 px-1.5 text-xs text-sky-300">
            {{ activeCount }}
          </span>
          <kbd class="text-[10px] text-slate-500">Alt+{{ item.key }}</kbd>
        </RouterLink>
      </nav>
      <div class="border-t border-slate-800 px-4 py-3">
        <div class="flex items-center gap-2 text-xs">
          <span
            class="h-2 w-2 rounded-full"
            :class="torrentStore.engineReady ? 'bg-emerald-400' : torrentStore.engineError ? 'bg-red-400' : 'bg-amber-400 animate-pulse'"
          ></span>
          <span class="text-slate-400">
            {{ torrentStore.engineReady ? "engine ready" : torrentStore.engineError ? "engine error" : "engine starting" }}
          </span>
        </div>
      </div>
    </aside>
    <main class="relative min-w-0 flex-1 bg-[#0b0f17]">
      <RouterView />
    </main>
  </div>
</template>
