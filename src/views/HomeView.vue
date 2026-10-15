<script setup lang="ts">
import { useRouter } from 'vue-router'
import Icon from '@/components/Icon.vue'
import PageHeader from '@/components/PageHeader.vue'
import Pill from '@/components/Pill.vue'
import ProgressBar from '@/components/ProgressBar.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { api, fileUrl } from '@/lib/api'
import { ago, runLabel, runTone, usd } from '@/lib/format'

const app = useAppStore()
const router = useRouter()

async function newProject() {
  if (!app.key.has_key) {
    router.push('/settings')
    return
  }
  const p = await app.guard(() => api.projectCreate('Untitled project'))
  if (!p) return
  await app.refreshProjects()
  router.push({ name: 'plan', params: { id: p.id } })
}
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader title="Projects" :subtitle="`${app.projects.length} project${app.projects.length === 1 ? '' : 's'}`">
      <UiButton variant="primary" icon="plus" @click="newProject">New project</UiButton>
    </PageHeader>

    <div class="min-h-0 flex-1 overflow-y-auto p-6">
      <!-- First run -->
      <div v-if="!app.projects.length" class="mx-auto mt-10 max-w-2xl">
        <div class="rounded-xl border border-line bg-surface p-8">
          <div class="mb-4 flex size-11 items-center justify-center rounded-lg bg-accent text-accent-fg">
            <span class="text-[24px] leading-none font-black">E</span>
          </div>
          <h2 class="text-[22px] font-extrabold tracking-tight">Forge a whole set of images from one idea</h2>
          <p class="mt-2 max-w-xl text-[14px] text-muted">
            Describe what you need. Eitri writes a plan with one shared style and a distinct prompt for every image,
            lets you preview a few, then draws the rest while you watch, pause, and redo anything with a note.
          </p>
          <ol class="mt-6 grid gap-3 sm:grid-cols-3">
            <li v-for="(s, i) in [
              ['key', 'Bring your own key', 'Stored in your system keychain. Never leaves this machine except to OpenAI.'],
              ['sparkles', 'Plan and preview', 'Draft a spec, tweak prompts, preview a few, lock the style.'],
              ['layers', 'Run and review', 'Live progress, budget cap, automatic checks, redo with notes.'],
            ]" :key="i" class="rounded-lg border border-line bg-surface-2 p-3">
              <Icon :name="s[0]" class="mb-2 text-accent" />
              <div class="font-bold">{{ s[1] }}</div>
              <div class="mt-0.5 text-[12px] text-muted">{{ s[2] }}</div>
            </li>
          </ol>
          <div class="mt-6 flex gap-2">
            <UiButton v-if="!app.key.has_key" variant="primary" icon="key" @click="router.push('/settings')">Add your API key</UiButton>
            <UiButton v-else variant="primary" icon="plus" @click="newProject">Start a project</UiButton>
          </div>
        </div>
      </div>

      <!-- Project cards -->
      <div v-else class="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">
        <RouterLink
          v-for="p in app.projects"
          :key="p.id"
          :to="{ name: p.stats.total ? 'run' : 'plan', params: { id: p.id } }"
          class="group overflow-hidden rounded-xl border border-line bg-surface transition hover:border-line-strong hover:shadow-md"
        >
          <div class="checker relative h-36 border-b border-line">
            <img v-if="p.cover" :src="fileUrl(p.cover)" class="size-full object-contain p-2" alt="" />
            <div v-else class="flex size-full items-center justify-center text-faint"><Icon name="image" :size="28" /></div>
            <div class="absolute top-2 right-2">
              <Pill :tone="runTone(app.runs[p.id])" dot :pulse="app.runs[p.id]?.state === 'running'">{{ runLabel(app.runs[p.id]) }}</Pill>
            </div>
          </div>
          <div class="space-y-2.5 p-3.5">
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <div class="truncate text-[14px] font-bold group-hover:text-accent">{{ p.name }}</div>
                <div class="truncate text-[12px] text-muted">{{ p.idea || 'No plan yet' }}</div>
              </div>
              <div class="text-right text-[12px] font-bold tabular-nums">{{ usd(p.stats.spent_usd) }}</div>
            </div>
            <ProgressBar :stats="p.stats" />
            <div class="flex items-center justify-between text-[11px] text-faint">
              <span class="tabular-nums">
                {{ p.stats.done + p.stats.approved }} of {{ p.stats.total }} done
                <template v-if="p.stats.flagged"> · <span class="text-warn">{{ p.stats.flagged }} to review</span></template>
              </span>
              <span>{{ ago(p.updated_at) }}</span>
            </div>
          </div>
        </RouterLink>
      </div>
    </div>
  </div>
</template>
