<script setup lang="ts">
import { computed } from 'vue'
import type { ProjectStats } from '@/lib/api'

// Stacked bar: approved, done, needs review, failed, drawing, queued, over the total.
const props = defineProps<{ stats: ProjectStats; height?: number }>()

const segments = computed(() => {
  const s = props.stats
  const total = Math.max(1, s.total - s.draft - s.rejected)
  return [
    { key: 'approved', n: s.approved, cls: 'bg-accent' },
    { key: 'done', n: s.done, cls: 'bg-ok' },
    { key: 'flagged', n: s.flagged, cls: 'bg-warn' },
    { key: 'failed', n: s.failed, cls: 'bg-bad' },
    { key: 'drawing', n: s.drawing, cls: 'bg-info animate-pulse' },
  ].map((x) => ({ ...x, pct: (x.n / total) * 100 }))
})
</script>

<template>
  <div class="flex w-full overflow-hidden rounded-full bg-surface-3" :style="{ height: `${height ?? 6}px` }">
    <div v-for="s in segments" :key="s.key" :class="s.cls" class="h-full transition-[width] duration-500" :style="{ width: `${s.pct}%` }" />
  </div>
</template>
