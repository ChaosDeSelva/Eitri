<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { partialPlan, type PartialPlan } from '@/lib/partialPlan'
import Icon from '@/components/Icon.vue'
import StatusPill from '@/components/StatusPill.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { useProjectStore } from '@/stores/project'
import { api, fileUrl, type Item } from '@/lib/api'
import { estimatePerImage, usd } from '@/lib/format'

const app = useAppStore()
const ps = useProjectStore()
const drafting = ref(false)
const newTitle = ref('')
const newSubject = ref('')

const form = reactive({ idea: '', count: 12, size: '1024x1024', background: 'transparent', gate: 'figure', notes: '' })
const meta = reactive({ style_brief: '', shared_tail: '' })

watch(
  () => ps.project?.id,
  () => {
    const p = ps.project
    if (!p) return
    form.idea = p.idea
    form.size = p.size
    form.background = p.background
    form.gate = p.gate === 'none' && !p.idea ? 'figure' : p.gate
    meta.style_brief = p.style_brief
    meta.shared_tail = p.shared_tail
  },
  { immediate: true },
)

const locked = computed(() => !!ps.project?.spec_locked)
const refs = computed(() => ps.detail?.references ?? [])
const perImage = computed(() => estimatePerImage(ps.project?.quality ?? 'medium', refs.value.length > 0 && !!ps.project?.use_references))
const estimate = computed(() => perImage.value * Math.max(ps.items.length, 0))
const selectedIds = computed(() => [...ps.selected].filter((id) => ps.items.some((i) => i.id === id)))
const allSelected = computed(() => ps.items.length > 0 && selectedIds.value.length === ps.items.length)

const stream = ref<PartialPlan | null>(null)
let unlisten: UnlistenFn | undefined
onBeforeUnmount(() => unlisten?.())

async function draft() {
  if (!form.idea.trim()) return app.notify('Describe the idea first.', 'bad')
  if (ps.items.some((i) => i.attempts > 0) && !window.confirm('Re-plan? Images you have already drawn are kept; unstarted ones are replaced.')) return
  drafting.value = true
  stream.value = { name: '', style_brief: '', shared_tail: '', items: [] }
  const pid = ps.id
  unlisten = await listen<{ project_id: string; text: string }>('plan-stream', (e) => {
    if (e.payload.project_id === pid) stream.value = partialPlan(e.payload.text)
  })
  const p = await app.guard(() => api.planDraft(ps.id!, { ...form, count: Number(form.count) || 1 }))
  unlisten?.()
  unlisten = undefined
  drafting.value = false
  stream.value = null
  if (p) {
    meta.style_brief = p.style_brief
    meta.shared_tail = p.shared_tail
    await ps.refresh()
    app.refreshProjects()
    app.notify('Plan drafted. Edit anything, preview a few, then approve.')
  }
}

async function saveMeta() {
  if (!ps.project) return
  if (meta.style_brief === ps.project.style_brief && meta.shared_tail === ps.project.shared_tail) return
  await ps.update({ style_brief: meta.style_brief, shared_tail: meta.shared_tail })
}

async function saveItem(item: Item, field: 'title' | 'subject', value: string) {
  if (value === item[field]) return
  ps.replaceItem(await app.guard(() => (field === 'title' ? api.itemEdit(item.id, value, undefined) : api.itemEdit(item.id, undefined, value))))
}

async function removeItem(item: Item) {
  await app.guard(() => api.itemDelete(item.id))
  await ps.refresh()
}

async function addItem() {
  if (!newSubject.value.trim()) return
  const it = await app.guard(() => api.itemAdd(ps.id!, newTitle.value, newSubject.value))
  if (it) {
    newTitle.value = ''
    newSubject.value = ''
    await ps.refresh()
  }
}

async function preview() {
  const ids = selectedIds.value.length ? selectedIds.value : ps.items.slice(0, 2).map((i) => i.id)
  if (!ids.length) return
  await app.guard(() => api.runPreview(ps.id!, ids))
  ps.selected = new Set()
  await ps.refresh()
  app.notify(`Previewing ${ids.length} image${ids.length === 1 ? '' : 's'}. Watch them in Run.`)
}

async function lock(value: boolean) {
  await saveMeta()
  const p = await app.guard(() => api.specLock(ps.id!, value))
  if (p) await ps.refresh()
}

function toggleAll() {
  ps.selected = allSelected.value ? new Set() : new Set(ps.items.map((i) => i.id))
}

async function importRef() {
  const path = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] })
  if (typeof path !== 'string') return
  await app.guard(() => api.referenceImport(ps.id!, path))
  await ps.refresh()
}

async function removeRef(id: string) {
  await app.guard(() => api.referenceRemove(id))
  await ps.refresh()
}
</script>

<template>
  <div v-if="ps.project" class="flex h-full">
    <!-- Brief -->
    <aside class="w-[340px] shrink-0 overflow-y-auto border-r border-line bg-surface p-5">
      <h2 class="mb-3 flex items-center gap-2 text-[13px] font-bold"><Icon name="sparkles" class="text-accent" /> Idea</h2>
      <div class="space-y-3">
        <label class="block">
          <span class="label">What should this set be?</span>
          <textarea
            v-model="form.idea"
            rows="4"
            class="field"
            :disabled="locked"
            placeholder="e.g. 40 fantasy potion bottles for an inventory screen, chunky hand-painted style"
          />
        </label>
        <div class="grid grid-cols-2 gap-3">
          <label>
            <span class="label">Images</span>
            <input v-model.number="form.count" type="number" min="1" max="500" class="field" :disabled="locked" />
          </label>
          <label>
            <span class="label">Canvas</span>
            <select v-model="form.size" class="field" :disabled="locked">
              <option value="1024x1024">Square 1024</option>
              <option value="1536x1024">Landscape 1536×1024</option>
              <option value="1024x1536">Portrait 1024×1536</option>
            </select>
          </label>
          <label>
            <span class="label">Background</span>
            <select v-model="form.background" class="field" :disabled="locked">
              <option value="transparent">Transparent</option>
              <option value="opaque">Opaque</option>
              <option value="auto">Model decides</option>
            </select>
          </label>
          <label>
            <span class="label">Composition check</span>
            <select v-model="form.gate" class="field" :disabled="locked">
              <option value="figure">Free-standing subject</option>
              <option value="portrait">Bust / portrait</option>
              <option value="none">No pixel checks</option>
            </select>
          </label>
        </div>
        <label class="block">
          <span class="label">Extra direction (optional)</span>
          <textarea v-model="form.notes" rows="2" class="field" :disabled="locked" placeholder="Audience, palette, must-haves, things to avoid" />
        </label>
        <UiButton variant="primary" icon="sparkles" class="w-full" :loading="drafting" :disabled="locked || !app.key.has_key" @click="draft">
          {{ ps.items.length ? 'Re-draft plan' : 'Draft plan' }}
        </UiButton>
        <p v-if="!app.key.has_key" class="text-[12px] text-bad">Add your API key in Settings to plan and draw.</p>
      </div>

      <h2 class="mt-7 mb-3 flex items-center gap-2 text-[13px] font-bold"><Icon name="settings" /> Models and quality</h2>
      <div class="space-y-3">
        <label class="block">
          <span class="label">Image model</span>
          <input :value="ps.project.image_model" class="field font-mono" @change="ps.update({ image_model: ($event.target as HTMLInputElement).value })" />
        </label>
        <label class="block">
          <span class="label">Planning model</span>
          <input :value="ps.project.text_model" class="field font-mono" @change="ps.update({ text_model: ($event.target as HTMLInputElement).value })" />
        </label>
        <div class="grid grid-cols-2 gap-3">
          <label>
            <span class="label">Quality</span>
            <select :value="ps.project.quality" class="field" @change="ps.update({ quality: ($event.target as HTMLSelectElement).value })">
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
            </select>
          </label>
          <label>
            <span class="label">Budget (USD)</span>
            <input :value="ps.project.budget_usd" type="number" min="0" step="0.5" class="field" @change="ps.update({ budget_usd: Number(($event.target as HTMLInputElement).value) })" />
          </label>
        </div>
        <div class="rounded-lg bg-surface-2 p-3 text-[12px]">
          <div class="flex justify-between"><span class="text-muted">Estimate per image</span><span class="font-bold tabular-nums">{{ usd(perImage) }}</span></div>
          <div class="mt-1 flex justify-between"><span class="text-muted">Whole plan ({{ ps.items.length }})</span><span class="font-bold tabular-nums">{{ usd(estimate) }}</span></div>
          <p class="mt-1.5 text-faint">Real spend is computed from each response's token usage.</p>
        </div>
      </div>

      <h2 class="mt-7 mb-1 flex items-center gap-2 text-[13px] font-bold"><Icon name="star" /> Style references</h2>
      <p class="mb-3 text-[12px] text-muted">
        Approved previews or your own images. Every new image is drawn to match them, which keeps a big set consistent.
      </p>
      <div class="grid grid-cols-4 gap-2">
        <div v-for="r in refs" :key="r.id" class="group relative">
          <div class="checker aspect-square overflow-hidden rounded-md border border-line">
            <img :src="fileUrl(r.file)" class="size-full object-contain" alt="" />
          </div>
          <button class="absolute -top-1.5 -right-1.5 hidden rounded-full bg-surface p-0.5 text-bad shadow group-hover:block" @click="removeRef(r.id)">
            <Icon name="x" :size="12" />
          </button>
        </div>
        <button class="flex aspect-square items-center justify-center rounded-md border border-dashed border-line text-faint hover:border-line-strong hover:text-fg" title="Import an image" @click="importRef">
          <Icon name="plus" />
        </button>
      </div>
      <label class="mt-3 flex items-center gap-2 text-[12px]">
        <input type="checkbox" :checked="ps.project.use_references" @change="ps.update({ use_references: ($event.target as HTMLInputElement).checked })" />
        Use references when drawing
      </label>
    </aside>

    <!-- Live draft -->
    <section v-if="drafting && stream" class="min-w-0 flex-1 overflow-y-auto p-6">
      <div class="mb-4 flex items-center gap-3">
        <span class="relative flex size-2.5">
          <span class="absolute inline-flex size-full animate-ping rounded-full bg-accent opacity-60" />
          <span class="relative inline-flex size-2.5 rounded-full bg-accent" />
        </span>
        <div class="font-bold">
          Writing the plan
          <span class="font-medium text-muted">
            · {{ stream.items.length }} of {{ form.count }} images
          </span>
        </div>
        <div class="h-1.5 w-48 overflow-hidden rounded-full bg-surface-3">
          <div class="h-full rounded-full bg-accent transition-[width] duration-300" :style="{ width: `${Math.min(100, (stream.items.length / Math.max(1, form.count)) * 100)}%` }" />
        </div>
      </div>

      <div class="mb-5 grid gap-4 xl:grid-cols-2">
        <div>
          <span class="label">Style brief</span>
          <div class="min-h-24 rounded-md border border-line bg-surface p-2.5 whitespace-pre-wrap">
            {{ stream.style_brief }}<span v-if="!stream.shared_tail" class="ml-0.5 inline-block h-3.5 w-1.5 animate-pulse bg-accent align-middle" />
          </div>
        </div>
        <div>
          <span class="label">Shared prompt</span>
          <div class="min-h-24 rounded-md border border-line bg-surface p-2.5 font-mono text-[12px] whitespace-pre-wrap text-muted">
            {{ stream.shared_tail }}<span v-if="stream.style_brief && !stream.items.length" class="ml-0.5 inline-block h-3.5 w-1.5 animate-pulse bg-accent align-middle" />
          </div>
        </div>
      </div>

      <TransitionGroup tag="div" class="space-y-2" enter-from-class="opacity-0 -translate-y-1" enter-active-class="transition duration-300">
        <div v-for="(it, i) in stream.items" :key="i + it.slug" class="flex gap-3 rounded-lg border border-line bg-surface p-3">
          <span class="w-6 shrink-0 pt-0.5 text-right text-[11px] font-bold text-faint tabular-nums">{{ i + 1 }}</span>
          <div class="min-w-0">
            <div class="font-semibold">{{ it.title }} <span class="font-mono text-[10.5px] font-normal text-faint">{{ it.slug }}</span></div>
            <div class="mt-0.5 text-[12.5px] text-muted">{{ it.subject }}</div>
          </div>
        </div>
      </TransitionGroup>
      <div v-if="stream.items.length < form.count" class="mt-2 flex items-center gap-2 rounded-lg border border-dashed border-line p-3 text-muted">
        <span class="size-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" />
        {{ stream.items.length ? 'Writing the next image…' : stream.shared_tail ? 'Writing the images…' : 'Setting the style…' }}
      </div>
    </section>

    <!-- Spec -->
    <section v-else class="min-w-0 flex-1 overflow-y-auto p-6">
      <div v-if="!ps.items.length && !meta.shared_tail" class="mx-auto mt-16 max-w-md text-center text-muted">
        <Icon name="sparkles" :size="28" class="mx-auto mb-3 text-accent" />
        <div class="text-[15px] font-bold text-fg">No plan yet</div>
        <p class="mt-1">Describe the idea on the left and draft a plan. You can also add images by hand below.</p>
      </div>

      <div v-if="meta.shared_tail || ps.items.length" class="mb-5 grid gap-4 xl:grid-cols-2">
        <label class="block">
          <span class="label">Style brief</span>
          <textarea v-model="meta.style_brief" rows="5" class="field" :disabled="locked" @blur="saveMeta" />
        </label>
        <label class="block">
          <span class="label">Shared prompt (appended to every image)</span>
          <textarea v-model="meta.shared_tail" rows="5" class="field font-mono text-[12px]" :disabled="locked" @blur="saveMeta" />
        </label>
      </div>

      <div class="mb-3 flex items-center gap-2">
        <h2 class="text-[14px] font-bold">Images <span class="font-medium text-faint">({{ ps.items.length }})</span></h2>
        <div class="flex-1" />
        <UiButton size="sm" icon="eye" :disabled="!ps.items.length" @click="preview">
          Preview {{ selectedIds.length ? `${selectedIds.length} selected` : 'first 2' }}
        </UiButton>
        <UiButton v-if="!locked" size="sm" variant="primary" icon="lock" :disabled="!ps.items.length" @click="lock(true)">Approve spec</UiButton>
        <UiButton v-else size="sm" icon="unlock" @click="lock(false)">Unlock to edit</UiButton>
      </div>

      <div class="overflow-hidden rounded-xl border border-line bg-surface">
        <table class="w-full table-fixed text-[12.5px]">
          <thead class="border-b border-line bg-surface-2 text-left text-[11px] text-faint">
            <tr>
              <th class="w-9 px-3 py-2"><input type="checkbox" :checked="allSelected" @change="toggleAll" /></th>
              <th class="w-44 px-2 py-2 font-semibold">Name</th>
              <th class="px-2 py-2 font-semibold">Description</th>
              <th class="w-28 px-2 py-2 font-semibold">Status</th>
              <th class="w-9" />
            </tr>
          </thead>
          <tbody>
            <tr v-for="it in ps.items" :key="it.id" class="border-b border-line align-top last:border-0 hover:bg-surface-2">
              <td class="px-3 py-2.5"><input type="checkbox" :checked="ps.selected.has(it.id)" @change="ps.toggle(it.id)" /></td>
              <td class="px-2 py-2">
                <input
                  :value="it.title"
                  class="w-full rounded bg-transparent px-1 py-0.5 font-semibold outline-none focus:bg-surface-3"
                  @change="saveItem(it, 'title', ($event.target as HTMLInputElement).value)"
                />
                <div class="truncate px-1 font-mono text-[10.5px] text-faint">{{ it.slug }}</div>
              </td>
              <td class="px-2 py-2">
                <textarea
                  :value="it.subject"
                  rows="2"
                  :disabled="locked"
                  class="w-full resize-y rounded bg-transparent px-1 py-0.5 outline-none focus:bg-surface-3 disabled:text-muted"
                  @change="saveItem(it, 'subject', ($event.target as HTMLTextAreaElement).value)"
                />
              </td>
              <td class="px-2 py-2.5"><StatusPill :status="it.status" /></td>
              <td class="py-2.5 pr-3 text-right">
                <button v-if="!locked && it.status !== 'drawing'" class="text-faint hover:text-bad" title="Remove" @click="removeItem(it)"><Icon name="trash" :size="14" /></button>
              </td>
            </tr>
            <tr class="bg-surface-2">
              <td />
              <td class="px-2 py-2"><input v-model="newTitle" class="field !py-1 text-[12px]" placeholder="Name" /></td>
              <td class="px-2 py-2"><input v-model="newSubject" class="field !py-1 text-[12px]" placeholder="Describe one more image" @keydown.enter="addItem" /></td>
              <td class="px-2 py-2" colspan="2"><UiButton size="sm" icon="plus" :disabled="!newSubject.trim()" @click="addItem">Add</UiButton></td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>
