import { describe, expect, it } from "vitest"

import { formatClock } from "./time"

describe("formatClock", () => {
  it("formats minutes and seconds", () => {
    expect(formatClock(0)).toBe("0:00")
    expect(formatClock(7_900)).toBe("0:07")
    expect(formatClock(205_000)).toBe("3:25")
  })
  it("supports pieces over an hour", () => {
    expect(formatClock(3_729_000)).toBe("1:02:09")
  })
  it("never goes negative", () => {
    expect(formatClock(-500)).toBe("0:00")
  })
})
