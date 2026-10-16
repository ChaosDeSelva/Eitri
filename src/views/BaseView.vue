<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { open, save } from '@tauri-apps/plugin-dialog'
import Icon from '@/components/Icon.vue'
import PageHeader from '@/components/PageHeader.vue'
import Pill from '@/components/Pill.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { api, fileUrl, type Base } from '@/lib/api'

const app = useAppStore()
const route = useRoute()
const router = useRouter()
const base = ref<Base | null>(null)
const form = reactive({ name: '', description: '', style_brief: '', shared_tail: '', size: '1024x1024', background: 'transparent', gate: 'figure' })
const saving = ref(false)

async function load() {
  const b = await app.guard(() => api.base(route.params.baseId as string))
  if (!b) return
  base.value = b
  Object.assign(form, {
    name: b.name, description: b.description, style_brief: b.style_brief, shared_tail: b.shared_tail,
    size: b.size, background: b.background, gate: b.gate,
  })
}
watch(() => route.params.baseId, (id) => id && load(), { immediate: true })

const readonly = computed(() => !!base.value?.builtin)
const dirty = computed(() => {
  const b = base.value
  return !!b && (Object.keys(form) as (keyof typeof form)[]).some((k) => form[k] !== b[k])
})

async function saveBase() {
  if (!base.value) return
  saving.value = true
  const b = await app.guard(() => api.baseUpdate(base.value!.id, { ...form }))
  saving.value = false
  if (b) {
    base.value = b
    app.notify('Base saved.')
  }
}

async function duplicate() {
  const b = await app.guard(() => api.baseDuplicate(base.value!.id))
  if (b) router.push({ name: 'base', params: { baseId: b.id } })
}

async function remove() {
  if (!base.value || !window.confirm(`Delete the base "${base.value.name}"? Projects that used it keep their own copy.`)) return
  const ok = await app.guard(() => api.baseDelete(base.value!.id))
  if (ok !== undefined) router.push('/library')
}

async function exportBase() {
  const name = (base.value?.name ?? 'base').toLowerCase().replace(/[^a-z0-9]+/g, '-')
  const path = await save({ defaultPath: `${name}.eitri-base.json`, filters: [{ name: 'Eitri base', extensions: ['json'] }] })
  if (!path) return
  await app.guard(() => api.baseExport(base.value!.id, path))
  app.notify('Base exported.')
}

async function addRef() {
  const path = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] })
  if (typeof path !== 'string') return
  if (await app.guard(() => api.baseRefAdd(base.value!.id, path))) load()
}

async function removeRef(id: string) {
  await app.guard(() => api.baseRefRemove(id))
  load()
}

async function startProject() {
  if (!app.hasAnyKey) {
    app.notify('Add an API key in Settings first.', 'bad')
    return
  }
  const p = await app.guard(() => api.projectCreate(`${base.value!.name} set`))
  if (!p) return
  await app.guard(() => api.baseApply(p.id, base.value!.id))
  await app.refreshProjects()
  router.push({ name: 'plan', params: { id: p.id } })
}
</script>

<template>
  <div v-if="base" class="flex h-full flex-col">
    <PageHeader :title="base.name" :subtitle="base.builtin ? 'Built-in starter. Duplicate it to make your own version.' : 'Your base'">
      <template #before>
        <RouterLink to="/library" class="rounded p-1 text-muted hover:bg-surface-3 hover:text-fg" title="Back to the library"><Icon name="left" /></RouterLink>
      </template>
      <template #badge><Pill v-if="base.builtin" tone="neutral">Built-in</Pill></template>
      <UiButton icon="export" @click="exportBase">Export</UiButton>
      <UiButton icon="layers" @click="duplicate">Duplicate</UiButton>
      <UiButton v-if="!readonly" variant="danger" icon="trash" @click="remove">Delete</UiButton>
      <UiButton variant="primary" icon="play" @click="startProject">New project from this base</UiButton>
    </PageHeader>

    <div class="min-h-0 flex-1 overflow-y-auto p-6">
      <div class="mx-auto grid max-w-5xl gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
        <section class="space-y-4 rounded-xl border border-line bg-surface p-5">
          <div class="grid gap-4 sm:grid-cols-2">
            <label class="block">
              <span class="label">Name</span>
              <input v-model="form.name" class="field" :disabled="readonly" />
            </label>
            <label class="block">
              <span class="label">Good for</span>
              <input v-model="form.description" class="field" :disabled="readonly" placeholder="What kinds of sets this suits" />
            </label>
          </div>
          <label class="block">
            <span class="label">Style brief (for people)</span>
            <textarea v-model="form.style_brief" rows="3" class="field" :disabled="readonly" />
          </label>
          <label class="block">
            <span class="label">Shared prompt (appended to every image)</span>
            <textarea v-model="form.shared_tail" rows="12" class="field font-mono text-[12px]" :disabled="readonly" />
            <span class="mt-1 block text-[12px] text-muted">
              Keep it generic: framing, style, canvas, background and things to avoid. Each image adds its own subject.
            </span>
          </label>
          <div class="grid gap-4 sm:grid-cols-3">
            <label>
              <span class="label">Canvas</span>
              <select v-model="form.size" class="field" :disabled="readonly">
                <option value="1024x1024">Square 1024</option>
                <option value="1536x1024">Landscape 1536×1024</option>
                <option value="1024x1536">Portrait 1024×1536</option>
              </select>
            </label>
            <label>
              <span class="label">Background</span>
              <select v-model="form.background" class="field" :disabled="readonly">
                <option value="transparent">Transparent</option>
                <option value="opaque">Opaque</option>
                <option value="auto">Model decides</option>
              </select>
            </label>
            <label>
              <span class="label">Composition check</span>
              <select v-model="form.gate" class="field" :disabled="readonly">
                <option value="figure">Free-standing subject</option>
                <option value="portrait">Bust / portrait</option>
                <option value="none">No pixel checks</option>
              </select>
            </label>
          </div>
          <div v-if="!readonly" class="flex justify-end">
            <UiButton variant="primary" icon="check" :loading="saving" :disabled="!dirty" @click="saveBase">Save base</UiButton>
          </div>
        </section>

        <aside class="space-y-4">
          <section class="rounded-xl border border-line bg-surface p-4">
            <div class="mb-1 flex items-center gap-2 font-bold"><Icon name="star" /> Style references</div>
            <p class="mb-3 text-[12px] text-muted">Copied into every project started from this base, so new images match them.</p>
            <div class="grid grid-cols-3 gap-2">
              <div v-for="r in base.references" :key="r.id" class="group relative">
                <div class="checker aspect-square overflow-hidden rounded-md border border-line">
                  <img :src="fileUrl(r.file)" class="size-full object-contain" alt="" />
                </div>
                <button v-if="!readonly" class="absolute -top-1.5 -right-1.5 hidden rounded-full bg-surface p-0.5 text-bad shadow group-hover:block" @click="removeRef(r.id)">
                  <Icon name="x" :size="12" />
                </button>
              </div>
              <button
                v-if="!readonly"
                class="flex aspect-square items-center justify-center rounded-md border border-dashed border-line text-faint hover:border-line-strong hover:text-fg"
                title="Add an image"
                @click="addRef"
              >
                <Icon name="plus" />
              </button>
            </div>
            <p v-if="readonly && !base.references.length" class="text-[12px] text-faint">Duplicate this base to add references.</p>
          </section>
          <section class="rounded-xl border border-line bg-surface p-4 text-[12px] text-muted">
            <div class="mb-1 font-bold text-fg">How bases are used</div>
            Picking a base in a project copies it in. The planner keeps this style and only writes the per-image
            descriptions. Editing the base later never changes a project that already used it.
          </section>
        </aside>
      </div>
    </div>
  </div>
</template>
