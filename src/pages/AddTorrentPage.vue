<script setup>
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { addTorrentFileBase64, addTorrentUri, torrentStore } from "../stores/torrents";

const router = useRouter();

const mode = ref("magnet"); // magnet | file
const magnetUri = ref("");
const peersInput = ref("");
const busy = ref(false);
const error = ref(null);
const addedTorrent = ref(null);

const MAGNET_RE = /^magnet:\?xt=urn:btih:[a-zA-Z0-9]{32,40}(&.*)?$/;
const isValidMagnet = computed(() => MAGNET_RE.test(magnetUri.value.trim()));

function parsePeers() {
  const raw = peersInput.value.trim();
  if (!raw) return null;
  const parts = raw.split(/[,\s]+/).filter(Boolean);
  return parts;
}

async function submit() {
  error.value = null;
  addedTorrent.value = null;
  busy.value = true;
  try {
    let res;
    if (mode.value === "magnet") {
      if (!isValidMagnet.value) {
        error.value = "Enter a valid magnet URI (magnet:?xt=urn:btih:...)";
        return;
      }
      res = await addTorrentUri(magnetUri.value.trim(), parsePeers());
    } else {
      const file = fileInput.value?.files?.[0];
      if (!file) {
        error.value = "Choose a .torrent file first.";
        return;
      }
      if (!file.name.toLowerCase().endsWith(".torrent")) {
        error.value = "The selected file is not a .torrent file.";
        return;
      }
      const buf = await file.arrayBuffer();
      const bytes = new Uint8Array(buf);
      let binary = "";
      for (let i = 0; i < bytes.length; i += 0x8000) {
        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      }
      const b64 = btoa(binary);
      res = await addTorrentFileBase64(b64, parsePeers());
    }
    if (res?.error) {
      error.value = res.error;
      return;
    }
    addedTorrent.value = res;
    const hash = res?.details?.info_hash;
    if (hash) {
      invoke("db_upsert_torrent", {
        infoHash: hash,
        name: res?.details?.name ?? "unknown",
        outputFolder: res?.output_folder ?? null,
      }).catch(() => {});
    }
  } catch (err) {
    error.value = String(err?.message ?? err);
  } finally {
    busy.value = false;
  }
}

const fileInput = ref(null);

function goToDetails() {
  const t = addedTorrent.value;
  if (t?.id != null) {
    router.push(`/torrent/${t.id}`);
  } else if (t?.details?.info_hash) {
    router.push(`/torrent/${t.details.info_hash}`);
  }
}
</script>

<template>
  <div class="flex h-full flex-col p-6">
    <header class="mb-6">
      <h1 class="text-2xl font-bold text-slate-100">Add Torrent</h1>
      <p class="mt-1 text-sm text-slate-400">
        Paste a magnet link or import a .torrent file. Metadata is retrieved asynchronously — the UI stays responsive.
      </p>
    </header>

    <div class="flex max-w-2xl flex-1 flex-col gap-4 overflow-y-auto">
      <div class="flex gap-2">
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium"
          :class="mode === 'magnet' ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'"
          @click="mode = 'magnet'"
        >Magnet URI</button>
        <button
          class="rounded-md px-4 py-1.5 text-sm font-medium"
          :class="mode === 'file' ? 'bg-sky-500/20 text-sky-300' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'"
          @click="mode = 'file'"
        >.torrent file</button>
      </div>

      <div v-if="mode === 'magnet'">
        <label class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-400" for="magnet">Magnet URI</label>
        <textarea
          id="magnet"
          v-model="magnetUri"
          rows="3"
          placeholder="magnet:?xt=urn:btih:..."
          class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:border-sky-500 focus:outline-none"
          :class="magnetUri && !isValidMagnet ? 'border-red-500/60' : ''"
        ></textarea>
        <p v-if="magnetUri && !isValidMagnet" class="mt-1 text-xs text-red-300">
          Not a valid magnet URI. Expecting magnet:?xt=urn:btih:&lt;hash&gt;
        </p>
      </div>

      <div v-else>
        <label class="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-400" for="file">Torrent file</label>
        <input
          id="file"
          ref="fileInput"
          type="file"
          accept=".torrent"
          class="w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 text-sm text-slate-300 file:mr-3 file:rounded file:border-0 file:bg-sky-500/20 file:px-3 file:py-1 file:text-sky-300"
        />
      </div>

      <details class="text-sm text-slate-400">
        <summary class="cursor-pointer select-none">Advanced: initial peers (optional, comma separated)</summary>
        <input
          v-model="peersInput"
          placeholder="127.0.0.1:6900"
          class="mt-2 w-full rounded-md border border-slate-700 bg-slate-800/60 px-3 py-2 font-mono text-xs text-slate-100 focus:border-sky-500 focus:outline-none"
        />
      </details>

      <p v-if="error" class="rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300">
        {{ error }}
      </p>

      <div v-if="addedTorrent" class="rounded-md border border-emerald-500/40 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-200">
        Torrent added: {{ addedTorrent?.details?.name || "metadata pending" }}
        <button class="ml-2 font-semibold text-emerald-300 underline hover:text-emerald-200" @click="goToDetails">
          open details →
        </button>
      </div>

      <div class="flex gap-2">
        <button
          class="rounded-md bg-sky-500 px-5 py-2 text-sm font-medium text-slate-950 hover:bg-sky-400 disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="busy || !torrentStore.engineReady"
          @click="submit"
        >
          {{ busy ? "Adding..." : "Add torrent" }}
        </button>
        <button
          class="rounded-md bg-slate-700 px-4 py-2 text-sm text-slate-200 hover:bg-slate-600"
          @click="router.push('/downloads')"
        >
          Go to downloads
        </button>
      </div>

      <p class="text-xs text-slate-500">
        StreamNest does not index or recommend content. Only add torrents you are legally authorized to access.
      </p>
    </div>
  </div>
</template>
