<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import Icon from '@/components/Icon.vue'
import PageHeader from '@/components/PageHeader.vue'
import Pill from '@/components/Pill.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { api, type Settings } from '@/lib/api'

const app = useAppStore()
const form = ref<Settings | null>(null)
const keyInputs = reactive<Record<string, string>>({})
const saving = ref<string | null>(null)
const open = ref<string | null>(null)
const showAdvanced = ref(false)

onMounted(async () => {
  form.value = (await app.guard(() => api.settingsGet())) ?? null
  await app.refreshKeys()
  open.value = app.hasAnyKey ? null : 'openai'
})

const planners = computed(() => app.catalog.filter((p) => p.planning))
const imageProvider = computed(() => app.catalog.find((p) => p.id === form.value?.default_provider))
const textProvider = computed(() => app.catalog.find((p) => p.id === form.value?.default_text_provider))

async function saveKey(provider: string) {
  const key = (keyInputs[provider] ?? '').trim()
  if (!key) return
  saving.value = provider
  const status = await app.guard(() => api.keySave(provider, key))
  saving.value = null
  if (status) {
    keyInputs[provider] = ''
    app.keys = { ...app.keys, [provider]: status }
    app.notify(`${app.providerName(provider)} key verified and saved to your keychain.`)
    open.value = null
    form.value = (await app.guard(() => api.settingsGet())) ?? form.value
  }
}

async function removeKey(provider: string) {
  const status = await app.guard(() => api.keyDelete(provider))
  if (status) {
    app.keys = { ...app.keys, [provider]: status }
    app.notify(`${app.providerName(provider)} key removed.`)
  }
}

async function saveSettings() {
  if (!form.value) return
  const s = await app.guard(() => api.settingsSave(form.value!))
  if (s) {
    form.value = s
    app.settings = s
    app.notify('Settings saved.')
  }
}

function pickProvider(id: string) {
  if (!form.value) return
  form.value.default_provider = id
  const first = app.catalog.find((p) => p.id === id)?.image_models[0]
  if (first) form.value.default_image_model = first.id
}

function pickPlanner(id: string) {
  if (!form.value) return
  form.value.default_text_provider = id
  const first = app.catalog.find((p) => p.id === id)?.text_models[0]
  if (first) form.value.default_text_model = first
}
</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader title="Settings" subtitle="Your provider keys and the defaults new projects start with." />

    <div class="min-h-0 flex-1 overflow-y-auto p-6">
      <div class="mx-auto max-w-3xl space-y-5">
        <!-- Providers -->
        <section class="overflow-hidden rounded-xl border border-line bg-surface">
          <div class="border-b border-line px-5 py-3">
            <div class="flex items-center gap-2 font-bold"><Icon name="key" /> Providers</div>
            <p class="mt-0.5 text-[12px] text-muted">
              Bring your own key for any provider you use. Each key is verified, then stored in your operating system's
              keychain. Eitri never writes keys to disk, never logs them, and never shows them again.
            </p>
          </div>
          <div v-for="p in app.catalog" :key="p.id" class="border-b border-line last:border-0">
            <button class="flex w-full items-center gap-3 px-5 py-3 text-left hover:bg-surface-2" @click="open = open === p.id ? null : p.id">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2 font-semibold">
                  {{ p.name }}
                  <Pill v-if="p.id === 'openai'" tone="neutral">reference</Pill>
                  <Pill v-if="p.planning" tone="neutral">plans</Pill>
                </div>
                <div class="truncate text-[12px] text-muted">{{ p.note }}</div>
              </div>
              <Pill v-if="app.keys[p.id]?.has_key" tone="ok" dot>Key ••••{{ app.keys[p.id]?.last4 }}</Pill>
              <Pill v-else-if="p.id === 'compatible' && form?.base_urls.compatible" tone="ok" dot>Configured</Pill>
              <Pill v-else tone="neutral">Not set</Pill>
              <Icon name="chevron" :size="14" class="text-faint transition" :class="open === p.id && 'rotate-90'" />
            </button>
            <div v-if="open === p.id" class="space-y-3 bg-surface-2 px-5 py-4">
              <form class="flex gap-2" @submit.prevent="saveKey(p.id)">
                <input
                  v-model="keyInputs[p.id]"
                  type="password"
                  autocomplete="off"
                  spellcheck="false"
                  class="field font-mono"
                  :placeholder="app.keys[p.id]?.has_key ? 'Paste a new key to replace the saved one' : p.key_hint"
                />
                <UiButton type="submit" variant="primary" :loading="saving === p.id" :disabled="!keyInputs[p.id]?.trim()">Verify and save</UiButton>
              </form>
              <div class="flex items-center gap-3 text-[12px]">
                <button v-if="p.key_url" class="font-semibold text-link" @click="api.openKeyPage(p.id)">Get a {{ p.name }} key ↗</button>
                <span v-if="p.id !== 'openai'" class="text-faint">Built from {{ p.name }}'s API docs. Report anything that breaks.</span>
                <div class="flex-1" />
                <UiButton v-if="app.keys[p.id]?.has_key" size="sm" variant="danger" icon="trash" @click="removeKey(p.id)">Remove</UiButton>
              </div>
              <label v-if="p.needs_base_url && form" class="block">
                <span class="label">Base URL</span>
                <div class="flex gap-2">
                  <input v-model="form.base_urls[p.id]" class="field font-mono" placeholder="http://localhost:8080/v1" />
                  <UiButton @click="saveSettings">Save</UiButton>
                </div>
              </label>
            </div>
          </div>
        </section>

        <template v-if="form">
          <!-- Defaults -->
          <section class="rounded-xl border border-line bg-surface">
            <div class="border-b border-line px-5 py-3 font-bold"><span class="flex items-center gap-2"><Icon name="settings" /> Defaults for new projects</span></div>
            <div class="grid gap-4 p-5 sm:grid-cols-2">
              <label>
                <span class="label">Image provider</span>
                <select :value="form.default_provider" class="field" @change="pickProvider(($event.target as HTMLSelectElement).value)">
                  <option v-for="p in app.catalog" :key="p.id" :value="p.id">{{ p.name }}</option>
                </select>
              </label>
              <label>
                <span class="label">Image model</span>
                <input v-model="form.default_image_model" class="field font-mono" list="default-image-models" />
                <datalist id="default-image-models">
                  <option v-for="m in imageProvider?.image_models ?? []" :key="m.id" :value="m.id">{{ m.label }}</option>
                </datalist>
              </label>
              <label>
                <span class="label">Planning provider</span>
                <select :value="form.default_text_provider" class="field" @change="pickPlanner(($event.target as HTMLSelectElement).value)">
                  <option v-for="p in planners" :key="p.id" :value="p.id">{{ p.name }}</option>
                </select>
              </label>
              <label>
                <span class="label">Planning model</span>
                <input v-model="form.default_text_model" class="field font-mono" list="default-text-models" />
                <datalist id="default-text-models"><option v-for="m in textProvider?.text_models ?? []" :key="m" :value="m" /></datalist>
              </label>
              <label>
                <span class="label">Images drawn at once</span>
                <input v-model.number="form.default_concurrency" type="number" min="1" max="8" class="field" />
              </label>
              <label>
                <span class="label">Budget cap per project (USD, 0 for none)</span>
                <input v-model.number="form.default_budget_usd" type="number" min="0" step="0.5" class="field" />
              </label>
            </div>
            <div class="flex justify-end border-t border-line px-5 py-3">
              <UiButton variant="primary" icon="check" @click="saveSettings">Save defaults</UiButton>
            </div>
          </section>

          <!-- Spend -->
          <section class="rounded-xl border border-line bg-surface p-5">
            <div class="flex items-center gap-2 font-bold"><Icon name="dollar" /> How spend is counted</div>
            <p class="mt-1.5 text-muted">
              Token-billed providers (OpenAI, Gemini) report the tokens each image used, and Eitri multiplies them by the
              published prices. Per-image providers (fal, Replicate, Stability, Imagen) are counted at their typical
              per-image price. You control spend per project with the <b class="text-fg">model</b>, the
              <b class="text-fg">quality</b> and the <b class="text-fg">budget cap</b>. Your provider's dashboard is the bill of record.
            </p>
          </section>

          <!-- Advanced -->
          <section class="rounded-xl border border-line bg-surface">
            <button class="flex w-full items-center justify-between px-5 py-3 font-bold" @click="showAdvanced = !showAdvanced">
              Advanced: base URLs
              <Icon name="chevron" :size="14" class="text-faint transition" :class="showAdvanced && 'rotate-90'" />
            </button>
            <div v-if="showAdvanced" class="space-y-3 border-t border-line p-5">
              <p class="text-[12px] text-muted">Leave empty to use each provider's default endpoint. Useful for proxies and regional gateways. Must be https.</p>
              <label v-for="p in app.catalog.filter((x) => !x.needs_base_url)" :key="p.id" class="block">
                <span class="label">{{ p.name }}</span>
                <input v-model="form.base_urls[p.id]" class="field font-mono" placeholder="default" />
              </label>
              <div class="flex justify-end"><UiButton variant="primary" icon="check" @click="saveSettings">Save</UiButton></div>
            </div>
          </section>
        </template>
      </div>
    </div>
  </div>
</template>
