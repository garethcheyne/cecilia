"use client"

import * as React from "react"

export const TEXT_SIZES = [
  { value: "normal", label: "Normal", scale: "100%" },
  { value: "large", label: "Large", scale: "112.5%" },
  { value: "larger", label: "Larger", scale: "125%" },
] as const

export type TextSize = (typeof TEXT_SIZES)[number]["value"]

const KEY = "cecilia-text-size"

export function readTextSize(): TextSize {
  try {
    const v = localStorage.getItem(KEY)
    return TEXT_SIZES.some((t) => t.value === v) ? (v as TextSize) : "normal"
  } catch {
    return "normal"
  }
}

export function applyTextSize(size: TextSize) {
  document.documentElement.style.fontSize = TEXT_SIZES.find(
    (t) => t.value === size
  )!.scale
  try {
    localStorage.setItem(KEY, size)
  } catch {
    // Private browsing: just this visit.
  }
}

/** Applies the reader's chosen text size (kept per device). */
export function TextSizeEffect() {
  React.useEffect(() => applyTextSize(readTextSize()), [])
  return null
}
