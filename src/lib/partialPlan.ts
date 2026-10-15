// Reads a plan out of JSON that is still being streamed: string fields as far as they have
// been written, and every item object that has fully arrived.

export interface PartialItem { slug: string; title: string; subject: string }
export interface PartialPlan { name: string; style_brief: string; shared_tail: string; items: PartialItem[] }

function unescape(raw: string): string {
  // Drop a dangling escape or a half-written \uXXXX at the end, then let JSON decode it.
  const safe = raw.replace(/\\u[0-9a-fA-F]{0,3}$/, '').replace(/(^|[^\\])(\\\\)*\\$/, '$1$2')
  try {
    return JSON.parse(`"${safe}"`)
  } catch {
    return safe
  }
}

function stringField(text: string, key: string): string {
  const m = new RegExp(`"${key}"\\s*:\\s*"((?:[^"\\\\]|\\\\.)*)`).exec(text)
  return m ? unescape(m[1]) : ''
}

function completeObjects(text: string, from: number): PartialItem[] {
  const out: PartialItem[] = []
  let depth = 0
  let start = -1
  let inString = false
  let escaped = false
  for (let i = from; i < text.length; i++) {
    const c = text[i]
    if (inString) {
      if (escaped) escaped = false
      else if (c === '\\') escaped = true
      else if (c === '"') inString = false
      continue
    }
    if (c === '"') inString = true
    else if (c === '{') {
      if (depth === 0) start = i
      depth++
    } else if (c === '}') {
      depth--
      if (depth === 0 && start >= 0) {
        try {
          const o = JSON.parse(text.slice(start, i + 1))
          if (typeof o.subject === 'string') out.push({ slug: o.slug ?? '', title: o.title ?? '', subject: o.subject })
        } catch { /* not a full object yet */ }
        start = -1
      }
    } else if (c === ']' && depth === 0) break
  }
  return out
}

export function partialPlan(text: string): PartialPlan {
  try {
    const full = JSON.parse(text)
    return { name: full.name ?? '', style_brief: full.style_brief ?? '', shared_tail: full.shared_tail ?? '', items: full.items ?? [] }
  } catch { /* still streaming */ }
  const itemsAt = text.search(/"items"\s*:\s*\[/)
  return {
    name: stringField(text, 'name'),
    style_brief: stringField(text, 'style_brief'),
    shared_tail: stringField(text, 'shared_tail'),
    items: itemsAt >= 0 ? completeObjects(text, text.indexOf('[', itemsAt) + 1) : [],
  }
}
