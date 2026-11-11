import { describe, expect, it } from "vitest"

import { DEFAULT_DISPLAYS, displayUrl, panelsFromQuery, parseDisplays, shown } from "@/lib/displays"

describe("parseDisplays", () => {
  it("falls back to the defaults", () => {
    expect(parseDisplays(null)).toEqual(DEFAULT_DISPLAYS)
    expect(parseDisplays("not json")).toEqual(DEFAULT_DISPLAYS)
  })
  it("keeps good values and repairs bad ones", () => {
    const d = parseDisplays(JSON.stringify({ layout: "split", panels: ["console", "order"], secondPanels: ["nope", 3] }))
    expect(d.layout).toBe("split")
    expect(d.panels).toEqual(["console", "order"])
    expect(d.secondPanels).toEqual(DEFAULT_DISPLAYS.secondPanels)
  })
})

describe("shown", () => {
  it("shows one panel, or two when split", () => {
    expect(shown("single", ["order", "music"])).toEqual(["order"])
    expect(shown("split", ["order", "music"])).toEqual(["order", "music"])
  })
  it("doesn't show the same panel twice", () => {
    expect(shown("split", ["music", "music"])).toEqual(["music"])
  })
})

describe("display addresses", () => {
  it("round-trips panels", () => {
    expect(displayUrl(["console", "music"])).toBe("/service/display/?panels=console,music")
    expect(panelsFromQuery("console,music")).toEqual(["console", "music"])
  })
  it("ignores unknown panels and repeats", () => {
    expect(panelsFromQuery("console,bogus,console")).toEqual(["console"])
    expect(panelsFromQuery(null)).toEqual(["order"])
  })
})
