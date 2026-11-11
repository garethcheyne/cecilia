import type { OrganEntry } from "@/lib/api/client"

export interface Maker {
  name: string
  models: number
}

export interface Model {
  model: string
  series: string
  /** One per variant; most models have just one. */
  setups: OrganEntry[]
}

const key = (s: string) => s.trim().toLocaleLowerCase()

/** Makers in the shared library, most models first. */
export function makers(organs: OrganEntry[]): Maker[] {
  const byMaker = new Map<string, { name: string; models: Set<string> }>()
  for (const o of organs) {
    const k = key(o.meta.maker)
    const m = byMaker.get(k) ?? {
      name: o.meta.maker.trim(),
      models: new Set<string>(),
    }
    m.models.add(key(o.meta.model))
    byMaker.set(k, m)
  }
  return [...byMaker.values()]
    .map((m) => ({ name: m.name, models: m.models.size }))
    .sort((a, b) => b.models - a.models || a.name.localeCompare(b.name))
}

/** A maker's models, grouped by series, each with its variants. */
export function models(
  organs: OrganEntry[],
  maker: string
): { series: string; models: Model[] }[] {
  const byModel = new Map<string, Model>()
  for (const o of organs) {
    if (key(o.meta.maker) !== key(maker)) continue
    const k = key(o.meta.model)
    const m = byModel.get(k) ?? {
      model: o.meta.model.trim(),
      series: (o.meta.series ?? "").trim(),
      setups: [],
    }
    m.setups.push(o)
    byModel.set(k, m)
  }
  const bySeries = new Map<string, Model[]>()
  for (const m of byModel.values()) {
    m.setups.sort((a, b) =>
      (a.meta.variant ?? "").localeCompare(b.meta.variant ?? "")
    )
    bySeries.set(m.series, [...(bySeries.get(m.series) ?? []), m])
  }
  const naturally = (a: string, b: string) =>
    a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" })
  return [...bySeries.entries()]
    .sort(([a], [b]) => (a === "" ? 1 : b === "" ? -1 : naturally(a, b)))
    .map(([series, ms]) => ({
      series,
      models: ms.sort((a, b) => naturally(a.model, b.model)),
    }))
}

/** Setups whose maker, model, series, variant or other names contain every word. */
export function searchOrgans(
  organs: OrganEntry[],
  query: string
): OrganEntry[] {
  const words = key(query).split(/\s+/).filter(Boolean)
  if (!words.length) return []
  return organs.filter((o) => {
    const text = key(
      [
        o.meta.maker,
        o.meta.model,
        o.meta.series ?? "",
        o.meta.variant ?? "",
        ...(o.meta.also_known_as ?? []),
      ].join(" ")
    )
    return words.every((w) => text.includes(w))
  })
}

export function organTitle(o: OrganEntry): string {
  return `${o.meta.maker} ${o.meta.model}`.trim()
}
