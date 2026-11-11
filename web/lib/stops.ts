import type { DivisionRole, OrganDefinition, StopRef } from "@/lib/api/client"
import { formatFootage } from "@/lib/organ"

const DIVISION: Record<DivisionRole, string> = {
  pedal: "Pedal",
  great: "Great",
  swell: "Swell",
  choir: "Choir",
  solo: "Solo",
  echo: "Echo",
}

export function divisionName(
  d: DivisionRole,
  organ?: OrganDefinition | null
): string {
  return organ?.divisions.find((x) => x.role === d)?.name ?? DIVISION[d]
}

function sameEquivalent(
  a: StopRef["equivalent"],
  b: StopRef["equivalent"]
): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

/** The stop on `organ` that is exactly this one, if any. */
export function findStop(ref: StopRef, organ?: OrganDefinition | null) {
  return organ?.stops.find(
    (s) =>
      s.division === ref.division &&
      sameEquivalent(s.equivalent, ref.equivalent)
  )
}

/** "Great Open Diapason 8′", or a generic name when this organ lacks it. */
export function stopLabel(
  ref: StopRef,
  organ?: OrganDefinition | null
): string {
  const d = divisionName(ref.division, organ)
  const exact = findStop(ref, organ)
  if (exact) {
    const name = exact.full_name ?? exact.names.join(" ")
    return exact.equivalent.type === "coupler" ? name : `${d} ${name}`
  }
  const eq = ref.equivalent
  switch (eq.type) {
    case "voice": {
      const family = eq.tag || eq.family[0].toUpperCase() + eq.family.slice(1)
      return `${d} ${family} ${formatFootage(eq.footage)}`
    }
    case "coupler":
      return `${divisionName(eq.source, organ)} to ${d}${eq.pitch === "unison" ? "" : eq.pitch === "sub" ? " 16′" : " 4′"}`
    case "tremulant":
      return `${d} Tremulant`
    case "unison_off":
      return `${d} Unison Off`
  }
}

export function stopRefOf(
  organ: OrganDefinition,
  stopId: string
): StopRef | null {
  const s = organ.stops.find((x) => x.id === stopId)
  return s ? { division: s.division, equivalent: s.equivalent } : null
}
