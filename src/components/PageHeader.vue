<script setup lang="ts">
defineProps<{ title: string; subtitle?: string }>()
</script>

<template>
  <header class="flex min-h-14 items-center gap-4 border-b border-line bg-surface px-6 py-2.5" data-tauri-drag-region>
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2">
        <slot name="before" />
        <h1 class="truncate text-[17px] font-extrabold tracking-tight">{{ title }}</h1>
        <slot name="badge" />
      </div>
      <p v-if="subtitle" class="truncate text-[12px] text-muted">{{ subtitle }}</p>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <slot />
    </div>
  </header>
</template>
