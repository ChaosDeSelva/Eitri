<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import Icon from '@/components/Icon.vue'
import Pill from '@/components/Pill.vue'
import ProgressBar from '@/components/ProgressBar.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { useProjectStore } from '@/stores/project'
import { api } from '@/lib/api'
import { runLabel, runTone, usd } from '@/lib/format'

const app = useAppStore()
const ps = useProjectStore()
const route = useRoute()
const router = useRouter()
const editingName = ref(false)
const nameDraft = ref('')
const busy = ref(false)

watch(() => route.params.id as string, (id) => id && ps.load(id), { immediate: true })

const tabs = computed(() => {
  const s = ps.detail?.stats
  return [
    { name: 'plan', label: 'Plan', icon: 'sparkles', count: s?.total || undefined },
    { name: 'run', label: 'Run', icon: 'layers', count: s ? s.queued + s.drawing || undefined : undefined },
    { name: 'review', label: 'Review', icon: 'eye', count: s ? s.done + s.flagged || undefined : undefined },
    { name: 'export', label: 'Export', icon: 'export', count: s?.approved || undefined },
    { name: 'activity', label: 'Activity', icon: 'activity' },
  ]
})

const state = computed(() => ps.run.state)
const reason = computed(() => (ps.run.state === 'stopped' ? ps.run.reason : ''))
const stats = computed(() => ps.detail?.stats)
const remaining = computed(() => (stats.value ? stats.value.draft + stats.value.queued : 0))

function startEditName() {
  nameDraft.value = ps.project?.name ?? ''
  editingName.value = true
}
async function saveName() {
  editingName.value = false
  if (nameDraft.value.trim() && nameDraft.value !== ps.project?.name) await ps.update({ name: nameDraft.value.trim() })
}

async function act(fn: () => Promise<unknown>) {
  busy.value = true
  await app.guard(fn)
  busy.value = false
  await ps.refresh()
}

const start = () => act(() => api.runStart(ps.id!))
const pause = () => act(() => api.runPause(ps.id!))
const stop = () => act(() => api.runStop(ps.id!))

async function remove() {
  if (!ps.project) return
  if (!window.confirm(`Delete "${ps.project.name}" and all of its images? This cannot be undone.`)) return
  const ok = await app.guard(() => api.projectDelete(ps.id!))
  if (ok !== undefined) {
    await app.refreshProjects()
    router.push('/')
  }
}
</script>

<template>
  <div v-if="ps.project && stats" class="flex h-full flex-col">
    <!-- Header -->
    <header class="border-b border-line bg-surface" data-tauri-drag-region>
      <div class="flex items-center gap-4 px-6 pt-3">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <input
              v-if="editingName"
              v-model="nameDraft"
              class="field max-w-md !py-1 text-[17px] font-extrabold"
              autofocus
              @blur="saveName"
              @keydown.enter="saveName"
              @keydown.esc="editingName = false"
            />
            <h1 v-else class="cursor-text truncate text-[17px] font-extrabold tracking-tight hover:text-accent" title="Rename" @click="startEditName">
              {{ ps.project.name }}
            </h1>
            <Pill :tone="runTone(ps.run)" dot :pulse="state === 'running'">{{ runLabel(ps.run) }}</Pill>
            <Pill v-if="ps.project.spec_locked" tone="accent"><Icon name="lock" :size="11" /> Spec approved</Pill>
          </div>
          <p v-if="reason" class="mt-0.5 truncate text-[12px] font-medium text-bad">{{ reason }}</p>
          <p v-else class="mt-0.5 truncate text-[12px] text-muted">{{ ps.project.idea || 'Describe an idea in Plan to get started.' }}</p>
        </div>

        <div class="flex shrink-0 items-center gap-2">
          <template v-if="state === 'running'">
            <UiButton icon="pause" :loading="busy" @click="pause">Pause</UiButton>
            <UiButton variant="ghost" icon="stop" @click="stop">Stop</UiButton>
          </template>
          <template v-else-if="state === 'paused'">
            <UiButton variant="primary" icon="play" :loading="busy" @click="start">Resume</UiButton>
            <UiButton variant="ghost" icon="stop" @click="stop">Stop</UiButton>
          </template>
          <UiButton v-else-if="state === 'stopping'" disabled loading>Stopping</UiButton>
          <UiButton
            v-else
            variant="primary"
            icon="play"
            :loading="busy"
            :disabled="!ps.project.spec_locked || remaining === 0"
            :title="!ps.project.spec_locked ? 'Approve the spec in Plan first' : ''"
            @click="start"
          >
            {{ remaining ? `Run ${remaining}` : 'Run' }}
          </UiButton>
          <button class="rounded-md p-1.5 text-faint hover:bg-bad-soft hover:text-bad" title="Delete project" @click="remove">
            <Icon name="trash" :size="15" />
          </button>
        </div>
      </div>

      <!-- Stats strip -->
      <div class="mt-3 flex items-center gap-6 px-6">
        <div class="flex-1"><ProgressBar :stats="stats" :height="7" /></div>
        <dl class="flex shrink-0 items-center gap-4 text-[12px] tabular-nums">
          <div class="flex items-center gap-1.5"><span class="size-2 rounded-full bg-ok" /><dt class="text-muted">Done</dt><dd class="font-bold">{{ stats.done + stats.approved }}</dd></div>
          <div class="flex items-center gap-1.5"><span class="size-2 rounded-full bg-warn" /><dt class="text-muted">Review</dt><dd class="font-bold">{{ stats.flagged }}</dd></div>
          <div class="flex items-center gap-1.5"><span class="size-2 rounded-full bg-bad" /><dt class="text-muted">Failed</dt><dd class="font-bold">{{ stats.failed }}</dd></div>
          <div class="flex items-center gap-1.5"><span class="size-2 rounded-full bg-line-strong" /><dt class="text-muted">Left</dt><dd class="font-bold">{{ remaining + stats.drawing }}</dd></div>
          <div class="flex items-center gap-1.5 border-l border-line pl-4">
            <dt class="text-muted">Spent</dt>
            <dd class="font-bold">{{ usd(stats.spent_usd) }}</dd>
            <dd v-if="ps.project.budget_usd > 0" class="text-faint">/ {{ usd(ps.project.budget_usd) }}</dd>
          </div>
        </dl>
      </div>

      <!-- Tabs -->
      <nav class="mt-2 flex gap-1 px-4">
        <RouterLink
          v-for="t in tabs"
          :key="t.name"
          :to="{ name: t.name, params: { id: ps.id } }"
          class="flex h-9 items-center gap-1.5 border-b-2 border-transparent px-3 font-semibold text-muted hover:text-fg"
          active-class="!border-accent !text-fg"
        >
          <Icon :name="t.icon" :size="14" />
          {{ t.label }}
          <span v-if="t.count" class="rounded-full bg-surface-3 px-1.5 text-[10px] font-bold tabular-nums">{{ t.count }}</span>
        </RouterLink>
      </nav>
    </header>

    <div class="min-h-0 flex-1 overflow-hidden">
      <RouterView />
    </div>
  </div>
  <div v-else class="flex h-full items-center justify-center text-faint">
    <span class="size-5 animate-spin rounded-full border-2 border-current border-t-transparent" />
  </div>
</template>
