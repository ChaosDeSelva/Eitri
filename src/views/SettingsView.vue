<script setup lang="ts">
import { onMounted, ref } from 'vue'
import Icon from '@/components/Icon.vue'
import PageHeader from '@/components/PageHeader.vue'
import Pill from '@/components/Pill.vue'
import UiButton from '@/components/UiButton.vue'
import { useAppStore } from '@/stores/app'
import { api, type ModelLists, type Settings } from '@/lib/api'

const app = useAppStore()
const keyInput = ref('')
const saving = ref(false)
const form = ref<Settings | null>(null)
const models = ref<ModelLists>({ image: [], text: [] })

onMounted(async () => {
  form.value = (await app.guard(() => api.settingsGet())) ?? null
  if (app.key.has_key) loadModels()
})

async function loadModels() {
  models.value = (await app.guard(() => api.models())) ?? models.value
}

async function saveKey() {
  if (!keyInput.value.trim()) return
  saving.value = true
  const status = await app.guard(() => api.keySave(keyInput.value))
  saving.value = false
  keyInput.value = ''
  if (status) {
    app.key = status
    app.notify('Key verified and saved to your keychain.')
    form.value = (await app.guard(() => api.settingsGet())) ?? form.value
    loadModels()
  }
}

async function removeKey() {
  const status = await app.guard(() => api.keyDelete())
  if (status) {
    app.key = status
    models.value = { image: [], text: [] }
    app.notify('Key removed from your keychain.')
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

</script>

<template>
  <div class="flex h-full flex-col">
    <PageHeader title="Settings" subtitle="Your key, defaults for new projects, and the price table used for spend." />

    <div class="min-h-0 flex-1 overflow-y-auto p-6">
      <div class="mx-auto max-w-3xl space-y-5">
        <!-- Key -->
        <section class="rounded-xl border border-line bg-surface">
          <div class="flex items-center justify-between border-b border-line px-5 py-3">
            <div class="flex items-center gap-2 font-bold"><Icon name="key" /> OpenAI API key</div>
            <Pill :tone="app.key.has_key ? 'ok' : 'accent'" dot>
              {{ app.key.has_key ? `Saved, ends in ${app.key.last4}` : 'Not set' }}
            </Pill>
          </div>
          <div class="space-y-3 p-5">
            <p class="text-muted">
              Eitri is bring-your-own-key. The key is checked against OpenAI, then stored in your operating system's
              keychain. It is never written to disk by Eitri, never logged, and never shown again.
            </p>
            <form class="flex gap-2" @submit.prevent="saveKey">
              <input
                v-model="keyInput"
                type="password"
                autocomplete="off"
                spellcheck="false"
                class="field font-mono"
                :placeholder="app.key.has_key ? 'Paste a new key to replace the saved one' : 'sk-...'"
              />
              <UiButton type="submit" variant="primary" :loading="saving" :disabled="!keyInput.trim()">Verify and save</UiButton>
            </form>
            <div v-if="app.key.has_key" class="flex justify-end">
              <UiButton size="sm" variant="danger" icon="trash" @click="removeKey">Remove key</UiButton>
            </div>
          </div>
        </section>

        <template v-if="form">
          <!-- Defaults -->
          <section class="rounded-xl border border-line bg-surface">
            <div class="flex items-center justify-between border-b border-line px-5 py-3">
              <div class="flex items-center gap-2 font-bold"><Icon name="settings" /> Defaults for new projects</div>
              <UiButton size="sm" variant="ghost" icon="redo" :disabled="!app.key.has_key" @click="loadModels">Refresh models</UiButton>
            </div>
            <div class="grid gap-4 p-5 sm:grid-cols-2">
              <label>
                <span class="label">Image model</span>
                <input v-model="form.default_image_model" class="field" list="image-models" />
                <datalist id="image-models"><option v-for="m in models.image" :key="m" :value="m" /></datalist>
              </label>
              <label>
                <span class="label">Planning model</span>
                <input v-model="form.default_text_model" class="field" list="text-models" />
                <datalist id="text-models"><option v-for="m in models.text" :key="m" :value="m" /></datalist>
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
          </section>

          <!-- Spend -->
          <section class="rounded-xl border border-line bg-surface p-5">
            <div class="flex items-center gap-2 font-bold"><Icon name="dollar" /> How spend is counted</div>
            <p class="mt-1.5 text-muted">
              Every response from OpenAI reports the tokens it used. Eitri multiplies those by OpenAI's published
              prices to show what each image and each project cost. You control spend per project with the
              <b class="text-fg">quality</b>, the <b class="text-fg">budget cap</b> (the run stops when it is reached),
              and how many images draw at once. Your OpenAI dashboard is the bill of record.
            </p>
          </section>

          <!-- Advanced -->
          <section class="rounded-xl border border-line bg-surface p-5">
            <label>
              <span class="label">API base URL</span>
              <input v-model="form.api_base" class="field font-mono" />
            </label>
            <p class="mt-1.5 text-[12px] text-muted">For OpenAI-compatible gateways. Must be https.</p>
          </section>

          <div class="flex justify-end pb-6">
            <UiButton variant="primary" icon="check" @click="saveSettings">Save settings</UiButton>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
