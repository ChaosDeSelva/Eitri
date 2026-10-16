<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import Icon from '@/components/Icon.vue'
import PageHeader from '@/components/PageHeader.vue'
import Pill from '@/components/Pill.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { api, fileUrl, type Base } from '@/lib/api'
import { partialPlan } from '@/lib/partialPlan'

const app = useAppStore()
const router = useRouter()
const bases = ref<Base[]>([])
const search = ref('')
const showGenerate = ref(false)
const generating = ref(false)
const stream = ref<{ name: string; style_brief: string; shared_tail: string } | null>(null)
const gen = reactive({ look: '', size: '1024x1024', background: 'transparent', gate: 'figure' })
let unlisten: UnlistenFn | undefined

async function load() {
  bases.value = (await app.guard(() => api.bases())) ?? bases.value
}
onMounted(load)
onBeforeUnmount(() => unlisten?.())

const visible = computed(() => {
  const q = search.value.trim().toLowerCase()
  return bases.value.filter((b) => !q || `${b.name} ${b.description} ${b.style_brief}`.toLowerCase().includes(q))
})
const builtins = computed(() => visible.value.filter((b) => b.builtin))
const mine = computed(() => visible.value.filter((b) => !b.builtin))
const canPlan = computed(() => {
  const p = app.settings?.default_text_provider ?? 'openai'
  return !!app.keys[p]?.has_key || (p === 'compatible' && !!app.settings?.base_urls.compatible)
})

async function generate() {
  if (!gen.look.trim()) return app.notify('Describe the look first.', 'bad')
  generating.value = true
  stream.value = { name: '', style_brief: '', shared_tail: '' }
  unlisten = await listen<{ text: string }>('base-stream', (e) => {
    stream.value = partialPlan(e.payload.text)
  })
  const b = await app.guard(() => api.baseGenerate({ ...gen }))
  unlisten?.()
  unlisten = undefined
  generating.value = false
  stream.value = null
  if (b) {
    app.notify(`Saved "${b.name}" to your library.`)
    router.push({ name: 'base', params: { baseId: b.id } })
  }
}

async function blank() {
  const b = await app.guard(() =>
    api.baseCreate({
      name: 'Untitled base',
      description: '',
      style_brief: '',
      shared_tail: 'Framing: \nStyle: \nCanvas: 1024 by 1024 pixels.\nBackground: fully transparent.\nNo text, no letters, no logos, no watermarks.',
      size: '1024x1024',
      background: 'transparent',
      gate: 'figure',
    }),
  )
  if (b) router.push({ name: 'base', params: { baseId: b.id } })
}

async function importBase() {
  const path = await open({ multiple: false, filters: [{ name: 'Eitri base', extensions: ['json'] }] })
  if (typeof path !== 'string') return
  const b = await app.guard(() => api.baseImport(path))
  if (b) {
    app.notify(`Imported "${b.name}".`)
    load()
  }
}

function excerpt(tail: string) {
  return tail.split('\n').find((l) => l.startsWith('Style:'))?.replace(/^Style:\s*/, '') ?? tail.slice(0, 160)
}
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader title="Base library" subtitle="Reusable generic prompts. Pick one in a project and only write what is specific to each image.">
      <UiButton icon="folder" @click="importBase">Import</UiButton>
      <UiButton icon="plus" @click="blank">New base</UiButton>
      <UiButton variant="primary" icon="sparkles" @click="showGenerate = !showGenerate">Generate base</UiButton>
    </PageHeader>

    <div class="min-h-0 flex-1 overflow-y-auto p-6">
      <!-- Generate -->
      <section v-if="showGenerate" class="mb-6 rounded-xl border border-line bg-surface p-5">
        <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <div class="space-y-3">
            <div class="flex items-center gap-2 font-bold"><Icon name="sparkles" class="text-accent" /> Describe the look</div>
            <textarea
              v-model="gen.look"
              rows="4"
              class="field"
              placeholder="e.g. moody watercolour tarot cards with gold leaf borders, or chunky low-poly vehicles for a toy racing game"
            />
            <div class="grid grid-cols-3 gap-3">
              <label>
                <span class="label">Canvas</span>
                <select v-model="gen.size" class="field">
                  <option value="1024x1024">Square</option>
                  <option value="1536x1024">Landscape</option>
                  <option value="1024x1536">Portrait</option>
                </select>
              </label>
              <label>
                <span class="label">Background</span>
                <select v-model="gen.background" class="field">
                  <option value="transparent">Transparent</option>
                  <option value="opaque">Opaque</option>
                  <option value="auto">Model decides</option>
                </select>
              </label>
              <label>
                <span class="label">Composition</span>
                <select v-model="gen.gate" class="field">
                  <option value="figure">Free-standing</option>
                  <option value="portrait">Bust / portrait</option>
                  <option value="none">Anything</option>
                </select>
              </label>
            </div>
            <UiButton variant="primary" icon="sparkles" :loading="generating" :disabled="!canPlan" @click="generate">Generate and save</UiButton>
            <p v-if="!canPlan" class="text-[12px] text-bad">Add a key for your default planning provider in Settings.</p>
          </div>
          <div class="rounded-lg border border-line bg-surface-2 p-3">
            <div v-if="!stream" class="flex h-full items-center justify-center text-center text-[12px] text-muted">
              The planning model writes a style brief and a shared prompt. Nothing here describes a particular subject, so
              the base works for any image in a set.
            </div>
            <div v-else class="space-y-2">
              <div class="font-bold">{{ stream.name || 'Naming the look…' }}</div>
              <div class="text-[12.5px]">{{ stream.style_brief }}</div>
              <pre class="max-h-56 overflow-auto font-mono text-[11.5px] whitespace-pre-wrap text-muted">{{ stream.shared_tail }}<span class="inline-block h-3 w-1.5 animate-pulse bg-accent align-middle" /></pre>
            </div>
          </div>
        </div>
      </section>

      <div class="mb-4 flex items-center gap-3">
        <div class="relative w-72">
          <Icon name="search" :size="14" class="absolute top-1/2 left-2.5 -translate-y-1/2 text-faint" />
          <input v-model="search" class="field !pl-8" placeholder="Search bases" />
        </div>
      </div>

      <template v-for="group in [{ title: 'Your bases', list: mine }, { title: 'Built-in starters', list: builtins }]" :key="group.title">
        <h2 v-if="group.list.length || group.title === 'Your bases'" class="mt-2 mb-3 text-[12px] font-bold tracking-wide text-faint uppercase">
          {{ group.title }} <span class="font-semibold">({{ group.list.length }})</span>
        </h2>
        <div v-if="group.title === 'Your bases' && !group.list.length" class="mb-6 rounded-xl border border-dashed border-line p-5 text-muted">
          Nothing saved yet. Generate one, duplicate a starter, or use <b class="text-fg">Save as base</b> on a project whose look you like.
        </div>
        <div class="mb-6 grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
          <RouterLink
            v-for="b in group.list"
            :key="b.id"
            :to="{ name: 'base', params: { baseId: b.id } }"
            class="group flex flex-col rounded-xl border border-line bg-surface p-4 transition hover:border-line-strong hover:shadow-md"
          >
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <div class="truncate text-[14px] font-bold group-hover:text-accent">{{ b.name }}</div>
                <div class="line-clamp-2 text-[12px] text-muted">{{ b.description || b.style_brief }}</div>
              </div>
              <Pill v-if="b.builtin" tone="neutral">Built-in</Pill>
            </div>
            <p class="mt-3 line-clamp-3 font-mono text-[11px] text-faint">{{ excerpt(b.shared_tail) }}</p>
            <div class="mt-auto flex items-center gap-1.5 pt-3">
              <Pill tone="neutral">{{ b.size.replace('x', '×') }}</Pill>
              <Pill :tone="b.background === 'transparent' ? 'info' : 'neutral'">{{ b.background }}</Pill>
              <div class="flex-1" />
              <div v-if="b.references.length" class="flex -space-x-2">
                <div v-for="r in b.references.slice(0, 3)" :key="r.id" class="checker size-7 overflow-hidden rounded-md border-2 border-surface">
                  <img :src="fileUrl(r.file)" class="size-full object-contain" alt="" />
                </div>
              </div>
            </div>
          </RouterLink>
        </div>
      </template>
    </div>
  </div>
</template>
