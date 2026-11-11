import type { DivisionRole, OrganDefinition, Stop } from "@/lib/api/client"

/**
 * Would pressing a key on this keyboard make any sound? Mirrors the server's
 * `ConsoleState::sounding`: the keyboard's own speaking stops (unless unison
 * off is drawn), plus the speaking stops of any division coupled in.
 */
export function keyboardSpeaks(organ: OrganDefinition, stopsOn: readonly string[], keyboard: DivisionRole): boolean {
  const on = new Set(stopsOn)
  const drawn = organ.stops.filter((s) => on.has(s.id))
  const sources = new Set<DivisionRole>()
  if (!drawn.some((s) => s.division === keyboard && s.equivalent.type === "unison_off")) sources.add(keyboard)
  for (const s of drawn) {
    if (s.division === keyboard && s.equivalent.type === "coupler") sources.add(s.equivalent.source)
  }
  return drawn.some((s) => sources.has(s.division) && s.equivalent.type === "voice")
}

const PREFERENCE = ["flute", "principal", "string"]

/**
 * A gentle stop to draw so a silent keyboard can be heard: an 8′ flute
 * (16′ on the pedals), falling back to the nearest foundation stop.
 */
export function starterStop(organ: OrganDefinition, keyboard: DivisionRole): Stop | null {
  const voices = organ.stops.filter((s) => s.division === keyboard && s.equivalent.type === "voice")
  const footage = keyboard === "pedal" ? ["16", "8"] : ["8", "4"]
  for (const f of footage) {
    for (const family of PREFERENCE) {
      const hit = voices.find(
        (s) => s.equivalent.type === "voice" && s.equivalent.footage.trim() === f && s.equivalent.family === family
      )
      if (hit) return hit
    }
  }
  return voices[0] ?? null
}

const COUPLER_SHIFT = { sub: -12, unison: 0, super: 12 } as const

/**
 * Keys that couplers are playing on other keyboards, by keyboard. With Swell
 * to Great drawn, holding Great C4 plays Swell C4 (C5 for a 4′ coupler), as
 * the keys of a mechanical console would move.
 */
export function coupledKeys(
  organ: OrganDefinition,
  stopsOn: readonly string[],
  notes: Readonly<Record<string, readonly number[] | undefined>>
): Map<DivisionRole, Set<number>> {
  const on = new Set(stopsOn)
  const out = new Map<DivisionRole, Set<number>>()
  for (const s of organ.stops) {
    if (!on.has(s.id) || s.equivalent.type !== "coupler") continue
    const shift = COUPLER_SHIFT[s.equivalent.pitch]
    for (const key of notes[s.division] ?? []) {
      const k = key + shift
      if (k < 0 || k > 127) continue
      const set = out.get(s.equivalent.source) ?? new Set<number>()
      set.add(k)
      out.set(s.equivalent.source, set)
    }
  }
  return out
}
