<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import Icon from '@/components/Icon.vue'
import StatusPill from '@/components/StatusPill.vue'
import Thumb from '@/components/Thumb.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { useProjectStore, type StatusFilter } from '@/stores/project'
import { api, type Item } from '@/lib/api'
import { ago, usd } from '@/lib/format'

const app = useAppStore()
const ps = useProjectStore()
const router = useRouter()
const view = ref<'grid' | 'list'>((localStorage.getItem('eitri.view') as 'grid' | 'list') || 'grid')

function setView(v: 'grid' | 'list') {
  view.value = v
  try { localStorage.setItem('eitri.view', v) } catch { /* ignore */ }
}

const filters = computed<{ key: StatusFilter; label: string; n: number }[]>(() => {
  const c = ps.counts
  const n = (...k: string[]) => k.reduce((a, s) => a + (c[s] ?? 0), 0)
  return [
    { key: 'all', label: 'All', n: ps.items.length },
    { key: 'active', label: 'In progress', n: n('queued', 'drawing') },
    { key: 'review', label: 'To review', n: n('done', 'flagged') },
    { key: 'problems', label: 'Problems', n: n('failed', 'refused', 'flagged') },
    { key: 'approved', label: 'Approved', n: n('approved') },
    { key: 'draft', label: 'Not started', n: n('draft') },
  ]
})

function openItem(item: Item) {
  router.push({ name: 'review', params: { id: ps.id }, query: { item: item.id } })
}

async function setConcurrency(delta: number) {
  const n = Math.min(8, Math.max(1, (ps.project?.concurrency ?? 3) + delta))
  await ps.update({ concurrency: n })
}

async function queueSelected() {
  const ids = [...ps.selected]
  if (!ids.length) return
  await app.guard(() => api.runPreview(ps.id!, ids))
  ps.selected = new Set()
  ps.refresh()
}

async function retryFailed() {
  const ids = ps.items.filter((i) => i.status === 'failed' || i.status === 'refused').map((i) => i.id)
  if (!ids.length) return
  await app.guard(() => api.runPreview(ps.id!, ids))
  ps.refresh()
}
</script>

<template>
  <div v-if="ps.project" class="flex h-full flex-col">
    <!-- Toolbar -->
    <div class="flex items-center gap-2 border-b border-line bg-surface-2 px-6 py-2.5">
      <div class="flex gap-1">
        <button
          v-for="f in filters"
          :key="f.key"
          class="flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[12px] font-semibold"
          :class="ps.filter === f.key ? 'bg-surface text-fg shadow-[0_0_0_1px_var(--border)]' : 'text-muted hover:text-fg'"
          @click="ps.filter = f.key"
        >
          {{ f.label }} <span class="tabular-nums text-faint">{{ f.n }}</span>
        </button>
      </div>
      <div class="relative ml-2 w-56">
        <Icon name="search" :size="14" class="absolute top-1/2 left-2.5 -translate-y-1/2 text-faint" />
        <input v-model="ps.search" class="field !h-7 !py-0 !pl-8 text-[12px]" placeholder="Search names and prompts" />
      </div>
      <div class="flex-1" />
      <UiButton v-if="ps.selected.size" size="sm" icon="play" @click="queueSelected">Draw {{ ps.selected.size }} selected</UiButton>
      <UiButton v-if="(ps.counts.failed ?? 0) + (ps.counts.refused ?? 0)" size="sm" icon="redo" @click="retryFailed">Retry failed</UiButton>
      <div class="flex h-7 items-center gap-1 rounded-md border border-line bg-surface px-1 text-[12px]" title="Images drawn at once">
        <button class="rounded px-1.5 text-muted hover:text-fg" @click="setConcurrency(-1)">−</button>
        <span class="w-14 text-center font-semibold tabular-nums">{{ ps.project.concurrency }} at once</span>
        <button class="rounded px-1.5 text-muted hover:text-fg" @click="setConcurrency(1)">+</button>
      </div>
      <div class="flex rounded-md border border-line bg-surface p-0.5">
        <button class="rounded p-1" :class="view === 'grid' ? 'bg-surface-3 text-fg' : 'text-faint'" @click="setView('grid')"><Icon name="grid" :size="14" /></button>
        <button class="rounded p-1" :class="view === 'list' ? 'bg-surface-3 text-fg' : 'text-faint'" @click="setView('list')"><Icon name="list" :size="14" /></button>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto p-6">
      <div v-if="!ps.items.length" class="mx-auto mt-16 max-w-sm text-center text-muted">
        <Icon name="layers" :size="28" class="mx-auto mb-3" />
        <div class="text-[15px] font-bold text-fg">Nothing to run yet</div>
        <p class="mt-1">Draft a plan first, then approve it and press Run.</p>
        <UiButton class="mt-4" icon="sparkles" @click="router.push({ name: 'plan', params: { id: ps.id } })">Go to Plan</UiButton>
      </div>
      <div v-else-if="!ps.visible.length" class="mt-16 text-center text-muted">No images match this filter.</div>

      <!-- Grid -->
      <div v-else-if="view === 'grid'" class="grid grid-cols-[repeat(auto-fill,minmax(168px,1fr))] gap-3">
        <div
          v-for="it in ps.visible"
          :key="it.id"
          class="group relative cursor-pointer overflow-hidden rounded-lg border bg-surface p-1.5 transition hover:shadow-md"
          :class="ps.selected.has(it.id) ? 'border-accent ring-2 ring-accent-soft' : 'border-line hover:border-line-strong'"
          @click="openItem(it)"
        >
          <Thumb :file="it.file" :status="it.status" :alt="it.title" />
          <input
            type="checkbox"
            class="absolute top-3 left-3 hidden group-hover:block"
            :class="ps.selected.has(it.id) && '!block'"
            :checked="ps.selected.has(it.id)"
            @click.stop
            @change="ps.toggle(it.id)"
          />
          <div class="px-1 pt-2 pb-0.5">
            <div class="flex items-center justify-between gap-1">
              <div class="truncate text-[12px] font-semibold">{{ it.title || it.slug }}</div>
              <span v-if="it.attempts > 1" class="shrink-0 text-[10px] font-bold text-faint">×{{ it.attempts }}</span>
            </div>
            <div class="mt-1 flex items-center justify-between">
              <StatusPill :status="it.status" />
              <span class="text-[11px] tabular-nums text-faint">{{ it.cost ? usd(it.cost) : '' }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- List -->
      <div v-else class="overflow-hidden rounded-xl border border-line bg-surface">
        <table class="w-full text-[12.5px]">
          <thead class="border-b border-line bg-surface-2 text-left text-[11px] text-faint">
            <tr>
              <th class="w-9 px-3 py-2" />
              <th class="w-14 py-2" />
              <th class="px-2 py-2 font-semibold">Name</th>
              <th class="w-32 px-2 py-2 font-semibold">Status</th>
              <th class="px-2 py-2 font-semibold">Notes</th>
              <th class="w-16 px-2 py-2 text-right font-semibold">Tries</th>
              <th class="w-20 px-2 py-2 text-right font-semibold">Cost</th>
              <th class="w-24 px-3 py-2 text-right font-semibold">Updated</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="it in ps.visible" :key="it.id" class="cursor-pointer border-b border-line last:border-0 hover:bg-surface-2" @click="openItem(it)">
              <td class="px-3"><input type="checkbox" :checked="ps.selected.has(it.id)" @click.stop @change="ps.toggle(it.id)" /></td>
              <td class="py-1.5"><div class="w-11"><Thumb :file="it.file" :status="it.status" /></div></td>
              <td class="px-2">
                <div class="font-semibold">{{ it.title || it.slug }}</div>
                <div class="font-mono text-[10.5px] text-faint">{{ it.slug }}</div>
              </td>
              <td class="px-2"><StatusPill :status="it.status" /></td>
              <td class="max-w-0 truncate px-2 text-muted" :class="(it.status === 'failed' || it.status === 'refused') && 'text-bad'">
                {{ it.last_error || it.reasons.join(' ') || it.redo_note || '' }}
              </td>
              <td class="px-2 text-right tabular-nums">{{ it.attempts }}</td>
              <td class="px-2 text-right tabular-nums">{{ usd(it.cost) }}</td>
              <td class="px-3 text-right text-faint">{{ ago(it.updated_at) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
