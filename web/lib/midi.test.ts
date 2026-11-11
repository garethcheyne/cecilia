import { describe, expect, it } from "vitest"

import { describeMidi, formatDuration, noteName, toHex } from "./midi"

describe("noteName", () => {
  it("names middle C C4", () => {
    expect(noteName(60)).toBe("C4")
    expect(noteName(36)).toBe("C2")
    expect(noteName(61)).toBe("C♯4")
  })
})

describe("describeMidi", () => {
  it("treats note-on with velocity 0 as note-off", () => {
    expect(describeMidi([0x90, 60, 0]).kind).toBe("note-off")
    expect(describeMidi([0x91, 60, 64])).toMatchObject({
      kind: "note-on",
      channel: 2,
    })
  })
  it("recognises Makin stop toggles and general cancel", () => {
    expect(describeMidi([0xcb, 0x12])).toMatchObject({
      kind: "program",
      channel: 12,
    })
    expect(
      describeMidi([0xf0, 0x00, 0x4a, 0x4f, 0x48, 0x41, 0x53, 0x7f, 0xf7]).kind
    ).toBe("sysex")
  })
})

describe("formatting", () => {
  it("formats hex and durations", () => {
    expect(toHex([0xb1, 0x0b, 0x7f])).toBe("B1 0B 7F")
    expect(formatDuration(0)).toBe("0:00.0")
    expect(formatDuration(83_450)).toBe("1:23.4")
  })
})
