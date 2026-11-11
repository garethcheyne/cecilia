import type { DivisionRole, Stop } from "@/lib/api/client"

/** Manuals from the top of the console down, then the pedalboard. */
export const CONSOLE_ORDER: DivisionRole[] = [
  "echo",
  "solo",
  "swell",
  "great",
  "choir",
  "pedal",
]

/** Fixed division colours (the same on every organ). Static strings so Tailwind sees them. */
export const DIVISION_BG: Record<DivisionRole, string> = {
  pedal: "bg-division-pedal",
  great: "bg-division-great",
  swell: "bg-division-swell",
  choir: "bg-division-choir",
  solo: "bg-division-solo",
  echo: "bg-division-echo",
}

export const DIVISION_RING: Record<DivisionRole, string> = {
  pedal: "ring-division-pedal",
  great: "ring-division-great",
  swell: "ring-division-swell",
  choir: "ring-division-choir",
  solo: "ring-division-solo",
  echo: "ring-division-echo",
}

export type StopKind = "voice" | "reed" | "coupler" | "accessory"

export function stopKind(stop: Stop): StopKind {
  const eq = stop.equivalent
  if (eq.type === "coupler") return "coupler"
  if (eq.type === "voice") return eq.family === "reed" ? "reed" : "voice"
  return "accessory"
}

const BLACK = new Set([1, 3, 6, 8, 10])
export const isBlackKey = (note: number) => BLACK.has(note % 12)
