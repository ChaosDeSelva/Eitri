<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import Icon from '@/components/Icon.vue'
import Pill from '@/components/Pill.vue'
import StatusPill from '@/components/StatusPill.vue'
import Thumb from '@/components/Thumb.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { useProjectStore } from '@/stores/project'
import { api, fileUrl, type Attempt, type Item } from '@/lib/api'
import { ago, usd } from '@/lib/format'

const app = useAppStore()
const ps = useProjectStore()
const route = useRoute()
const router = useRouter()

const scope = ref<'review' | 'all'>('review')
const attempts = ref<Attempt[]>([])
const shownAttempt = ref<string | null>(null)
const note = ref('')
const editCurrent = ref(true)
const noteBox = ref<HTMLTextAreaElement | null>(null)
const showPrompt = ref(false)

// Items needing a decision come first: flagged, then done, then everything else with an image.
const order: Record<string, number> = { flagged: 0, done: 1, drawing: 2, queued: 3, failed: 4, refused: 4, approved: 5, rejected: 6, draft: 7 }
const queue = computed(() => {
  const list = scope.value === 'review'
    ? ps.items.filter((i) => i.status === 'done' || i.status === 'flagged')
    : ps.items.filter((i) => i.status !== 'draft')
  return [...list].sort((a, b) => (order[a.status] ?? 9) - (order[b.status] ?? 9) || a.ord - b.ord)
})

const currentId = computed(() => (route.query.item as string | undefined) ?? queue.value[0]?.id ?? null)
const current = computed<Item | undefined>(() => ps.items.find((i) => i.id === currentId.value))
const index = computed(() => queue.value.findIndex((i) => i.id === currentId.value))
const attempt = computed(() => attempts.value.find((a) => a.id === shownAttempt.value) ?? attempts.value[0])

watch(
  () => [currentId.value, current.value?.attempts, current.value?.current_attempt],
  async () => {
    const id = currentId.value
    if (!id) return (attempts.value = [])
    const list = await app.guard(() => api.attempts(id))
    if (id !== currentId.value || !list) return
    attempts.value = list
    shownAttempt.value = current.value?.approved_attempt ?? current.value?.current_attempt ?? list[0]?.id ?? null
  },
  { immediate: true },
)

watch(currentId, () => {
  note.value = ''
  showPrompt.value = false
})

function go(item: Item | undefined) {
  if (item) router.replace({ name: 'review', params: { id: ps.id }, query: { item: item.id } })
}
function step(d: number) {
  if (!queue.value.length) return
  const i = index.value < 0 ? 0 : Math.min(queue.value.length - 1, Math.max(0, index.value + d))
  go(queue.value[i])
}
/** After a decision in review mode the item leaves the queue, so the next one slides in. */
function advanceFrom(id: string) {
  const i = queue.value.findIndex((x) => x.id === id)
  const next = queue.value[i + 1] ?? queue.value[i - 1]
  if (next && next.id !== id) go(next)
}

async function approve() {
  const it = current.value
  if (!it || !attempt.value?.file) return
  const updated = await app.guard(() => api.itemApprove(it.id, attempt.value!.id))
  ps.replaceItem(updated)
  ps.softRefresh()
  if (scope.value === 'review') advanceFrom(it.id)
}

async function reject() {
  const it = current.value
  if (!it) return
  ps.replaceItem(await app.guard(() => api.itemReject(it.id)))
  ps.softRefresh()
  if (scope.value === 'review') advanceFrom(it.id)
}

async function redo() {
  const it = current.value
  if (!it) return
  const updated = await app.guard(() => api.itemRedo(it.id, note.value || null, editCurrent.value && !!note.value))
  if (updated) {
    ps.replaceItem(updated)
    app.notify(note.value ? 'Redo queued with your note.' : 'Redraw queued.')
    note.value = ''
    if (scope.value === 'review') advanceFrom(it.id)
  }
}

async function useAsReference() {
  if (!attempt.value?.file) return
  const r = await app.guard(() => api.referenceFromAttempt(attempt.value!.id))
  if (r) {
    app.notify('Added as a style reference. New images will match it.')
    ps.refresh()
  }
}

function onKey(e: KeyboardEvent) {
  const t = e.target as HTMLElement
  if (t && (t.tagName === 'TEXTAREA' || t.tagName === 'INPUT')) {
    if (e.key === 'Escape') t.blur()
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) redo()
    return
  }
  if (e.key === 'ArrowRight' || e.key === 'j') step(1)
  else if (e.key === 'ArrowLeft' || e.key === 'k') step(-1)
  else if (e.key === 'a') approve()
  else if (e.key === 'r') reject()
  else if (e.key === 'f') {
    e.preventDefault()
    noteBox.value?.focus()
  } else return
  e.preventDefault()
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

const gate = computed(() => attempt.value?.gate ?? null)
const verdictTone = computed(() => (gate.value?.verdict === 'pass' ? 'ok' : gate.value?.verdict === 'flag' ? 'warn' : 'bad'))
</script>

<template>
  <div class="flex h-full">
    <!-- Filmstrip -->
    <aside class="flex w-56 shrink-0 flex-col border-r border-line bg-surface-2">
      <div class="flex gap-1 p-2">
        <button
          v-for="s in (['review', 'all'] as const)"
          :key="s"
          class="h-7 flex-1 rounded-md text-[12px] font-semibold"
          :class="scope === s ? 'bg-surface text-fg shadow-[0_0_0_1px_var(--border)]' : 'text-muted hover:text-fg'"
          @click="scope = s"
        >
          {{ s === 'review' ? `To review (${ps.items.filter((i) => i.status === 'done' || i.status === 'flagged').length})` : 'All' }}
        </button>
      </div>
      <div class="min-h-0 flex-1 space-y-1 overflow-y-auto px-2 pb-3">
        <button
          v-for="it in queue"
          :key="it.id"
          class="flex w-full items-center gap-2 rounded-md p-1.5 text-left"
          :class="it.id === currentId ? 'bg-surface shadow-[0_0_0_1px_var(--border-strong)]' : 'hover:bg-surface-3'"
          @click="go(it)"
        >
          <div class="w-10 shrink-0"><Thumb :file="it.file" :status="it.status" /></div>
          <div class="min-w-0">
            <div class="truncate text-[12px] font-semibold">{{ it.title || it.slug }}</div>
            <StatusPill :status="it.status" />
          </div>
        </button>
        <div v-if="!queue.length" class="px-2 pt-8 text-center text-[12px] text-muted">
          {{ scope === 'review' ? 'Nothing waiting for review.' : 'No images yet.' }}
        </div>
      </div>
    </aside>

    <!-- Stage -->
    <section v-if="current" class="flex min-w-0 flex-1 flex-col">
      <div class="flex min-h-0 flex-1 items-center justify-center p-6">
        <div class="checker relative aspect-square h-full max-h-[calc(100vh-260px)] max-w-full overflow-hidden rounded-xl border border-line shadow-sm">
          <img v-if="attempt?.file" :src="fileUrl(attempt.file)" class="size-full object-contain" alt="" />
          <div v-else class="flex size-full items-center justify-center"><Thumb :file="null" :status="current.status" /></div>
        </div>
      </div>
      <!-- Attempts strip -->
      <div class="flex items-center gap-2 border-t border-line bg-surface px-6 py-2.5">
        <button class="rounded p-1 text-muted hover:text-fg" title="Previous (←)" @click="step(-1)"><Icon name="left" /></button>
        <div class="flex min-w-0 flex-1 gap-2 overflow-x-auto">
          <button
            v-for="a in attempts.filter((x) => x.file)"
            :key="a.id"
            class="relative w-14 shrink-0 rounded-md border-2"
            :class="a.id === attempt?.id ? 'border-accent' : 'border-transparent'"
            :title="`Attempt ${a.n} · ${a.kind}`"
            @click="shownAttempt = a.id"
          >
            <Thumb :file="a.file" />
            <span class="absolute right-0.5 bottom-0.5 rounded bg-black/60 px-1 text-[9px] font-bold text-white">#{{ a.n }}</span>
            <span v-if="a.id === current.approved_attempt" class="absolute top-0.5 left-0.5 text-accent"><Icon name="star" :size="12" /></span>
          </button>
        </div>
        <span class="text-[12px] text-faint tabular-nums">{{ index + 1 }} / {{ queue.length }}</span>
        <button class="rounded p-1 text-muted hover:text-fg" title="Next (→)" @click="step(1)"><Icon name="chevron" /></button>
      </div>
    </section>
    <section v-else class="flex flex-1 items-center justify-center text-muted">
      <div class="text-center">
        <Icon name="eye" :size="28" class="mx-auto mb-2" />
        Nothing to review yet. Finished images show up here.
      </div>
    </section>

    <!-- Inspector -->
    <aside v-if="current" class="flex w-[340px] shrink-0 flex-col border-l border-line bg-surface">
      <div class="min-h-0 flex-1 overflow-y-auto p-5">
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <h2 class="truncate text-[15px] font-bold">{{ current.title || current.slug }}</h2>
            <div class="font-mono text-[11px] text-faint">{{ current.slug }}</div>
          </div>
          <StatusPill :status="current.status" />
        </div>
        <p class="mt-3 text-[12.5px] text-muted selectable">{{ current.subject }}</p>

        <div v-if="current.last_error" class="mt-3 rounded-lg bg-bad-soft p-2.5 text-[12px] text-bad selectable">{{ current.last_error }}</div>

        <!-- Check -->
        <div v-if="gate" class="mt-4 rounded-lg border border-line p-3">
          <div class="flex items-center justify-between">
            <span class="text-[12px] font-bold">Pixel check</span>
            <Pill :tone="verdictTone" dot>{{ gate.verdict === 'pass' ? 'Passed' : gate.verdict === 'flag' ? 'Look closer' : 'Failed' }}</Pill>
          </div>
          <ul v-if="gate.reasons.length" class="mt-2 space-y-1 text-[12px] text-muted">
            <li v-for="r in gate.reasons" :key="r" class="flex gap-1.5"><Icon name="alert" :size="13" class="mt-0.5 text-warn" />{{ r }}</li>
          </ul>
          <dl class="mt-2 grid grid-cols-3 gap-2 text-[11px]">
            <div><dt class="text-faint">Background</dt><dd class="font-semibold tabular-nums">{{ Math.round(gate.background * 100) }}%</dd></div>
            <div><dt class="text-faint">Sealed holes</dt><dd class="font-semibold tabular-nums">{{ Math.max(gate.holes, gate.enclosed) }}</dd></div>
            <div><dt class="text-faint">Size</dt><dd class="font-semibold tabular-nums">{{ gate.width }}×{{ gate.height }}</dd></div>
          </dl>
        </div>

        <!-- Attempt -->
        <div v-if="attempt" class="mt-4 rounded-lg border border-line p-3 text-[12px]">
          <div class="flex items-center justify-between">
            <span class="font-bold">Attempt #{{ attempt.n }} <span class="font-medium text-faint">· {{ attempt.kind }}</span></span>
            <span class="font-bold tabular-nums">{{ usd(attempt.cost) }}</span>
          </div>
          <div class="mt-1 text-faint">{{ attempt.model }} · {{ attempt.quality }} · {{ (attempt.ms / 1000).toFixed(0) }}s · {{ ago(attempt.created_at) }}</div>
          <div v-if="attempt.note" class="mt-2 flex gap-1.5 text-muted"><Icon name="message" :size="13" class="mt-0.5" />{{ attempt.note }}</div>
          <button class="mt-2 font-semibold text-link" @click="showPrompt = !showPrompt">{{ showPrompt ? 'Hide' : 'Show' }} full prompt</button>
          <pre v-if="showPrompt" class="mt-2 max-h-56 overflow-auto rounded bg-surface-2 p-2 font-mono text-[11px] whitespace-pre-wrap text-muted selectable">{{ attempt.prompt }}</pre>
        </div>

        <!-- Redo -->
        <div class="mt-4">
          <label class="label">Feedback for a redo <span class="font-normal text-faint">(F)</span></label>
          <textarea
            ref="noteBox"
            v-model="note"
            rows="3"
            class="field"
            placeholder="e.g. make the bottle taller and the liquid more green"
          />
          <label v-if="attempt?.file" class="mt-2 flex items-center gap-2 text-[12px] text-muted">
            <input v-model="editCurrent" type="checkbox" />
            Edit this image (keeps everything else) instead of redrawing
          </label>
          <UiButton class="mt-2 w-full" icon="redo" @click="redo">{{ note ? 'Redo with note' : 'Redraw' }} <span class="text-faint">⌘↵</span></UiButton>
        </div>
      </div>

      <div class="space-y-2 border-t border-line p-4">
        <div class="grid grid-cols-2 gap-2">
          <UiButton variant="primary" icon="check" :disabled="!attempt?.file" @click="approve">Approve <span class="opacity-70">A</span></UiButton>
          <UiButton icon="x" @click="reject">Reject <span class="text-faint">R</span></UiButton>
        </div>
        <UiButton variant="ghost" icon="star" class="w-full" :disabled="!attempt?.file" @click="useAsReference">Use as style reference</UiButton>
      </div>
    </aside>
  </div>
</template>
