<script setup>
import { computed } from "vue";
import { torrentName, torrentProgress, fmtBytes, fmtSpeed } from "../../stores/torrents";

const props = defineProps({
  torrent: { type: Object, required: true },
  active: { type: Boolean, default: false },
});
const emit = defineEmits(["open"]);

const progress = computed(() => torrentProgress(props.torrent));
const live = computed(() => Boolean(props.torrent?.stats?.live || props.torrent?.live));
const downloaded = computed(() => props.torrent?.stats?.downloaded_bytes ?? 0);
const total = computed(() => props.torrent?.stats?.total_bytes ?? 0);
const downSpeed = computed(() => props.torrent?.stats?.live?.downloaded ?? props.torrent?.stats?.downloaded_bytes_rate ?? 0);
const errorText = computed(() => props.torrent?.stats?.error ?? null);
</script>

<template>
  <button
    class="w-full rounded-lg border px-4 py-3 text-left transition-colors"
    :class="active ? 'border-sky-500/60 bg-slate-800/70' : 'border-slate-700/60 bg-slate-800/40 hover:bg-slate-800/80'"
    @click="emit('open', torrent)"
  >
    <div class="flex items-center justify-between gap-3">
      <span class="truncate text-sm font-medium text-slate-100">{{ torrentName(torrent) }}</span>
      <span
        v-if="errorText"
        class="shrink-0 rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-300"
      >error</span>
      <span
        v-else-if="progress >= 100"
        class="shrink-0 rounded bg-emerald-500/15 px-2 py-0.5 text-xs text-emerald-300"
      >complete</span>
      <span
        v-else-if="live"
        class="shrink-0 rounded bg-sky-500/15 px-2 py-0.5 text-xs text-sky-300"
      >downloading</span>
      <span v-else class="shrink-0 rounded bg-slate-500/15 px-2 py-0.5 text-xs text-slate-300">paused</span>
    </div>
    <div class="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-slate-700/70">
      <div
        class="h-full rounded-full transition-all"
        :class="progress >= 100 ? 'bg-emerald-400' : 'bg-sky-400'"
        :style="{ width: `${progress}%` }"
      ></div>
    </div>
    <div class="mt-1.5 flex items-center gap-4 text-xs text-slate-400">
      <span>{{ progress }}%</span>
      <span>{{ fmtBytes(downloaded) }} / {{ fmtBytes(total) }}</span>
      <span v-if="live && downSpeed">{{ fmtSpeed(downSpeed) }}</span>
      <span v-if="errorText" class="truncate text-red-300">{{ errorText }}</span>
    </div>
  </button>
</template>
