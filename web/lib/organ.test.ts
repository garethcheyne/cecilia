import { describe, expect, it } from "vitest"

import { formatFootage } from "./organ"

describe("formatFootage", () => {
  it("adds a foot mark", () => {
    expect(formatFootage("8")).toBe("8′")
    expect(formatFootage("16")).toBe("16′")
  })
  it("uses vulgar fractions for mutations", () => {
    expect(formatFootage("2 2/3")).toBe("2⅔′")
    expect(formatFootage("1 3/5")).toBe("1⅗′")
    expect(formatFootage("1 1/3")).toBe("1⅓′")
  })
})
