"use client"

import * as React from "react"

/** Piano-style layout: the home row is the white keys, the row above the black keys. */
const KEY_OFFSETS: Record<string, number> = {
  a: 0, w: 1, s: 2, e: 3, d: 4, f: 5, t: 6, g: 7, y: 8, h: 9, u: 10, j: 11,
  k: 12, o: 13, l: 14, p: 15, ";": 16,
}

function isTyping(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
  )
}

/**
 * Play notes from the computer keyboard. Z / X shift the octave.
 * Returns the current base note (C of the playing octave).
 */
export function useComputerKeyboard(
  enabled: boolean,
  onNote: (note: number, on: boolean) => void
): { base: number; setBase: (n: number) => void } {
  const [base, setBase] = React.useState(60)
  const held = React.useRef(new Map<string, number>())
  const onNoteRef = React.useRef(onNote)
  React.useEffect(() => {
    onNoteRef.current = onNote
  }, [onNote])

  React.useEffect(() => {
    if (!enabled) return
    const pressed = held.current

    const down = (e: KeyboardEvent) => {
      if (e.repeat || e.metaKey || e.ctrlKey || e.altKey || isTyping(e.target)) return
      const k = e.key.toLowerCase()
      if (k === "z") return setBase((b) => Math.max(24, b - 12))
      if (k === "x") return setBase((b) => Math.min(96, b + 12))
      const offset = KEY_OFFSETS[k]
      if (offset === undefined || pressed.has(k)) return
      e.preventDefault()
      const note = base + offset
      pressed.set(k, note)
      onNoteRef.current(note, true)
    }
    const up = (e: KeyboardEvent) => {
      const k = e.key.toLowerCase()
      const note = pressed.get(k)
      if (note === undefined) return
      pressed.delete(k)
      onNoteRef.current(note, false)
    }
    // Don't leave notes sounding when the window loses focus.
    const blur = () => {
      pressed.forEach((note) => onNoteRef.current(note, false))
      pressed.clear()
    }
    window.addEventListener("keydown", down)
    window.addEventListener("keyup", up)
    window.addEventListener("blur", blur)
    return () => {
      window.removeEventListener("keydown", down)
      window.removeEventListener("keyup", up)
      window.removeEventListener("blur", blur)
      blur()
    }
  }, [enabled, base])

  return { base, setBase }
}
