<script setup>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const status = ref("checking backend...");

async function checkHealth() {
  try {
    const res = await invoke("health");
    status.value = `backend: ${res}`;
  } catch (err) {
    status.value = `backend error: ${err}`;
  }
}

checkHealth();
</script>

<template>
  <div class="flex h-full flex-col items-center justify-center gap-3">
    <h1 class="text-4xl font-bold tracking-tight text-sky-400">StreamNest</h1>
    <p class="text-sm text-slate-400">{{ status }}</p>
  </div>
</template>
