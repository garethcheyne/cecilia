/**
 * MIDI learn: working out an organ's MIDI from what it sends while the
 * organist pulls stops, plays keys and moves pedals.
 */
import type { Schemas } from "@/lib/api/client"

export type StopBinding = Schemas["StopBinding"]
export type ValueBinding = Schemas["ValueBinding"]

export const hex = (bytes: number[]) => bytes.map((b) => b.toString(16).padStart(2, "0").toUpperCase()).join(" ")

export const isNote = (m: number[]) => m.length === 3 && (m[0] & 0xe0) === 0x80

/** The first message that isn't a key going down or up. */
export function firstControl(msgs: number[][]): number[] | null {
  return msgs.find((m) => !isNote(m) && m[0] !== 0xfe && m[0] !== 0xf8) ?? null
}

/**
 * The binding for a stop from what it sent when drawn (`on`) and pushed in
 * (`off`): the same message both times is a toggle; messages differing in
 * one data byte are a switch with that byte as the value; anything else is
 * a pair of on and off messages.
 */
export function stopBinding(on: number[][], off: number[][]): StopBinding | null {
  const a = firstControl(on)
  const b = firstControl(off)
  if (!a || !b) return null
  if (hex(a) === hex(b)) return { type: "toggle", msg: hex(a) }
  if (a.length === b.length && a[0] === b[0]) {
    const diff = a.map((x, i) => (x !== b[i] ? i : -1)).filter((i) => i >= 0)
    // Only a trailing value byte (e.g. a controller's value) makes a switch;
    // two program changes are two different messages, not one with a value.
    if (diff.length === 1 && a.length >= 3 && diff[0] === a.length - 1) {
      const i = diff[0]
      const pattern = a.map((x, j) => (j === i ? "vv" : x.toString(16).padStart(2, "0").toUpperCase())).join(" ")
      return { type: "switch", pattern, on: a[i], off: b[i] }
    }
  }
  return { type: "on_off", on: hex(a), off: hex(b) }
}

/** A pedal swept through its range: the controller that moved, and how far. */
export function valueBinding(msgs: number[][]): ValueBinding | null {
  const counts = new Map<string, { status: number; cc: number; values: number[] }>()
  for (const m of msgs) {
    if (m.length !== 3 || (m[0] & 0xf0) !== 0xb0) continue
    const k = `${m[0]}:${m[1]}`
    const c = counts.get(k) ?? { status: m[0], cc: m[1], values: [] }
    c.values.push(m[2])
    counts.set(k, c)
  }
  const best = [...counts.values()].sort((x, y) => y.values.length - x.values.length)[0]
  if (!best || best.values.length < 2) return null
  const min = Math.min(...best.values)
  const max = Math.max(...best.values)
  if (max - min < 4) return null
  const h = (n: number) => n.toString(16).padStart(2, "0").toUpperCase()
  return { pattern: `${h(best.status)} ${h(best.cc)} vv`, min, max }
}

/** A keyboard from its lowest and highest notes being played. */
export function keyboard(msgs: number[][]): { channel: number; low: number; high: number } | null {
  const ons = msgs.filter((m) => isNote(m) && (m[0] & 0xf0) === 0x90 && m[2] > 0)
  if (ons.length === 0) return null
  const channel = (ons[0][0] & 0x0f) + 1
  const keys = ons.filter((m) => (m[0] & 0x0f) + 1 === channel).map((m) => m[1])
  return { channel, low: Math.min(...keys), high: Math.max(...keys) }
}

/** Human summary of a binding, e.g. "Toggle CB 12". */
export function describeBinding(b: StopBinding | undefined): string {
  if (!b) return "Not learned"
  switch (b.type) {
    case "toggle":
      return `Toggle ${b.msg}`
    case "on_off":
      return `On ${b.on} · Off ${b.off}`
    case "switch":
      return `${b.pattern} (on ${b.on}, off ${b.off})`
  }
}
