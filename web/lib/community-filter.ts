import type { MusicEntry } from "@/lib/api/client"

/** Occasions, grouped the way an organist plans: the church year, then services. */
export const OCCASION_GROUPS = [
  {
    label: "Church year",
    tags: [
      "advent",
      "christmas",
      "epiphany",
      "lent",
      "easter",
      "ascension",
      "pentecost",
      "trinity",
      "saints",
      "harvest",
      "remembrance",
      "new year",
    ],
  },
  {
    label: "Services",
    tags: [
      "communion",
      "baptism",
      "wedding",
      "funeral",
      "marian",
      "morning",
      "evening",
      "mission",
      "praise",
    ],
  },
] as const

/** Tags that say what kind of piece it is, not when it's used. */
const KIND_TAGS = new Set(["hymn", "psalm", "voluntary", "prelude"])

export type Kind = "all" | "hymns" | "organ"
export type Sort = "title" | "tune" | "shortest" | "longest"
export type SourceFilter = "all" | "openhymnal" | "cecilia"

export interface Filters {
  query: string
  occasion: string | null
  kind: Kind
  source: SourceFilter
  hideMine: boolean
  sort: Sort
}

export const NO_FILTERS: Filters = {
  query: "",
  occasion: null,
  kind: "all",
  source: "all",
  hideMine: false,
  sort: "title",
}

export function isFiltered(f: Filters): boolean {
  return (
    f.query.trim() !== "" ||
    f.occasion !== null ||
    f.kind !== "all" ||
    f.source !== "all" ||
    f.hideMine
  )
}

/** Occasions found in the catalogue: the known groups, then anything else. */
export function occasions(
  music: MusicEntry[]
): { label: string; tags: string[] }[] {
  const present = new Set(
    music.flatMap((m) => m.tags).filter((t) => !KIND_TAGS.has(t))
  )
  const known = new Set<string>(OCCASION_GROUPS.flatMap((g) => g.tags))
  const groups = OCCASION_GROUPS.map((g) => ({
    label: g.label,
    tags: g.tags.filter((t) => present.has(t)),
  }))
  const other = [...present].filter((t) => !known.has(t)).sort()
  return [...groups, { label: "Other", tags: other }].filter(
    (g) => g.tags.length > 0
  )
}

export function isHymn(m: MusicEntry): boolean {
  return m.tags.includes("hymn") || m.tags.includes("psalm")
}

export function sourceOf(m: MusicEntry): SourceFilter {
  return m.source_url?.includes("openhymnal") ? "openhymnal" : "cecilia"
}

/** For sorting: no leading quotes or punctuation, case-blind. */
export function sortKey(s: string): string {
  return s.replace(/^[^\p{L}\p{N}]+/u, "").toLocaleLowerCase()
}

export function filterMusic(
  music: MusicEntry[],
  f: Filters,
  mine: (m: MusicEntry) => boolean
): MusicEntry[] {
  const words = f.query.trim().toLowerCase().split(/\s+/).filter(Boolean)
  const shown = music.filter((m) => {
    if (f.occasion && !m.tags.includes(f.occasion)) return false
    if (f.kind === "hymns" && !isHymn(m)) return false
    if (f.kind === "organ" && isHymn(m)) return false
    if (f.source !== "all" && sourceOf(m) !== f.source) return false
    if (f.hideMine && mine(m)) return false
    if (words.length) {
      const text = [
        m.title,
        m.composer,
        m.melody,
        ...(m.other_titles ?? []),
        ...m.tags,
      ]
        .join(" ")
        .toLowerCase()
      if (!words.every((w) => text.includes(w))) return false
    }
    return true
  })
  const by = {
    title: (a: MusicEntry, b: MusicEntry) =>
      sortKey(a.title).localeCompare(sortKey(b.title)),
    tune: (a: MusicEntry, b: MusicEntry) =>
      sortKey(a.melody || a.title).localeCompare(sortKey(b.melody || b.title)),
    shortest: (a: MusicEntry, b: MusicEntry) => a.duration_ms - b.duration_ms,
    longest: (a: MusicEntry, b: MusicEntry) => b.duration_ms - a.duration_ms,
  }[f.sort]
  return shown.sort(by)
}
