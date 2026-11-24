<script setup>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { torrentStore, addTorrentFileBase64, streamUrl } from "../stores/torrents";
import { openInPlayer } from "../stores/player";
import { useRouter } from "vue-router";

const router = useRouter();
const e2eLog = ref([]);
const busy = ref(false);

function addLog(line) {
  const ts = new Date().toISOString().slice(11, 23);
  e2eLog.value.unshift(`[${ts}] ${line}`);
  invoke("dev_log", { message: `E2E ${line}` }).catch(() => {});
}

async function runE2e() {
  busy.value = true;
  try {
    addLog("reading test torrent (Sintel)");
    const b64 = await invoke("dev_read_test_torrent", {
      name: "Sintel.2010.1080p.mkv.torrent",
    });
    addLog("adding torrent with initial peer 127.0.0.1:6900");
    const res = await addTorrentFileBase64(b64, ["127.0.0.1:6900"]);
    const id = res.id;
    addLog(`torrent added id=${id}`);
    const details = await invoke("torrent_details", { id });
    const files = details?.files ?? [];
    addLog(`files=${files.length ? JSON.stringify(files.map((f) => f.name)) : "metadata pending"}`);
    const url = await streamUrl(id, 0);
    addLog("stream url ready — opening player");
    openInPlayer({ src: url, title: details?.name ?? "Sintel", torrentId: id, fileId: 0 });
    router.push("/player");
  } catch (err) {
    addLog(`ERROR ${err}`);
  } finally {
    busy.value = false;
  }
}

async function runLocalVideo() {
  openInPlayer({ src: "E:\\streamnest\\.testmedia\\bbb_1080_10s.mp4", title: "Big Buck Bunny (local)" });
  router.push("/player");
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Dev Harness</h1>
      <p class="mt-1 text-sm text-slate-400">
        Milestone verification tools. Not part of the user-facing product (dev builds only).
      </p>
    </header>

    <div class="flex max-w-xl flex-col gap-4">
      <div class="rounded-lg border border-slate-700/60 bg-slate-800/40 p-4">
        <div class="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-400">
          Torrent streaming E2E (local seeder :6900, 3 MB/s throttle)
        </div>
        <div class="flex gap-2">
          <button
            class="rounded-md bg-emerald-500 px-4 py-1.5 text-sm font-medium text-slate-950 hover:bg-emerald-400 disabled:opacity-50"
            :disabled="busy || !torrentStore.engineReady"
            @click="runE2e"
          >
            {{ busy ? "Running..." : "Run E2E" }}
          </button>
          <button
            class="rounded-md bg-slate-700 px-4 py-1.5 text-sm text-slate-200 hover:bg-slate-600"
            @click="runLocalVideo"
          >
            Play local file
          </button>
        </div>
        <p class="mt-2 text-xs text-slate-500">Requires: cargo run --bin seeder -- E:\streamnest\.testmedia 6900 3000000</p>
      </div>

      <div v-if="e2eLog.length" class="rounded-lg border border-slate-700/60 bg-slate-900/60 p-3">
        <div class="max-h-64 overflow-y-auto font-mono text-[11px] leading-4 text-slate-300">
          <div v-for="(line, i) in e2eLog" :key="i">{{ line }}</div>
        </div>
      </div>
    </div>
  </div>
</template>
