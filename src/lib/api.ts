// Typed wrappers for every Rust command. The API key goes in through keySave and never
// comes back: the UI only ever sees KeyStatus.
import { invoke, convertFileSrc } from '@tauri-apps/api/core'

export type ItemStatus =
  | 'draft' | 'queued' | 'drawing' | 'done' | 'flagged' | 'failed' | 'refused' | 'approved' | 'rejected'

export type RunState =
  | { state: 'idle' } | { state: 'running' } | { state: 'paused' } | { state: 'stopping' }
  | { state: 'stopped'; reason: string }

export interface KeyStatus { has_key: boolean; last4: string | null }
export interface Price { text_in: number; image_in: number; output: number }
export interface Settings {
  default_image_model: string
  default_text_model: string
  default_concurrency: number
  default_budget_usd: number
  api_base: string
  prices: Record<string, Price>
}
export interface Project {
  id: string; name: string; idea: string; style_brief: string; shared_tail: string
  output_dir: string | null; image_model: string; text_model: string; quality: string; size: string
  background: string; gate: string; concurrency: number; budget_usd: number
  export_sizes: number[]; webp_sizes: number[]; filename_pattern: string
  use_references: boolean; spec_locked: boolean; created_at: number; updated_at: number
}
export interface ProjectStats {
  total: number; draft: number; queued: number; drawing: number; done: number; flagged: number
  failed: number; approved: number; rejected: number; spent_usd: number
}
export interface ProjectSummary extends Project { stats: ProjectStats; cover: string | null }
export interface Reference { id: string; project_id: string; file: string; attempt_id: string | null; created_at: number }
export interface ProjectDetail { project: Project; stats: ProjectStats; run: RunState; references: Reference[] }
export interface Item {
  id: string; project_id: string; ord: number; slug: string; title: string; subject: string
  prompt_sha: string | null; status: ItemStatus; priority: number; redo_note: string | null
  redo_from: string | null; auto_retries: number; current_attempt: string | null
  approved_attempt: string | null; last_error: string | null; updated_at: number
  file: string | null; attempts: number; cost: number; verdict: string | null; reasons: string[]
}
export interface GateReport {
  verdict: 'pass' | 'flag' | 'fail'; reasons: string[]; corners: number[]; holes: number
  enclosed: number; background: number; solid: boolean; width: number; height: number
}
export interface Attempt {
  id: string; item_id: string; project_id: string; n: number; kind: string; model: string
  quality: string; prompt: string; note: string | null; file: string | null; cost: number
  usage: { input_text: number; input_image: number; output: number } | null
  gate: GateReport | null; verdict: string | null; error: string | null; ms: number; created_at: number
}
export interface ActivityEvent {
  id: number; project_id: string | null; item_id: string | null; kind: string; message: string; ts: number
}
export interface SpendSummary { today_usd: number; total_usd: number }
export interface ModelLists { image: string[]; text: string[] }
export interface PlanRequest { idea: string; count: number; background: string; gate: string; size: string; notes: string }
export interface ExportReport { written: number; skipped: number; dir: string }
export type ProjectPatch = Partial<Omit<Project, 'id' | 'spec_locked' | 'created_at' | 'updated_at'>>

export const api = {
  keyStatus: () => invoke<KeyStatus>('key_status'),
  keySave: (key: string) => invoke<KeyStatus>('key_save', { key }),
  keyDelete: () => invoke<KeyStatus>('key_delete'),
  models: () => invoke<ModelLists>('models'),
  settingsGet: () => invoke<Settings>('settings_get'),
  settingsSave: (settings: Settings) => invoke<Settings>('settings_save', { settings }),
  spend: () => invoke<SpendSummary>('spend'),

  projects: () => invoke<ProjectSummary[]>('projects'),
  runStates: () => invoke<Record<string, RunState>>('run_states'),
  projectCreate: (name: string) => invoke<Project>('project_create', { name }),
  projectGet: (id: string) => invoke<ProjectDetail>('project_get', { id }),
  projectUpdate: (id: string, patch: ProjectPatch) => invoke<Project>('project_update', { id, patch }),
  projectDelete: (id: string) => invoke<void>('project_delete', { id }),

  planDraft: (projectId: string, request: PlanRequest) => invoke<Project>('plan_draft', { projectId, request }),
  specLock: (projectId: string, locked: boolean) => invoke<Project>('spec_lock', { projectId, locked }),

  items: (projectId: string) => invoke<Item[]>('items', { projectId }),
  itemAdd: (projectId: string, title: string, subject: string) => invoke<Item>('item_add', { projectId, title, subject }),
  itemEdit: (id: string, title?: string, subject?: string) => invoke<Item>('item_edit', { id, title, subject }),
  itemDelete: (id: string) => invoke<void>('item_delete', { id }),
  attempts: (itemId: string) => invoke<Attempt[]>('attempts', { itemId }),
  itemApprove: (id: string, attemptId?: string) => invoke<Item>('item_approve', { id, attemptId }),
  itemReject: (id: string) => invoke<Item>('item_reject', { id }),
  itemRedo: (id: string, note: string | null, editCurrent: boolean) => invoke<Item>('item_redo', { id, note, editCurrent }),

  runStart: (projectId: string) => invoke<RunState>('run_start', { projectId }),
  runPreview: (projectId: string, itemIds: string[]) => invoke<RunState>('run_preview', { projectId, itemIds }),
  runPause: (projectId: string) => invoke<RunState>('run_pause', { projectId }),
  runStop: (projectId: string) => invoke<RunState>('run_stop', { projectId }),

  referenceFromAttempt: (attemptId: string) => invoke<Reference>('reference_from_attempt', { attemptId }),
  referenceImport: (projectId: string, path: string) => invoke<Reference>('reference_import', { projectId, path }),
  referenceRemove: (id: string) => invoke<void>('reference_remove', { id }),

  exportProject: (projectId: string, onlyApproved: boolean) => invoke<ExportReport>('export_project', { projectId, onlyApproved }),
  events: (projectId: string, limit = 300) => invoke<ActivityEvent[]>('events', { projectId, limit }),
}

/** Local image path to a URL the webview can load (scoped to the app-data folder). */
export function fileUrl(path: string | null | undefined): string {
  if (!path) return ''
  try {
    return convertFileSrc(path)
  } catch {
    return ''
  }
}

export function errorText(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return 'Something went wrong.'
}
