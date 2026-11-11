import { describe, expect, it } from "vitest"

import type { OrganEntry } from "@/lib/api/client"
import { makers, models, searchOrgans } from "@/lib/organ-catalogue"

const organ = (
  id: string,
  maker: string,
  model: string,
  extra: Partial<OrganEntry["meta"]> = {}
): OrganEntry => ({
  id,
  path: `organs/${id}.organ.json`,
  name: `${maker} ${model}`,
  divisions: 3,
  stops: 30,
  meta: {
    maker,
    model,
    years: "",
    confidence: "template",
    notes: [],
    contributor: "",
    licence: "CC0-1.0",
    ...extra,
  },
})

const organs = [
  organ("s170", "Johannus", "Studio 170", { series: "Studio" }),
  organ("s170-solo", "Johannus", "Studio 170", {
    series: "Studio",
    variant: "with Solo division",
  }),
  organ("s350", "Johannus", "Studio 350", { series: "Studio" }),
  organ("op10", "Johannus", "Opus 10", { series: "Opus" }),
  organ("west", "Makin", "Westmorland", { also_known_as: ["Westmorland 2"] }),
  organ("west2", "makin ", "Westmorland 3"),
]

describe("makers", () => {
  it("counts models, not variants, and ignores case", () => {
    expect(makers(organs)).toEqual([
      { name: "Johannus", models: 3 },
      { name: "Makin", models: 2 },
    ])
  })
})

describe("models", () => {
  it("groups by series in natural order, with each model's variants", () => {
    const groups = models(organs, "Johannus")
    expect(groups.map((g) => g.series)).toEqual(["Opus", "Studio"])
    const studio = groups[1].models
    expect(studio.map((m) => m.model)).toEqual(["Studio 170", "Studio 350"])
    expect(studio[0].setups.map((s) => s.id)).toEqual(["s170", "s170-solo"])
  })
})

describe("searchOrgans", () => {
  it("finds by maker, model, variant and other names", () => {
    expect(searchOrgans(organs, "johannus 170").map((o) => o.id)).toEqual([
      "s170",
      "s170-solo",
    ])
    expect(searchOrgans(organs, "solo").map((o) => o.id)).toEqual(["s170-solo"])
    expect(searchOrgans(organs, "westmorland 2").map((o) => o.id)).toEqual([
      "west",
    ])
    expect(searchOrgans(organs, "")).toEqual([])
  })
})
