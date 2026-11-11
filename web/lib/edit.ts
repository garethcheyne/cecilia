/**
 * Track editing as pure functions. The editor works on an `EditTrack`
 * (notes as start/end pairs plus everything else) and turns it back into a
 * normal track to save. Every operation returns a new value, which makes
 * undo a simple stack.
 */
import type { DivisionRole, Schemas, TimedEvent, Track } from "@/lib/api/client"

export interface Note {
  id: number
  division: DivisionRole
  key: number
  start: number
  end: number
  velocity: number
}

/** Everything that isn't a note: stop changes, expression, master volume. */
export type NewControl = Exclude<Schemas["TrackEvent"], { type: "note" }> & { t: number }
export type Control = NewControl & { id: number }

export interface EditTrack {
  track: Track
  notes: Note[]
  controls: Control[]
  /** Next free id. */
  nextId: number
}

export const MIN_NOTE_MS = 20

export function toEdit(track: Track): EditTrack {
  const notes: Note[] = []
  const controls: Control[] = []
  // Open notes per key, oldest first. Two voices can sound the same key
  // (a unison in a hymn), so a note-off closes the oldest open note.
  const open = new Map<string, Note[]>()
  let id = 1
  for (const e of track.events) {
    if (e.type === "note") {
      const k = `${e.division}:${e.key}`
      const stack = open.get(k) ?? []
      if (e.velocity > 0) {
        const n: Note = { id: id++, division: e.division, key: e.key, start: e.t, end: track.duration_ms, velocity: e.velocity }
        notes.push(n)
        stack.push(n)
        open.set(k, stack)
      } else {
        const n = stack.shift()
        if (n) n.end = Math.max(n.start + 1, e.t)
      }
    } else {
      controls.push({ ...e, id: id++ } as Control)
    }
  }
  return { track, notes, controls, nextId: id }
}

/** Back to a track: events sorted the way the server expects. */
export function fromEdit(e: EditTrack): Track {
  type Ev = TimedEvent & { rank: number }
  const events: Ev[] = []
  for (const n of e.notes) {
    events.push({ t: Math.round(n.start), type: "note", division: n.division, key: n.key, velocity: n.velocity, rank: 2 })
    events.push({ t: Math.round(n.end), type: "note", division: n.division, key: n.key, velocity: 0, rank: 0 })
  }
  for (const c of e.controls) {
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const { id, ...rest } = c
    events.push({ ...(rest as TimedEvent), t: Math.round(c.t), rank: 1 })
  }
  events.sort((a, b) => a.t - b.t || a.rank - b.rank)
  const last = events.length ? events[events.length - 1].t : 0
  return {
    ...e.track,
    duration_ms: Math.max(e.track.duration_ms, last + 500),
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    events: events.map(({ rank, ...ev }) => ev as TimedEvent),
  }
}

const clampKey = (k: number) => Math.max(0, Math.min(127, k))

export function moveNotes(e: EditTrack, ids: Set<number>, dt: number, dkey: number): EditTrack {
  const minStart = Math.min(...e.notes.filter((n) => ids.has(n.id)).map((n) => n.start))
  const shift = Math.max(dt, -minStart) // never before the start
  return {
    ...e,
    notes: e.notes.map((n) =>
      ids.has(n.id) ? { ...n, start: n.start + shift, end: n.end + shift, key: clampKey(n.key + dkey) } : n
    ),
  }
}

export function resizeNotes(e: EditTrack, ids: Set<number>, dEnd: number): EditTrack {
  return {
    ...e,
    notes: e.notes.map((n) => (ids.has(n.id) ? { ...n, end: Math.max(n.start + MIN_NOTE_MS, n.end + dEnd) } : n)),
  }
}

export function setDivision(e: EditTrack, ids: Set<number>, division: DivisionRole): EditTrack {
  return { ...e, notes: e.notes.map((n) => (ids.has(n.id) ? { ...n, division } : n)) }
}

export function deleteNotes(e: EditTrack, ids: Set<number>): EditTrack {
  return { ...e, notes: e.notes.filter((n) => !ids.has(n.id)) }
}

export function deleteControls(e: EditTrack, ids: Set<number>): EditTrack {
  return { ...e, controls: e.controls.filter((c) => !ids.has(c.id)) }
}

export function addNote(e: EditTrack, n: Omit<Note, "id">): [EditTrack, number] {
  const id = e.nextId
  return [{ ...e, notes: [...e.notes, { ...n, id }], nextId: id + 1 }, id]
}

export function addControl(e: EditTrack, c: NewControl): [EditTrack, number] {
  const id = e.nextId
  return [{ ...e, controls: [...e.controls, { ...c, id } as Control], nextId: id + 1 }, id]
}

export function moveControl(e: EditTrack, id: number, t: number): EditTrack {
  return { ...e, controls: e.controls.map((c) => (c.id === id ? { ...c, t: Math.max(0, t) } : c)) }
}

/** Transpose notes (all notes when `ids` is empty). */
export function transpose(e: EditTrack, ids: Set<number>, semitones: number): EditTrack {
  const all = ids.size === 0
  return { ...e, notes: e.notes.map((n) => (all || ids.has(n.id) ? { ...n, key: clampKey(n.key + semitones) } : n)) }
}

/** Notes overlapping a time/pitch rectangle. */
export function notesIn(e: EditTrack, t0: number, t1: number, k0: number, k1: number): number[] {
  const [a, b] = [Math.min(t0, t1), Math.max(t0, t1)]
  const [lo, hi] = [Math.min(k0, k1), Math.max(k0, k1)]
  return e.notes.filter((n) => n.end > a && n.start < b && n.key >= lo && n.key <= hi).map((n) => n.id)
}

export type RangeKind = "notes" | "stops" | "expression"

/**
 * Delete what's between `start` and `end`. With `close`, later events move
 * back to close the gap (cutting out a section); otherwise the time stays as
 * silence. Notes that straddle an edge are trimmed.
 */
export function deleteRange(
  e: EditTrack,
  start: number,
  end: number,
  kinds: RangeKind[] = ["notes", "stops", "expression"],
  close = true
): EditTrack {
  const len = end - start
  const shift = (t: number) => (close && t >= end ? t - len : t)
  const notes: Note[] = []
  for (const n of e.notes) {
    if (!kinds.includes("notes")) {
      notes.push({ ...n, start: shift(n.start), end: shift(n.end) })
      continue
    }
    if (n.end <= start || n.start >= end) {
      notes.push({ ...n, start: shift(n.start), end: shift(n.end) })
    } else if (n.start < start) {
      notes.push({ ...n, end: start }) // keep the part before
    } else if (n.end > end && !close) {
      notes.push({ ...n, start: end })
    }
  }
  const isKind = (c: Control) => (c.type === "stop" ? kinds.includes("stops") : kinds.includes("expression"))
  const controls = e.controls
    .filter((c) => !(isKind(c) && c.t >= start && c.t < end))
    .map((c) => ({ ...c, t: shift(c.t) }))
  const track = close ? shiftTrack(e.track, start, -len) : e.track
  return { ...e, track, notes: notes.filter((n) => n.end - n.start >= 1), controls }
}

/** Insert `ms` of silence at `at`, pushing later events along. */
export function insertTime(e: EditTrack, at: number, ms: number): EditTrack {
  const s = (t: number) => (t >= at ? t + ms : t)
  return {
    ...e,
    track: shiftTrack(e.track, at, ms),
    notes: e.notes.map((n) => ({ ...n, start: s(n.start), end: n.start < at && n.end > at ? n.end : s(n.end) })),
    controls: e.controls.map((c) => ({ ...c, t: s(c.t) })),
  }
}

function shiftTrack(track: Track, at: number, by: number): Track {
  const verses = (track.verses ?? []).map((v) => (v >= at ? v + by : v)).filter((v) => v > 0)
  return { ...track, verses, duration_ms: Math.max(0, track.duration_ms + by) }
}

export interface Clip {
  length: number
  notes: Omit<Note, "id">[]
  controls: NewControl[]
}

/** Copy a section (times relative to its start). */
export function copyRange(e: EditTrack, start: number, end: number): Clip {
  return {
    length: end - start,
    notes: e.notes
      .filter((n) => n.start >= start && n.start < end)
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      .map(({ id, ...n }) => ({ ...n, start: n.start - start, end: Math.min(n.end, end) - start })),
    controls: e.controls
      .filter((c) => c.t >= start && c.t < end)
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      .map(({ id, ...c }) => ({ ...c, t: c.t - start }) as NewControl),
  }
}

/** Insert a copied section at `at`, making room for it. */
export function pasteClip(e: EditTrack, clip: Clip, at: number): EditTrack {
  let next = insertTime(e, at, clip.length)
  let id = next.nextId
  next = {
    ...next,
    notes: [...next.notes, ...clip.notes.map((n) => ({ ...n, id: id++, start: n.start + at, end: n.end + at }))],
    controls: [...next.controls, ...clip.controls.map((c) => ({ ...c, id: id++, t: c.t + at }) as Control)],
  }
  return { ...next, nextId: id }
}

/** Snap a time to a grid (ms), or leave it when the grid is 0. */
export function snap(t: number, grid: number): number {
  return grid > 0 ? Math.round(t / grid) * grid : t
}

/** Keep an undo history of edit states. */
export class History<T> {
  private past: T[] = []
  private future: T[] = []
  constructor(private limit = 200) {}
  push(state: T) {
    this.past.push(state)
    if (this.past.length > this.limit) this.past.shift()
    this.future = []
  }
  undo(current: T): T | null {
    const prev = this.past.pop()
    if (prev === undefined) return null
    this.future.push(current)
    return prev
  }
  redo(current: T): T | null {
    const next = this.future.pop()
    if (next === undefined) return null
    this.past.push(current)
    return next
  }
  get canUndo() {
    return this.past.length > 0
  }
  get canRedo() {
    return this.future.length > 0
  }
}
