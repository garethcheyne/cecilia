import { describe, expect, it } from "vitest"

import type { OrganDefinition } from "@/lib/api/client"
import { coupledKeys, keyboardSpeaks, starterStop } from "@/lib/sounding"

const voice = (
  id: string,
  division: string,
  family: string,
  footage: string
) => ({
  id,
  division,
  name: id,
  equivalent: { type: "voice", family, footage },
})

const organ = {
  stops: [
    voice("sw-oboe", "swell", "reed", "8"),
    voice("sw-gedackt", "swell", "flute", "8"),
    voice("gt-principal", "great", "principal", "4"),
    voice("gt-diapason", "great", "principal", "8"),
    voice("gt-flute", "great", "flute", "8"),
    voice("pd-principal", "pedal", "principal", "8"),
    voice("pd-subbass", "pedal", "flute", "16"),
    {
      id: "sw-gt",
      division: "great",
      name: "Swell to Great",
      equivalent: { type: "coupler", source: "swell", pitch: "unison" },
    },
    {
      id: "gt-uo",
      division: "great",
      name: "Unison off",
      equivalent: { type: "unison_off" },
    },
  ],
} as unknown as OrganDefinition

describe("keyboardSpeaks", () => {
  it("is silent with nothing drawn", () => {
    expect(keyboardSpeaks(organ, [], "great")).toBe(false)
  })
  it("speaks with one of its own stops drawn", () => {
    expect(keyboardSpeaks(organ, ["gt-flute"], "great")).toBe(true)
    expect(keyboardSpeaks(organ, ["gt-flute"], "swell")).toBe(false)
  })
  it("speaks through a coupler", () => {
    expect(keyboardSpeaks(organ, ["sw-gt", "sw-oboe"], "great")).toBe(true)
    expect(keyboardSpeaks(organ, ["sw-gt"], "great")).toBe(false)
  })
  it("is silent with unison off and no coupler", () => {
    expect(keyboardSpeaks(organ, ["gt-uo", "gt-flute"], "great")).toBe(false)
  })
})

describe("starterStop", () => {
  it("picks a soft 8′ flute on a manual", () => {
    expect(starterStop(organ, "swell")?.id).toBe("sw-gedackt")
    expect(starterStop(organ, "great")?.id).toBe("gt-flute")
  })
  it("picks the 16′ on the pedals", () => {
    expect(starterStop(organ, "pedal")?.id).toBe("pd-subbass")
  })
  it("has nothing for a keyboard without stops", () => {
    expect(starterStop(organ, "choir")).toBeNull()
  })
})

describe("coupledKeys", () => {
  it("lights the Swell keys a Swell to Great coupler plays", () => {
    const c = coupledKeys(organ, ["sw-gt"], { great: [60, 64] })
    expect([...(c.get("swell") ?? [])]).toEqual([60, 64])
  })
  it("shifts octave couplers", () => {
    const super4 = {
      stops: [
        {
          id: "sw-gt4",
          division: "great",
          equivalent: { type: "coupler", source: "swell", pitch: "super" },
        },
      ],
    } as unknown as OrganDefinition
    expect([
      ...(coupledKeys(super4, ["sw-gt4"], { great: [60] }).get("swell") ?? []),
    ]).toEqual([72])
  })
  it("does nothing with the coupler off", () => {
    expect(coupledKeys(organ, [], { great: [60] }).size).toBe(0)
  })
})
