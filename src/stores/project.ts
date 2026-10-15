import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api, type Item, type ItemStatus, type ProjectDetail, type ProjectPatch } from '@/lib/api'
import { useAppStore } from './app'

export type StatusFilter = 'all' | 'active' | 'review' | 'problems' | ItemStatus

export const useProjectStore = defineStore('project', () => {
  const app = useAppStore()
  const id = ref<string | null>(null)
  const detail = ref<ProjectDetail | null>(null)
  const items = ref<Item[]>([])
  const filter = ref<StatusFilter>('all')
  const search = ref('')
  const selected = ref<Set<string>>(new Set())
  let refreshTimer: ReturnType<typeof setTimeout> | undefined

  const project = computed(() => detail.value?.project ?? null)
  const run = computed(() => (id.value ? app.runs[id.value] : undefined) ?? detail.value?.run ?? { state: 'idle' as const })

  const counts = computed(() => {
    const c: Record<string, number> = {}
    for (const it of items.value) c[it.status] = (c[it.status] ?? 0) + 1
    return c
  })

  const visible = computed(() => {
    const q = search.value.trim().toLowerCase()
    return items.value.filter((it) => {
      const f = filter.value
      const okStatus =
        f === 'all' ||
        (f === 'active' && (it.status === 'queued' || it.status === 'drawing')) ||
        (f === 'review' && (it.status === 'done' || it.status === 'flagged')) ||
        (f === 'problems' && (it.status === 'failed' || it.status === 'refused' || it.status === 'flagged')) ||
        it.status === f
      return okStatus && (!q || it.slug.includes(q) || it.title.toLowerCase().includes(q) || it.subject.toLowerCase().includes(q))
    })
  })

  async function load(projectId: string) {
    if (id.value !== projectId) {
      id.value = projectId
      detail.value = null
      items.value = []
      selected.value = new Set()
      filter.value = 'all'
      search.value = ''
    }
    await refresh()
  }

  async function refresh() {
    const pid = id.value
    if (!pid) return
    const [d, list] = await Promise.all([app.guard(() => api.projectGet(pid)), app.guard(() => api.items(pid))])
    if (pid !== id.value) return
    if (d) detail.value = d
    if (list) items.value = list
  }

  /** Coalesces bursts of events into one refresh of the counts and spend. */
  function softRefresh() {
    clearTimeout(refreshTimer)
    refreshTimer = setTimeout(async () => {
      const pid = id.value
      if (!pid) return
      const d = await app.guard(() => api.projectGet(pid))
      if (d && pid === id.value) detail.value = d
    }, 250)
  }

  app.onItem((item) => {
    if (item.project_id !== id.value) return
    const i = items.value.findIndex((x) => x.id === item.id)
    if (i >= 0) items.value.splice(i, 1, item)
    else items.value.push(item)
    softRefresh()
  })

  async function update(patch: ProjectPatch) {
    const pid = id.value
    if (!pid) return
    const p = await app.guard(() => api.projectUpdate(pid, patch))
    if (p && detail.value) detail.value = { ...detail.value, project: p }
    app.refreshProjects()
    return p
  }

  function replaceItem(item: Item | undefined) {
    if (!item) return
    const i = items.value.findIndex((x) => x.id === item.id)
    if (i >= 0) items.value.splice(i, 1, item)
  }

  function toggle(itemId: string) {
    const s = new Set(selected.value)
    if (s.has(itemId)) s.delete(itemId)
    else s.add(itemId)
    selected.value = s
  }

  return {
    id, detail, items, filter, search, selected, project, run, counts, visible,
    load, refresh, softRefresh, update, replaceItem, toggle,
  }
})
