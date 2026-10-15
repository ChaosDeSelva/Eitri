<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import Pill from '@/components/Pill.vue'
import { useAppStore } from '@/stores/app'
import { useProjectStore } from '@/stores/project'
import { api, type ActivityEvent } from '@/lib/api'
import { clock, type Tone } from '@/lib/format'

const app = useAppStore()
const ps = useProjectStore()
const events = ref<ActivityEvent[]>([])
let timer: ReturnType<typeof setInterval> | undefined

async function load() {
  if (!ps.id) return
  events.value = (await app.guard(() => api.events(ps.id!, 500))) ?? events.value
}

onMounted(() => {
  load()
  timer = setInterval(load, 3000)
})
onBeforeUnmount(() => clearInterval(timer))

const TONES: Record<string, Tone> = {
  run: 'info', limit: 'bad', error: 'bad', gate: 'warn', review: 'accent', plan: 'ok', export: 'ok', spec: 'neutral', reference: 'neutral', project: 'neutral',
}
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-4xl overflow-hidden rounded-xl border border-line bg-surface">
      <table class="w-full text-[12.5px]">
        <thead class="border-b border-line bg-surface-2 text-left text-[11px] text-faint">
          <tr>
            <th class="w-24 px-4 py-2 font-semibold">Time</th>
            <th class="w-24 px-2 py-2 font-semibold">Kind</th>
            <th class="px-2 py-2 font-semibold">Event</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="e in events" :key="e.id" class="border-b border-line last:border-0">
            <td class="px-4 py-2 font-mono text-[11px] text-faint tabular-nums">{{ clock(e.ts) }}</td>
            <td class="px-2 py-2"><Pill :tone="TONES[e.kind] ?? 'neutral'">{{ e.kind }}</Pill></td>
            <td class="px-2 py-2 selectable">{{ e.message }}</td>
          </tr>
          <tr v-if="!events.length"><td colspan="3" class="px-4 py-10 text-center text-muted">No activity yet.</td></tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
