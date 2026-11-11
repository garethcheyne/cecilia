import { describe, expect, it } from "vitest"

import { uniqueName } from "@/components/organs/organ-names"

describe("uniqueName", () => {
  it("keeps a free name", () => {
    expect(uniqueName("Our organ", ["Cecilia Demo Organ"])).toBe("Our organ")
  })
  it("numbers a name that's taken, ignoring case", () => {
    expect(uniqueName("Our organ", ["our organ"])).toBe("Our organ 2")
    expect(uniqueName("Our organ", ["Our organ", "Our organ 2"])).toBe(
      "Our organ 3"
    )
  })
})
