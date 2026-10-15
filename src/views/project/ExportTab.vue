<script setup lang="ts">
import { computed, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import Icon from '@/components/Icon.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { useProjectStore } from '@/stores/project'
import { api, type ExportReport } from '@/lib/api'

const app = useAppStore()
const ps = useProjectStore()
const busy = ref<'approved' | 'all' | null>(null)
const last = ref<ExportReport | null>(null)
const SIZES = [128, 256, 512, 1024, 2048]

const approved = computed(() => ps.counts.approved ?? 0)
const passing = computed(() => approved.value + (ps.counts.done ?? 0) + (ps.counts.flagged ?? 0))
const sample = computed(() => {
  const p = ps.project
  const it = ps.items[0]
  if (!p || !it) return '{slug}.png'
  return p.filename_pattern.replace('{slug}', it.slug).replace('{n}', String(it.ord + 1).padStart(3, '0')).replace('{project}', p.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')) + '.png'
})

async function pickFolder() {
  const dir = await open({ directory: true, multiple: false })
  if (typeof dir === 'string') await ps.update({ output_dir: dir })
}

function toggle(list: 'export_sizes' | 'webp_sizes', size: number) {
  const cur = ps.project?.[list] ?? []
  const next = cur.includes(size) ? cur.filter((s) => s !== size) : [...cur, size].sort((a, b) => b - a)
  ps.update({ [list]: next })
}

async function run(onlyApproved: boolean) {
  busy.value = onlyApproved ? 'approved' : 'all'
  const r = await app.guard(() => api.exportProject(ps.id!, onlyApproved))
  busy.value = null
  if (r) {
    last.value = r
    app.notify(`Exported ${r.written} images.`)
  }
}
</script>

<template>
  <div v-if="ps.project" class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-3xl space-y-5">
      <section class="rounded-xl border border-line bg-surface p-5">
        <div class="mb-3 flex items-center gap-2 font-bold"><Icon name="folder" /> Output folder</div>
        <div class="flex gap-2">
          <div class="field flex items-center truncate font-mono text-[12px]" :class="!ps.project.output_dir && 'text-faint'">
            {{ ps.project.output_dir || 'No folder chosen' }}
          </div>
          <UiButton icon="folder" @click="pickFolder">Choose</UiButton>
        </div>
        <p class="mt-2 text-[12px] text-muted">Point this at your game or site's art folder. Files are written atomically, so nothing half-written is ever picked up.</p>
      </section>

      <section class="rounded-xl border border-line bg-surface p-5">
        <div class="mb-3 font-bold">Formats</div>
        <div class="space-y-4">
          <div>
            <span class="label">PNG masters (square, transparent padding)</span>
            <div class="flex flex-wrap gap-1.5">
              <button
                v-for="s in SIZES"
                :key="s"
                class="h-7 rounded-md border px-2.5 text-[12px] font-semibold tabular-nums"
                :class="ps.project.export_sizes.includes(s) ? 'border-accent bg-accent-soft text-accent' : 'border-line text-muted hover:text-fg'"
                @click="toggle('export_sizes', s)"
              >
                {{ s }}
              </button>
            </div>
          </div>
          <div>
            <span class="label">WebP copies (written to a folder per size)</span>
            <div class="flex flex-wrap gap-1.5">
              <button
                v-for="s in SIZES"
                :key="s"
                class="h-7 rounded-md border px-2.5 text-[12px] font-semibold tabular-nums"
                :class="ps.project.webp_sizes.includes(s) ? 'border-accent bg-accent-soft text-accent' : 'border-line text-muted hover:text-fg'"
                @click="toggle('webp_sizes', s)"
              >
                {{ s }}
              </button>
            </div>
          </div>
          <label class="block">
            <span class="label">File name pattern</span>
            <input :value="ps.project.filename_pattern" class="field font-mono" @change="ps.update({ filename_pattern: ($event.target as HTMLInputElement).value })" />
            <span class="mt-1 block text-[12px] text-muted">
              Tokens: <code>{slug}</code> <code>{n}</code> <code>{project}</code>. Example: <span class="font-mono text-fg">{{ sample }}</span>
            </span>
          </label>
        </div>
      </section>

      <section class="flex items-center gap-3 rounded-xl border border-line bg-surface p-5">
        <div class="flex-1">
          <div class="font-bold">Export</div>
          <div class="text-[12px] text-muted">{{ approved }} approved · {{ passing }} with a usable image</div>
        </div>
        <UiButton :loading="busy === 'all'" :disabled="!passing || !ps.project.output_dir" @click="run(false)">Export all usable</UiButton>
        <UiButton variant="primary" icon="export" :loading="busy === 'approved'" :disabled="!approved || !ps.project.output_dir" @click="run(true)">
          Export approved
        </UiButton>
      </section>

      <div v-if="last" class="rounded-lg bg-ok-soft p-3 text-[12px] text-ok selectable">
        Wrote {{ last.written }} images to {{ last.dir }}<template v-if="last.skipped"> ({{ last.skipped }} skipped)</template>.
      </div>
    </div>
  </div>
</template>
