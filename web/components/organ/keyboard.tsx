"use client"

import * as React from "react"

import { noteName } from "@/lib/midi"
import { cn } from "@/lib/utils"

import { isBlackKey } from "./divisions"

/** Minimum white-key width so small screens get a playable, scrollable keyboard. */
const MIN_WHITE_REM = 1.45

/**
 * A playable keyboard (or pedalboard). Keys light up in the division's colour
 * when held, whether pressed here or by MIDI from elsewhere.
 */
export function Keyboard({
  low,
  high,
  held,
  colorClass,
  pedal = false,
  label,
  onNote,
}: {
  low: number
  high: number
  held: ReadonlySet<number>
  /** e.g. "bg-division-great" */
  colorClass: string
  pedal?: boolean
  label: string
  onNote: (key: number, on: boolean) => void
}) {
  const keys = React.useMemo(() => Array.from({ length: high - low + 1 }, (_, i) => low + i), [low, high])
  const whites = keys.filter((k) => !isBlackKey(k))
  const whiteIndex = new Map(whites.map((k, i) => [k, i]))
  const down = React.useRef(new Set<number>())

  const press = (key: number) => {
    if (down.current.has(key)) return
    down.current.add(key)
    onNote(key, true)
  }
  const release = (key: number) => {
    if (!down.current.delete(key)) return
    onNote(key, false)
  }

  const handlers = (key: number) => ({
    onPointerDown: (e: React.PointerEvent) => {
      e.preventDefault()
      ;(e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId)
      press(key)
    },
    onPointerUp: () => release(key),
    onPointerCancel: () => release(key),
    onPointerLeave: () => release(key),
    // Slide across keys with the button held (glissando).
    onPointerEnter: (e: React.PointerEvent) => {
      if (e.buttons === 1) press(key)
    },
  })

  const whiteWidth = 100 / whites.length

  // When the keyboard is wider than the screen, open it centred on middle C
  // (or the middle of the compass) rather than at the bottom note.
  const scroller = React.useRef<HTMLDivElement>(null)
  React.useEffect(() => {
    const el = scroller.current
    if (!el || el.scrollWidth <= el.clientWidth) return
    const centreKey = low <= 60 && high >= 60 ? 60 : Math.round((low + high) / 2)
    const idx = whites.findIndex((k) => k >= centreKey)
    const x = (Math.max(idx, 0) / whites.length) * el.scrollWidth
    el.scrollLeft = x - el.clientWidth / 2
    // Only on mount / compass change, never while playing.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [low, high])

  return (
    <div ref={scroller} className="overflow-x-auto overscroll-x-contain rounded-lg bg-black/40 p-1 [scrollbar-width:thin]">
      <div
        role="group"
        aria-label={label}
        className={cn("relative flex w-full gap-px touch-none select-none", pedal ? "h-16" : "h-28 sm:h-32")}
        style={{ minWidth: `${whites.length * MIN_WHITE_REM}rem` }}
      >
        {whites.map((key) => {
          const isHeld = held.has(key)
          return (
            <div
              key={key}
              role="button"
              aria-label={noteName(key)}
              aria-pressed={isHeld}
              {...handlers(key)}
              className={cn(
                "relative flex flex-1 items-end justify-center rounded-b-[5px] pb-1.5",
                "transition-[background-color,box-shadow] duration-75",
                isHeld
                  ? cn(colorClass, "shadow-[inset_0_2px_6px_rgb(0_0_0/0.25)]")
                  : "bg-key-white shadow-[inset_0_-4px_0_rgb(0_0_0/0.07)] hover:bg-[color-mix(in_oklch,var(--key-white),black_4%)]"
              )}
            >
              {key % 12 === 0 && (
                <span className={cn("text-[0.6rem] font-medium", isHeld ? "text-white/90" : "text-black/35")}>
                  {noteName(key)}
                </span>
              )}
            </div>
          )
        })}
        {keys.filter(isBlackKey).map((key) => {
          const isHeld = held.has(key)
          const left = ((whiteIndex.get(key - 1) ?? 0) + 1) * whiteWidth
          return (
            <div
              key={key}
              role="button"
              aria-label={noteName(key)}
              aria-pressed={isHeld}
              {...handlers(key)}
              className={cn(
                "absolute top-0 z-10 -translate-x-1/2 rounded-b-[4px] shadow-[0_3px_6px_rgb(0_0_0/0.35)]",
                "transition-[background-color] duration-75",
                pedal ? "h-[55%]" : "h-[62%]",
                isHeld
                  ? colorClass
                  : "bg-key-black bg-[linear-gradient(to_bottom,transparent_80%,rgb(255_255_255/0.12))] ring-1 ring-black/40"
              )}
              style={{ left: `${left}%`, width: `${whiteWidth * (pedal ? 0.5 : 0.62)}%` }}
            />
          )
        })}
      </div>
    </div>
  )
}
