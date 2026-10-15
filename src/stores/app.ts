import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { api, errorText, type Item, type KeyStatus, type ProjectSummary, type RunState, type Settings, type SpendSummary } from '@/lib/api'

type Theme = 'system' | 'light' | 'dark'

export const useAppStore = defineStore('app', () => {
  const key = ref<KeyStatus>({ has_key: false, last4: null })
  const settings = ref<Settings | null>(null)
  const projects = ref<ProjectSummary[]>([])
  const spend = ref<SpendSummary>({ today_usd: 0, total_usd: 0 })
  const runs = ref<Record<string, RunState>>({})
  const toast = ref<{ text: string; tone: 'ok' | 'bad' } | null>(null)
  const theme = ref<Theme>((localStorage.getItem('eitri.theme') as Theme) || 'system')
  const itemListeners = new Set<(item: Item) => void>()
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let started = false

  function applyTheme() {
    const root = document.documentElement
    if (theme.value === 'system') root.removeAttribute('data-theme')
    else root.setAttribute('data-theme', theme.value)
  }

  function setTheme(t: Theme) {
    theme.value = t
    try { localStorage.setItem('eitri.theme', t) } catch { /* private storage */ }
    applyTheme()
  }

  function notify(text: string, tone: 'ok' | 'bad' = 'ok') {
    toast.value = { text, tone }
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast.value = null), tone === 'bad' ? 6000 : 2800)
  }

  async function guard<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn()
    } catch (e) {
      notify(errorText(e), 'bad')
      return undefined
    }
  }

  async function refreshProjects() {
    projects.value = (await guard(() => api.projects())) ?? projects.value
  }

  async function refreshSpend() {
    spend.value = (await guard(() => api.spend())) ?? spend.value
  }

  async function refreshKey() {
    key.value = (await guard(() => api.keyStatus())) ?? key.value
  }

  function onItem(fn: (item: Item) => void) {
    itemListeners.add(fn)
    return () => itemListeners.delete(fn)
  }

  async function start() {
    if (started) return
    started = true
    applyTheme()
    await Promise.all([refreshKey(), refreshProjects(), refreshSpend()])
    settings.value = (await guard(() => api.settingsGet())) ?? null
    runs.value = (await guard(() => api.runStates())) ?? {}
    await listen<Item>('item-updated', (e) => itemListeners.forEach((fn) => fn(e.payload)))
    await listen<{ project_id: string; run: RunState }>('run-updated', (e) => {
      runs.value = { ...runs.value, [e.payload.project_id]: e.payload.run }
      if (e.payload.run.state === 'stopped') notify(`Run stopped: ${e.payload.run.reason}`, 'bad')
      refreshProjects()
    })
    await listen('spend-updated', () => refreshSpend())
  }

  return {
    key, settings, projects, spend, runs, toast, theme,
    start, notify, guard, refreshProjects, refreshSpend, refreshKey, onItem, setTheme,
  }
})
