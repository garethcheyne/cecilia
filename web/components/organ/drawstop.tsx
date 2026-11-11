"use client"

import type { Stop } from "@/lib/api/client"
import { formatFootage } from "@/lib/organ"
import { cn } from "@/lib/utils"

import { stopKind } from "./divisions"

/** Label colour by stop kind: lit (on) and unlit (off). */
const LABEL: Record<
  ReturnType<typeof stopKind>,
  { on: string; off: string }
> = {
  voice: { on: "text-lamp-foreground", off: "text-panel-foreground/80" },
  reed: {
    on: "text-stop-reed",
    off: "text-[color-mix(in_oklch,var(--stop-reed),white_45%)]",
  },
  coupler: {
    on: "text-stop-coupler",
    off: "text-[color-mix(in_oklch,var(--stop-coupler),white_55%)]",
  },
  accessory: {
    on: "text-lamp-foreground italic",
    off: "text-panel-foreground/80 italic",
  },
}

/**
 * A drawknob (round) or tab (rectangle) as an illuminated control: unlit
 * graphite when off, a lit face ringed in the division's colour when drawn.
 */
export function Drawstop({
  stop,
  on,
  onToggle,
  ringClass,
}: {
  stop: Stop
  on: boolean
  onToggle: (on: boolean) => void
  /** e.g. "ring-division-great"; the drawn stop's ring colour. */
  ringClass?: string
}) {
  const kind = stopKind(stop)
  const tab = stop.style?.shape === "rectangle"
  const lines = [...(stop.names.length > 0 ? stop.names : [stop.id])]
  // Engrave the footage line with a foot mark: "8" -> "8′", "2 2/3" -> "2⅔′".
  const eq = stop.equivalent
  const last = lines.length - 1
  const hasFootage =
    eq.type === "voice" && lines.length > 1 && lines[last] === eq.footage
  if (hasFootage) lines[last] = formatFootage(lines[last])

  return (
    <button
      type="button"
      aria-pressed={on}
      title={stop.full_name ?? lines.join(" ")}
      onClick={() => onToggle(!on)}
      className={cn(
        "group/stop relative flex shrink-0 flex-col items-center justify-center px-1.5 text-center font-display leading-[1.08] font-semibold tracking-[-0.01em] [font-stretch:85%] outline-none select-none",
        "focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-primary",
        "transition-[background-color,box-shadow,color,scale] duration-150 ease-out active:scale-95",
        // Tabs are wider than a knob: span two cells of the phone grid.
        tab
          ? "col-span-2 h-12 w-24 rounded-lg text-[0.74rem]"
          : "size-19 rounded-full text-[0.78rem]",
        on
          ? cn(
              "bg-lamp ring-[3px] ring-offset-2 ring-offset-panel",
              ringClass ?? "ring-primary",
              "shadow-[0_0_24px_-2px_color-mix(in_oklch,var(--lamp),transparent_55%),inset_0_-3px_0_rgb(0_0_0/0.08)]",
              LABEL[kind].on
            )
          : cn(
              "bg-panel-raised shadow-[inset_0_1px_0_rgb(255_255_255/0.08),0_1px_2px_rgb(0_0_0/0.4)] hover:bg-[color-mix(in_oklch,var(--panel-raised),white_6%)]",
              LABEL[kind].off
            )
      )}
    >
      {lines.map((line, i) => (
        <span
          key={i}
          className={cn(
            hasFootage &&
              i === last &&
              "mt-0.5 font-sans text-[0.7rem] font-medium tabular-nums opacity-70"
          )}
        >
          {line}
        </span>
      ))}
      <span className="sr-only">{on ? " (on)" : " (off)"}</span>
    </button>
  )
}
