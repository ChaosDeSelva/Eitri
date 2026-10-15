import type { ItemStatus, RunState } from './api'

export function usd(n: number): string {
  if (!Number.isFinite(n)) return '$0.00'
  if (n > 0 && n < 0.01) return `$${n.toFixed(3)}`
  return `$${n.toFixed(2)}`
}

export function ago(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000))
  if (s < 45) return 'just now'
  const m = Math.round(s / 60)
  if (m < 60) return `${m}m ago`
  const h = Math.round(m / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.round(h / 24)
  return `${d}d ago`
}

export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

export const STATUS_LABEL: Record<ItemStatus, string> = {
  draft: 'Draft',
  queued: 'Queued',
  drawing: 'Drawing',
  done: 'Done',
  flagged: 'Needs review',
  failed: 'Failed',
  refused: 'Refused',
  approved: 'Approved',
  rejected: 'Rejected',
}

export type Tone = 'neutral' | 'info' | 'ok' | 'warn' | 'bad' | 'accent'

export const STATUS_TONE: Record<ItemStatus, Tone> = {
  draft: 'neutral',
  queued: 'neutral',
  drawing: 'info',
  done: 'ok',
  flagged: 'warn',
  failed: 'bad',
  refused: 'bad',
  approved: 'accent',
  rejected: 'neutral',
}

export function runLabel(r: RunState | undefined): string {
  switch (r?.state) {
    case 'running': return 'Running'
    case 'paused': return 'Paused'
    case 'stopping': return 'Stopping'
    case 'stopped': return 'Stopped'
    default: return 'Idle'
  }
}

export function runTone(r: RunState | undefined): Tone {
  switch (r?.state) {
    case 'running': return 'info'
    case 'paused': return 'warn'
    case 'stopping': return 'warn'
    case 'stopped': return 'bad'
    default: return 'neutral'
  }
}

/** Estimated dollars per image, for the plan screen's cost preview. */
export function estimatePerImage(quality: string, withReferences: boolean): number {
  const base = quality === 'low' ? 0.011 : quality === 'high' ? 0.167 : 0.042
  return withReferences ? base * 1.45 : base
}
