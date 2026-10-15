import { describe, expect, it } from 'vitest'
import { partialPlan } from './partialPlan'

const full = JSON.stringify({
  name: 'Potion Icons',
  style_brief: 'Chunky "hand-painted" icons.',
  shared_tail: 'Framing: centred.\nStyle: flat.',
  items: [
    { slug: 'red', title: 'Red', subject: 'A red potion {glowing}' },
    { slug: 'blue', title: 'Blue', subject: 'A blue potion' },
  ],
})

describe('partialPlan', () => {
  it('parses a finished plan', () => {
    expect(partialPlan(full).items).toHaveLength(2)
  })

  it('reads strings as far as they are written', () => {
    const cut = full.slice(0, full.indexOf('icons.') + 3)
    const p = partialPlan(cut)
    expect(p.name).toBe('Potion Icons')
    expect(p.style_brief).toBe('Chunky "hand-painted" ico')
    expect(p.items).toEqual([])
  })

  it('returns only items that have fully arrived, ignoring braces inside strings', () => {
    const cut = full.slice(0, full.indexOf('A blue') + 4)
    const p = partialPlan(cut)
    expect(p.items).toEqual([{ slug: 'red', title: 'Red', subject: 'A red potion {glowing}' }])
    expect(p.shared_tail).toBe('Framing: centred.\nStyle: flat.')
  })

  it('survives a cut in the middle of an escape', () => {
    const p = partialPlan('{"name":"A\\')
    expect(p.name).toBe('A')
  })
})
