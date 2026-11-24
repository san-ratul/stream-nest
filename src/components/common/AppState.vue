<script setup>
defineProps({
  state: { type: String, required: true }, // loading | error | empty
  title: { type: String, required: true },
  message: { type: String, default: "" },
});
const emit = defineEmits(["action"]);
</script>

<template>
  <div class="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
    <div v-if="state === 'loading'" class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-sky-400"></div>
    <div v-else-if="state === 'error'" class="text-3xl text-red-400">⚠</div>
    <div v-else class="text-3xl text-slate-500">◈</div>
    <h3 class="text-base font-semibold" :class="state === 'error' ? 'text-red-300' : 'text-slate-200'">{{ title }}</h3>
    <p v-if="message" class="max-w-sm text-sm text-slate-400">{{ message }}</p>
    <slot />
  </div>
</template>
