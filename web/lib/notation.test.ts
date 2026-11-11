import { describe, expect, it } from "vitest"

import type { Track } from "@/lib/api/client"

import { durationParts, gridFor, keyName, layout, spell } from "./notation"

function track(): Track {
  // 120 bpm: a crotchet is 500 ms, a semiquaver 125 ms. Downbeat at 1000 ms.
  const note = (
    t: number,
    key: number,
    v: number,
    division: "great" | "pedal" = "great"
  ) => ({ t, type: "note", division, key, velocity: v }) as const
  return {
    schema: 1,
    id: "t",
    info: { title: "T", volume: 100 },
    source: "sample",
    created: 0,
    modified: 0,
    initial: { stops: [], expression: {} },
    duration_ms: 5000,
    meter: {
      bpm: 120,
      beats: 4,
      beat_unit: 4,
      fifths: 1,
      mode: "major",
      offset_ms: 1000,
    },
    events: [
      note(1000, 67, 80), // G4 minim
      note(1000, 43, 80, "pedal"), // G2 semibreve in the pedal
      note(1000, 59, 80), // B3 crotchet (bass staff)
      note(1465, 59, 0),
      note(1965, 67, 0),
      note(2000, 69, 80), // A4 crotchets
      note(2465, 69, 0),
      note(2500, 71, 80),
      note(2965, 71, 0),
      note(2965, 43, 0, "pedal"),
    ],
  }
}

describe("layout", () => {
  it("splits staves and slices a bar", () => {
    const t = track()
    const g = gridFor(t)
    const { measures, staves } = layout(t, g)
    expect(staves).toEqual(["treble", "bass", "pedal"])
    expect(measures).toHaveLength(1)
    const treble = measures[0].staves.treble
    expect(treble.map((s) => [s.start, s.len, s.keys])).toEqual([
      [0, 8, [67]],
      [8, 4, [69]],
      [12, 4, [71]],
    ])
    expect(measures[0].staves.bass.map((s) => [s.len, s.keys])).toEqual([
      [4, [59]],
      [12, []],
    ])
    expect(measures[0].staves.pedal.map((s) => [s.len, s.keys])).toEqual([
      [16, [43]],
    ])
  })

  it("ties notes that span a cut", () => {
    const t = track()
    // Overlap: the A starts while the G is still held.
    t.events = t.events.map((e) =>
      e.type === "note" && e.key === 67 && e.velocity === 0
        ? { ...e, t: 2465 }
        : e
    )
    const { measures } = layout(t, gridFor(t))
    const s = measures[0].staves.treble
    expect(s[1].keys).toEqual([67, 69])
    expect(s[1].tiedIn).toEqual([67])
  })

  it("guesses a grid for live recordings", () => {
    const t = track()
    delete t.meter
    const g = gridFor(t)
    expect(g.approximate).toBe(true)
    expect(g.offsetMs).toBe(1000)
  })
})

describe("helpers", () => {
  it("breaks lengths into note values", () => {
    expect(durationParts(16)).toEqual([{ duration: "w", dots: 0, len: 16 }])
    expect(durationParts(6)).toEqual([{ duration: "q", dots: 1, len: 6 }])
    expect(durationParts(5).map((p) => p.duration)).toEqual(["q", "16"])
  })
  it("spells for the key", () => {
    expect(spell(66, 1)).toBe("f#/4")
    expect(spell(70, -2)).toBe("bb/4")
    expect(keyName(1)).toBe("G")
    expect(keyName(-3)).toBe("Eb")
  })
})
