<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { torrentStore, torrentLive } from "../stores/torrents";
import { openInPlayer } from "../stores/player";
import { playerState, playerSeek } from "../composables/usePlayer";

const route = useRoute();
const router = useRouter();

const isDev = import.meta.env.DEV;
const navOpen = ref(false);
let ranAutoStream = false;

const nav = [
  { path: "/", label: "Home", key: "1" },
  { path: "/discover", label: "Discover", key: "2" },
  { path: "/search", label: "Search", key: "3" },
  { path: "/add", label: "Add Source", key: "4" },
  { path: "/sources", label: "Sources", key: "5" },
  { path: "/downloads", label: "Downloads", key: "6" },
  { path: "/library", label: "Library", key: "7" },
  { path: "/favorites", label: "Favorites", key: "8" },
  { path: "/settings", label: "Settings", key: "9" },
  { path: "/about", label: "About", key: "0" },
];

const activeCount = computed(() => torrentStore.torrents.filter((t) => torrentLive(t)).length);

function onKeydown(e) {
  if (e.key === "Escape") {
    navOpen.value = false;
    return;
  }
  if (!e.altKey || e.ctrlKey || e.shiftKey) return;
  const item = nav.find((n) => n.key === e.key);
  if (item) {
    e.preventDefault();
    router.push(item.path);
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  if (isDev) {
    const stop = setInterval(() => {
      if (torrentStore.engineReady && !ranAutoStream) {
        clearInterval(stop);
        setTimeout(devAutoStream, 1200);
      }
    }, 1000);
  }
});

onUnmounted(() => window.removeEventListener("keydown", onKeydown));

function go(item) {
  navOpen.value = false;
  router.push(item.path);
}

async function devAutoStream() {
  if (ranAutoStream) return;
  ranAutoStream = true;
  try {
    const list = await invoke("torrent_list");
    const t = (list?.torrents ?? [])[0];
    if (!t) {
      ranAutoStream = false;
      return;
    }
    const id = t.id ?? t.info_hash;
    const url = await invoke("stream_url", { id, fileId: 0 });
    openInPlayer({
      src: url,
      title: t.name ?? "auto-test",
      torrentId: id,
      fileId: 0,
      infoHash: t.info_hash ?? null,
    });
    invoke("dev_log", { message: `E2E auto-stream started id=${id}` }).catch(() => {});
    router.push("/player");
    setTimeout(async () => {
      if (playerState.duration > 320) {
        invoke("dev_log", { message: "E2E auto-seek to 300s" }).catch(() => {});
        await playerSeek(300);
      }
    }, 5000);
  } catch (err) {
    invoke("dev_log", { message: `E2E auto-stream error ${err}` }).catch(() => {});
  }
}

watch(
  () => torrentStore.engineReady,
  (ready) => {
    if (ready && isDev && !ranAutoStream) {
      setTimeout(devAutoStream, 1500);
    }
  }
);
</script>

<template>
  <div class="relative h-full w-full">
    <button
      class="absolute left-4 top-4 z-50 flex h-11 w-11 items-center justify-center rounded-full border border-white/10 bg-[#111827]/85 text-lg text-slate-100 shadow-lg backdrop-blur-xl transition hover:bg-white/10"
      title="Menu (Esc closes)"
      @click="navOpen = !navOpen"
    >
      ☰
    </button>

    <main
      class="relative h-full w-full"
      :class="route.path === '/player' ? 'bg-transparent' : 'bg-[#0b0f17] pt-14'"
    >
      <RouterView />
    </main>

    <div v-if="navOpen" class="fixed inset-0 z-40" @click.self="navOpen = false">
      <div
        class="absolute left-4 top-16 z-50 w-72 overflow-hidden rounded-2xl border border-white/10 bg-[#111827]/90 shadow-2xl shadow-black/60 backdrop-blur-xl"
        @click.stop
      >
        <div class="flex items-center gap-2 border-b border-white/10 px-4 py-3">
          <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-sky-500/15 text-sm font-bold text-sky-400">S</div>
          <span class="text-sm font-bold tracking-wide text-slate-100">StreamNest</span>
        </div>
        <nav class="p-2">
          <RouterLink
            v-for="item in nav"
            :key="item.path"
            :to="item.path"
            class="flex items-center justify-between rounded-lg px-3 py-2 text-sm transition"
            :class="route.path === item.path ? 'bg-sky-500/15 text-sky-300' : 'text-slate-300 hover:bg-white/10'"
            @click="navOpen = false"
          >
            <span>{{ item.label }}</span>
            <span class="flex items-center gap-2">
              <span
                v-if="item.path === '/downloads' && activeCount"
                class="rounded bg-sky-500/20 px-1.5 text-xs text-sky-300"
              >
                {{ activeCount }}
              </span>
              <kbd class="text-[10px] text-slate-500">Alt+{{ item.key }}</kbd>
            </span>
          </RouterLink>
        </nav>
        <div class="border-t border-white/10 px-4 py-3">
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
      </div>
    </div>
  </div>
</template>
