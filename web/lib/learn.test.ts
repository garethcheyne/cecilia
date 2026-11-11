import { describe, expect, it } from "vitest"

import { describeBinding, keyboard, stopBinding, valueBinding } from "./learn"

describe("stopBinding", () => {
  it("learns a Makin toggle", () => {
    expect(stopBinding([[0xcb, 0x12]], [[0xcb, 0x12]])).toEqual({ type: "toggle", msg: "CB 12" })
  })
  it("learns a value switch", () => {
    expect(stopBinding([[0xb0, 0x50, 0x7f]], [[0xb0, 0x50, 0x00]])).toEqual({
      type: "switch",
      pattern: "B0 50 vv",
      on: 127,
      off: 0,
    })
  })
  it("learns separate on and off messages, ignoring keys", () => {
    expect(stopBinding([[0x90, 60, 80], [0xc0, 1]], [[0xc0, 2]])).toEqual({ type: "on_off", on: "C0 01", off: "C0 02" })
  })
  it("needs something to learn from", () => {
    expect(stopBinding([[0x90, 60, 80]], [])).toBeNull()
  })
})

describe("valueBinding", () => {
  it("finds the swell pedal's range", () => {
    const sweep = [55, 70, 90, 127, 100, 55].map((v) => [0xb1, 0x0b, v])
    expect(valueBinding([[0xb1, 0x07, 100], ...sweep])).toEqual({ pattern: "B1 0B vv", min: 55, max: 127 })
  })
  it("ignores a controller that barely moved", () => {
    expect(valueBinding([[0xb0, 7, 100], [0xb0, 7, 101]])).toBeNull()
  })
})

describe("keyboard", () => {
  it("reads channel and compass", () => {
    expect(keyboard([[0x91, 36, 90], [0x81, 36, 0], [0x91, 96, 90]])).toEqual({ channel: 2, low: 36, high: 96 })
  })
})

describe("describeBinding", () => {
  it("reads naturally", () => {
    expect(describeBinding({ type: "toggle", msg: "CB 12" })).toBe("Toggle CB 12")
    expect(describeBinding(undefined)).toBe("Not learned")
  })
})
