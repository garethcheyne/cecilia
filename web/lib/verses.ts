import type { VersePlan } from "@/lib/api/client"

/**
 * Verses typed the way organists do (same rules as the server): a number
 * under 20 is a count ("4" = four verses); anything else lists verse numbers
 * ("12357", "1, 2, 3, 5, 7"). Empty = every recorded verse.
 */
export function parseVerses(
  text: string
): { ok: true; plan: VersePlan | null } | { ok: false } {
  const t = text.trim()
  if (!t) return { ok: true, plan: null }
  let list: number[]
  if (/[ ,]/.test(t)) {
    list = t.split(/[ ,]+/).filter(Boolean).map(Number)
  } else {
    if (!/^\d+$/.test(t)) return { ok: false }
    const n = Number(t)
    if (n < 20) return n === 0 ? { ok: false } : { ok: true, plan: n }
    list = [...t].map(Number)
  }
  if (list.some((v) => !Number.isInteger(v) || v < 1)) return { ok: false }
  return { ok: true, plan: list }
}

export function formatVerses(plan: VersePlan | null | undefined): string {
  if (plan == null) return ""
  if (typeof plan === "number") return String(plan)
  // Always separated, so [1, 4] never reads as "14" (fourteen verses).
  return plan.length === 1 ? `${plan[0]},` : plan.join(", ")
}
