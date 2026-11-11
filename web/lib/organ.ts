const FRACTIONS: Record<string, string> = {
  "1/2": "½",
  "1/3": "⅓",
  "2/3": "⅔",
  "1/4": "¼",
  "3/5": "⅗",
  "1/5": "⅕",
  "4/5": "⅘",
  "1/7": "⅐",
  "1/8": "⅛",
}

/** Footage as engraved on a stop: "8" -> "8′", "2 2/3" -> "2⅔′". */
export function formatFootage(footage: string): string {
  const parts = footage.trim().split(/\s+/).map((p) => FRACTIONS[p] ?? p)
  return `${parts.join("")}′`
}
