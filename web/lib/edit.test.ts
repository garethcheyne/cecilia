import { describe, expect, it } from "vitest"

import type { Track } from "@/lib/api/client"

import {
  copyNotes,
  copyRange,
  flipStop,
  overlayClip,
  deleteRange,
  fromEdit,
  History,
  insertTime,
  moveNotes,
  notesIn,
  pasteClip,
  resizeNotes,
  toEdit,
  transpose,
} from "./edit"

const sw = { division: "swell" as const, equivalent: { type: "tremulant" as const } }

function track(): Track {
  return {
    schema: 1,
    id: "t",
    info: { title: "T", volume: 100 },
    source: "recorded",
    created: 0,
    modified: 0,
    initial: { stops: [], expression: {} },
    duration_ms: 5000,
    verses: [2500],
    events: [
      { t: 1000, type: "note", division: "great", key: 60, velocity: 80 },
      { t: 1500, type: "stop", stop: sw, on: true },
      { t: 1900, type: "note", division: "great", key: 60, velocity: 0 },
      { t: 2000, type: "note", division: "pedal", key: 36, velocity: 80 },
      { t: 3000, type: "note", division: "pedal", key: 36, velocity: 0 },
    ],
  }
}

describe("toEdit / fromEdit", () => {
  it("pairs notes and round-trips", () => {
    const e = toEdit(track())
    expect(e.notes.map((n) => [n.key, n.start, n.end])).toEqual([
      [60, 1000, 1900],
      [36, 2000, 3000],
    ])
    expect(e.controls).toHaveLength(1)
    const back = fromEdit(e)
    expect(back.events).toEqual(track().events)
  })
  it("orders note-off, then stops, then note-on at the same time", () => {
    const t = track()
    t.events = [
      { t: 100, type: "note", division: "great", key: 60, velocity: 80 },
      { t: 200, type: "note", division: "great", key: 60, velocity: 0 },
      { t: 200, type: "stop", stop: sw, on: true },
      { t: 200, type: "note", division: "great", key: 62, velocity: 80 },
      { t: 300, type: "note", division: "great", key: 62, velocity: 0 },
    ]
    const types = fromEdit(toEdit(t)).events.filter((e) => e.t === 200).map((e) => (e.type === "note" ? e.velocity : "stop"))
    expect(types).toEqual([0, "stop", 80])
  })
})

describe("unisons", () => {
  it("keeps two voices on the same key as two whole notes", () => {
    const t = track()
    t.events = [
      { t: 100, type: "note", division: "great", key: 63, velocity: 80 },
      { t: 100, type: "note", division: "great", key: 63, velocity: 80 },
      { t: 900, type: "note", division: "great", key: 63, velocity: 0 },
      { t: 900, type: "note", division: "great", key: 63, velocity: 0 },
    ]
    const e = toEdit(t)
    expect(e.notes.map((n) => [n.start, n.end])).toEqual([
      [100, 900],
      [100, 900],
    ])
  })
})

describe("note edits", () => {
  it("moves, resizes and transposes", () => {
    let e = toEdit(track())
    const ids = new Set([e.notes[0].id])
    e = moveNotes(e, ids, 100, 2)
    expect(e.notes[0]).toMatchObject({ key: 62, start: 1100, end: 2000 })
    e = resizeNotes(e, ids, -5000)
    expect(e.notes[0].end - e.notes[0].start).toBe(20)
    e = transpose(e, new Set(), -12)
    expect(e.notes.map((n) => n.key)).toEqual([50, 24])
  })
  it("never moves notes before the start", () => {
    const e = moveNotes(toEdit(track()), new Set([1]), -99999, 0)
    expect(e.notes[0].start).toBe(0)
  })
  it("selects by rectangle", () => {
    const e = toEdit(track())
    expect(notesIn(e, 0, 1500, 0, 127)).toEqual([e.notes[0].id])
    expect(notesIn(e, 0, 5000, 30, 40)).toEqual([e.notes[1].id])
  })
})

describe("sections", () => {
  it("cuts a section and closes the gap", () => {
    const e = deleteRange(toEdit(track()), 1400, 2100)
    // The first note is trimmed at 1400; the stop goes; the pedal note (2000) goes.
    expect(e.notes.map((n) => [n.key, n.start, n.end])).toEqual([[60, 1000, 1400]])
    expect(e.controls).toHaveLength(0)
    expect(e.track.duration_ms).toBe(4300)
    expect(e.track.verses).toEqual([1800])
  })
  it("inserts silence", () => {
    const e = insertTime(toEdit(track()), 1950, 1000)
    expect(e.notes[1].start).toBe(3000)
    expect(e.notes[0].end).toBe(1900)
    expect(e.track.duration_ms).toBe(6000)
  })
  it("copies and pastes", () => {
    let e = toEdit(track())
    const clip = copyRange(e, 900, 2000)
    expect(clip.notes).toHaveLength(1)
    expect(clip.controls).toHaveLength(1)
    e = pasteClip(e, clip, 4000)
    expect(e.notes.filter((n) => n.key === 60).map((n) => n.start)).toEqual([1000, 4100])
    expect(new Set(e.notes.map((n) => n.id)).size).toBe(e.notes.length)
  })
})

describe("context menu edits", () => {
  it("copies notes and lays them over another place", () => {
    let e = toEdit(track())
    const clip = copyNotes(e, new Set([e.notes[0].id]))
    expect(clip.length).toBe(900)
    const [overlaid, ids] = overlayClip(e, clip, 3000)
    e = overlaid
    expect(e.notes).toHaveLength(3)
    expect(e.notes.find((n) => n.id === ids[0])).toMatchObject({ key: 60, start: 3000, end: 3900 })
    expect(e.track.duration_ms).toBe(5000)
  })
  it("flips a stop change", () => {
    const e = toEdit(track())
    const id = e.controls[0].id
    const c = flipStop(e, id).controls[0]
    expect(c.type === "stop" && c.on).toBe(false)
  })
})

describe("History", () => {
  it("undoes and redoes", () => {
    const h = new History<number>()
    h.push(1)
    h.push(2)
    expect(h.undo(3)).toBe(2)
    expect(h.undo(2)).toBe(1)
    expect(h.redo(1)).toBe(2)
    expect(h.canRedo).toBe(true)
    h.push(9)
    expect(h.canRedo).toBe(false)
  })
})
