/**
 * Turning a track into notation: quantise notes to a semiquaver grid, split
 * them into treble, bass and pedal staves, and cut each bar into slices
 * (moments where the set of sounding notes is constant). Slices become
 * chords or rests; notes that carry on from one slice to the next are tied.
 * This is always rhythmically faithful, and reads naturally for hymns.
 */
import type { Track } from "@/lib/api/client"

export type StaffId = "treble" | "bass" | "pedal"

export interface Grid {
  /** Crotchets per minute. */
  bpm: number
  beats: number
  beatUnit: number
  /** Track time of the first downbeat. */
  offsetMs: number
  fifths: number
  /** True when the track had no meter and the grid is a guess. */
  approximate: boolean
}

export interface Slice {
  /** In semiquavers from the start of the bar. */
  start: number
  len: number
  keys: number[]
  /** Keys that continue from the previous slice (tie into this one). */
  tiedIn: number[]
}

export interface Measure {
  index: number
  /** Track time where the bar starts and ends (ms). */
  startMs: number
  endMs: number
  staves: Record<StaffId, Slice[]>
}

const RELEASE_MS = 35

/** The track's own meter, or a guess for live recordings. */
export function gridFor(track: Track, override?: Partial<Grid>): Grid {
  const m = track.meter
  const firstNote =
    track.events.find((e) => e.type === "note" && e.velocity > 0)?.t ?? 0
  const base: Grid = m
    ? {
        bpm: m.bpm,
        beats: m.beats,
        beatUnit: m.beat_unit,
        offsetMs: m.offset_ms ?? 0,
        fifths: m.fifths ?? 0,
        approximate: false,
      }
    : {
        bpm: 80,
        beats: 4,
        beatUnit: 4,
        offsetMs: firstNote,
        fifths: 0,
        approximate: true,
      }
  return { ...base, ...override }
}

export function measureSixteenths(g: Grid): number {
  return (g.beats * 16) / g.beatUnit
}

interface QNote {
  staff: StaffId
  key: number
  s: number
  e: number
}

/** Quantised notes in semiquavers from the first downbeat. */
export function quantise(
  track: Track,
  g: Grid,
  span?: { start: number; end: number }
): QNote[] {
  const six = 60000 / g.bpm / 4
  const out: QNote[] = []
  const open = new Map<string, number>()
  for (const ev of track.events) {
    if (ev.type !== "note") continue
    if (span && (ev.t < span.start || ev.t >= span.end) && ev.velocity > 0)
      continue
    const k = `${ev.division}:${ev.key}`
    if (ev.velocity > 0) {
      open.set(k, ev.t)
    } else if (open.has(k)) {
      const t0 = open.get(k)!
      open.delete(k)
      const s = Math.round((t0 - g.offsetMs) / six)
      const e = Math.max(
        s + 1,
        Math.round((ev.t + RELEASE_MS - g.offsetMs) / six)
      )
      const staff: StaffId =
        ev.division === "pedal" ? "pedal" : ev.key >= 60 ? "treble" : "bass"
      out.push({ staff, key: ev.key, s, e })
    }
  }
  return out
}

/** Bars of slices, for every staff that has notes (treble and bass always). */
export function layout(
  track: Track,
  g: Grid,
  span?: { start: number; end: number }
): { measures: Measure[]; staves: StaffId[] } {
  const notes = quantise(track, g, span)
  const len = measureSixteenths(g)
  const six = 60000 / g.bpm / 4
  const staves: StaffId[] = ["treble", "bass"]
  if (notes.some((n) => n.staff === "pedal")) staves.push("pedal")
  if (notes.length === 0) return { measures: [], staves }
  const first = Math.floor(Math.min(...notes.map((n) => n.s)) / len)
  const last = Math.ceil(Math.max(...notes.map((n) => n.e)) / len)
  const measures: Measure[] = []
  for (let m = first; m < last; m++) {
    const m0 = m * len
    const m1 = m0 + len
    const staffSlices = {} as Record<StaffId, Slice[]>
    for (const staff of staves) {
      const here = notes.filter(
        (n) => n.staff === staff && n.s < m1 && n.e > m0
      )
      const cuts = new Set<number>([m0, m1])
      for (const n of here) {
        if (n.s > m0) cuts.add(n.s)
        if (n.e < m1) cuts.add(n.e)
      }
      const bounds = [...cuts].sort((a, b) => a - b)
      const slices: Slice[] = []
      for (let i = 0; i < bounds.length - 1; i++) {
        const [b0, b1] = [bounds[i], bounds[i + 1]]
        const sounding = here.filter((n) => n.s < b1 && n.e > b0)
        const keys = [...new Set(sounding.map((n) => n.key))].sort(
          (a, b) => a - b
        )
        const tiedIn = [
          ...new Set(sounding.filter((n) => n.s < b0).map((n) => n.key)),
        ]
        const prev = slices[slices.length - 1]
        if (prev && keys.length === 0 && prev.keys.length === 0) {
          prev.len += b1 - b0 // merge rests
        } else {
          slices.push({ start: b0 - m0, len: b1 - b0, keys, tiedIn })
        }
      }
      staffSlices[staff] = slices
    }
    measures.push({
      index: m - first,
      startMs: g.offsetMs + m0 * six,
      endMs: g.offsetMs + m1 * six,
      staves: staffSlices,
    })
  }
  return { measures, staves }
}

const VALUES: { len: number; duration: string; dots: number }[] = [
  { len: 16, duration: "w", dots: 0 },
  { len: 12, duration: "h", dots: 1 },
  { len: 8, duration: "h", dots: 0 },
  { len: 6, duration: "q", dots: 1 },
  { len: 4, duration: "q", dots: 0 },
  { len: 3, duration: "8", dots: 1 },
  { len: 2, duration: "8", dots: 0 },
  { len: 1, duration: "16", dots: 0 },
]

/** A length in semiquavers as tied note values, longest first. */
export function durationParts(
  len: number
): { duration: string; dots: number; len: number }[] {
  const out: { duration: string; dots: number; len: number }[] = []
  let left = len
  while (left > 0) {
    const v = VALUES.find((x) => x.len <= left)!
    out.push({ duration: v.duration, dots: v.dots, len: v.len })
    left -= v.len
  }
  return out
}

const SHARPS = ["c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b"]
const FLATS = ["c", "db", "d", "eb", "e", "f", "gb", "g", "ab", "a", "bb", "b"]

/** VexFlow key name, spelled with sharps or flats to suit the key. */
export function spell(midi: number, fifths: number): string {
  const names = fifths < 0 ? FLATS : SHARPS
  return `${names[midi % 12]}/${Math.floor(midi / 12) - 1}`
}

const MAJOR = [
  "Cb",
  "Gb",
  "Db",
  "Ab",
  "Eb",
  "Bb",
  "F",
  "C",
  "G",
  "D",
  "A",
  "E",
  "B",
  "F#",
  "C#",
]

export function keyName(fifths: number): string {
  return MAJOR[Math.max(-7, Math.min(7, fifths)) + 7]
}
