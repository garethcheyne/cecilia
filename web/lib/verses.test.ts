import { describe, expect, it } from "vitest"

import { formatVerses, parseVerses } from "./verses"

describe("parseVerses", () => {
  it("reads counts and lists like the original", () => {
    expect(parseVerses("")).toEqual({ ok: true, plan: null })
    expect(parseVerses("4")).toEqual({ ok: true, plan: 4 })
    expect(parseVerses("12")).toEqual({ ok: true, plan: 12 })
    expect(parseVerses("12357")).toEqual({ ok: true, plan: [1, 2, 3, 5, 7] })
    expect(parseVerses("1, 2, 4")).toEqual({ ok: true, plan: [1, 2, 4] })
  })
  it("rejects nonsense", () => {
    expect(parseVerses("0").ok).toBe(false)
    expect(parseVerses("1x").ok).toBe(false)
    expect(parseVerses("1, 0").ok).toBe(false)
  })
  it("round-trips", () => {
    expect(formatVerses([1, 2, 3, 5, 7])).toBe("1, 2, 3, 5, 7")
    expect(formatVerses([1, 4])).toBe("1, 4")
    expect(parseVerses(formatVerses([1, 4]))).toEqual({ ok: true, plan: [1, 4] })
    expect(parseVerses(formatVerses([2]))).toEqual({ ok: true, plan: [2] })
    expect(formatVerses(4)).toBe("4")
    expect(formatVerses(null)).toBe("")
  })
})
