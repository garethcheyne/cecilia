import { describe, expect, it } from "vitest"

import type { MusicEntry } from "@/lib/api/client"
import {
  filterMusic,
  isFiltered,
  NO_FILTERS,
  occasions,
} from "@/lib/community-filter"

const entry = (
  id: string,
  title: string,
  tags: string[],
  extra: Partial<MusicEntry> = {}
): MusicEntry => ({
  id,
  path: `music/${id}.score.json`,
  kind: "score",
  title,
  composer: "",
  melody: id.toUpperCase(),
  licence: "public-domain",
  tags,
  verses: 4,
  duration_ms: 120_000,
  ...extra,
})

const music = [
  entry("adeste", "O Come, All Ye Faithful", ["hymn", "christmas"], {
    source_url: "https://openhymnal.org",
    duration_ms: 200_000,
  }),
  entry("cranham", "'In the Bleak Midwinter", ["hymn", "christmas"], {
    other_titles: ["Love Came Down at Christmas"],
  }),
  entry("eventide", "Abide with Me", ["hymn", "evening", "funeral"], {
    duration_ms: 90_000,
  }),
  entry("ground", "Prelude on a Ground", ["voluntary", "rugby"]),
]
const none = () => false

describe("filterMusic", () => {
  it("sorts by title, ignoring leading quotes", () => {
    expect(filterMusic(music, NO_FILTERS, none).map((m) => m.id)).toEqual([
      "eventide",
      "cranham",
      "adeste",
      "ground",
    ])
  })
  it("filters by occasion and kind", () => {
    expect(
      filterMusic(music, { ...NO_FILTERS, occasion: "christmas" }, none).map(
        (m) => m.id
      )
    ).toEqual(["cranham", "adeste"])
    expect(
      filterMusic(music, { ...NO_FILTERS, kind: "organ" }, none).map(
        (m) => m.id
      )
    ).toEqual(["ground"])
  })
  it("searches every word, including other titles", () => {
    expect(
      filterMusic(music, { ...NO_FILTERS, query: "love christmas" }, none).map(
        (m) => m.id
      )
    ).toEqual(["cranham"])
  })
  it("filters by source and hides pieces already added", () => {
    expect(
      filterMusic(music, { ...NO_FILTERS, source: "openhymnal" }, none).map(
        (m) => m.id
      )
    ).toEqual(["adeste"])
    const mine = (m: MusicEntry) => m.id === "eventide"
    expect(
      filterMusic(music, { ...NO_FILTERS, hideMine: true }, mine).map(
        (m) => m.id
      )
    ).not.toContain("eventide")
  })
  it("sorts by length", () => {
    expect(
      filterMusic(music, { ...NO_FILTERS, sort: "shortest" }, none)[0].id
    ).toBe("eventide")
    expect(
      filterMusic(music, { ...NO_FILTERS, sort: "longest" }, none)[0].id
    ).toBe("adeste")
  })
  it("knows when anything is filtered", () => {
    expect(isFiltered(NO_FILTERS)).toBe(false)
    expect(isFiltered({ ...NO_FILTERS, sort: "longest" })).toBe(false)
    expect(isFiltered({ ...NO_FILTERS, hideMine: true })).toBe(true)
  })
})

describe("occasions", () => {
  it("groups known occasions and keeps the rest under Other", () => {
    expect(occasions(music)).toEqual([
      { label: "Church year", tags: ["christmas"] },
      { label: "Services", tags: ["funeral", "evening"] },
      { label: "Other", tags: ["rugby"] },
    ])
  })
})
