<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import Icon from '@/components/Icon.vue'
import Pill from '@/components/Pill.vue'
import { useAppStore } from '@/stores/app'
import { api } from '@/lib/api'
import { runTone, usd } from '@/lib/format'

const app = useAppStore()
const route = useRoute()
const router = useRouter()

onMounted(() => app.start())

const activeProject = computed(() => (route.params.id as string | undefined) ?? null)

async function newProject() {
  if (!app.hasAnyKey) {
    app.notify('Add an API key in Settings first.', 'bad')
    router.push('/settings')
    return
  }
  const p = await app.guard(() => api.projectCreate('Untitled project'))
  if (!p) return
  await app.refreshProjects()
  router.push({ name: 'plan', params: { id: p.id } })
}

function cycleTheme() {
  app.setTheme(app.theme === 'system' ? 'dark' : app.theme === 'dark' ? 'light' : 'system')
}
</script>

<template>
  <div class="flex h-full">
    <!-- Sidebar -->
    <aside class="flex w-60 shrink-0 flex-col border-r border-line bg-surface-2">
      <div class="flex h-12 items-center gap-2 px-4" data-tauri-drag-region>
        <div class="flex size-7 items-center justify-center rounded-md bg-accent text-accent-fg">
          <span class="text-[16px] leading-none font-black">E</span>
        </div>
        <div class="leading-tight">
          <div class="text-[14px] font-extrabold tracking-tight">Eitri</div>
          <div class="text-[10px] font-medium text-faint">Image forge</div>
        </div>
      </div>

      <nav class="space-y-0.5 px-2 pt-2">
        <RouterLink
          to="/"
          class="flex h-8 items-center gap-2 rounded-md px-2 font-semibold text-muted hover:bg-surface-3 hover:text-fg"
          exact-active-class="!bg-surface-3 !text-fg"
        >
          <Icon name="grid" /> Projects
        </RouterLink>
        <RouterLink
          to="/settings"
          class="flex h-8 items-center gap-2 rounded-md px-2 font-semibold text-muted hover:bg-surface-3 hover:text-fg"
          active-class="!bg-surface-3 !text-fg"
        >
          <Icon name="settings" /> Settings
          <span v-if="!app.hasAnyKey" class="ml-auto size-2 rounded-full bg-accent" title="Add an API key" />
        </RouterLink>
      </nav>

      <div class="mt-4 flex items-center justify-between px-4 pb-1">
        <span class="text-[11px] font-bold tracking-wide text-faint uppercase">Projects</span>
        <button class="rounded p-0.5 text-muted hover:bg-surface-3 hover:text-fg" title="New project" @click="newProject">
          <Icon name="plus" :size="14" />
        </button>
      </div>
      <div class="min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-3">
        <RouterLink
          v-for="p in app.projects"
          :key="p.id"
          :to="{ name: 'run', params: { id: p.id } }"
          class="group flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-surface-3"
          :class="activeProject === p.id && 'bg-surface-3'"
        >
          <span
            class="size-2 shrink-0 rounded-full"
            :class="{
              'bg-info animate-pulse': runTone(app.runs[p.id]) === 'info',
              'bg-warn': runTone(app.runs[p.id]) === 'warn',
              'bg-bad': runTone(app.runs[p.id]) === 'bad',
              'bg-line-strong': runTone(app.runs[p.id]) === 'neutral',
            }"
          />
          <span class="min-w-0 flex-1 truncate font-medium" :class="activeProject === p.id ? 'text-fg' : 'text-muted group-hover:text-fg'">
            {{ p.name }}
          </span>
          <span class="text-[11px] tabular-nums text-faint">{{ p.stats.done + p.stats.approved }}/{{ p.stats.total }}</span>
        </RouterLink>
        <button
          v-if="!app.projects.length"
          class="w-full rounded-md border border-dashed border-line px-3 py-3 text-left text-muted hover:border-line-strong hover:text-fg"
          @click="newProject"
        >
          Start your first project
        </button>
      </div>

      <div class="border-t border-line p-3">
        <div class="mb-2 grid grid-cols-2 gap-2">
          <div class="rounded-md bg-surface px-2 py-1.5">
            <div class="text-[10px] font-semibold text-faint uppercase">Today</div>
            <div class="font-bold tabular-nums">{{ usd(app.spend.today_usd) }}</div>
          </div>
          <div class="rounded-md bg-surface px-2 py-1.5">
            <div class="text-[10px] font-semibold text-faint uppercase">All time</div>
            <div class="font-bold tabular-nums">{{ usd(app.spend.total_usd) }}</div>
          </div>
        </div>
        <div class="flex items-center justify-between">
          <RouterLink to="/settings">
            <Pill :tone="app.hasAnyKey ? 'ok' : 'accent'" dot>
              {{ app.hasAnyKey ? `${app.keyCount || 1} provider${app.keyCount > 1 ? 's' : ''} ready` : 'No API key' }}
            </Pill>
          </RouterLink>
          <button class="rounded p-1 text-muted hover:bg-surface-3 hover:text-fg" :title="`Theme: ${app.theme}`" @click="cycleTheme">
            <Icon :name="app.theme === 'dark' ? 'moon' : 'sun'" :size="15" />
          </button>
        </div>
      </div>
    </aside>

    <!-- Main -->
    <main class="min-w-0 flex-1 overflow-hidden">
      <RouterView />
    </main>

    <Transition
      enter-from-class="translate-y-2 opacity-0"
      leave-to-class="translate-y-2 opacity-0"
      enter-active-class="transition duration-200"
      leave-active-class="transition duration-200"
    >
      <div
        v-if="app.toast"
        class="fixed right-5 bottom-5 z-50 flex max-w-md items-start gap-2 rounded-lg border px-3.5 py-2.5 text-[13px] font-medium shadow-lg selectable"
        :class="app.toast.tone === 'bad' ? 'border-bad/40 bg-bad-soft text-bad' : 'border-line bg-surface text-fg'"
      >
        <Icon :name="app.toast.tone === 'bad' ? 'alert' : 'check'" :size="15" class="mt-0.5" />
        <span>{{ app.toast.text }}</span>
      </div>
    </Transition>
  </div>
</template>
