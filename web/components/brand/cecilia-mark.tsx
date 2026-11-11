import { cn } from "@/lib/utils"

// Façade pipes, tallest in the middle, as in a classic organ case "flat".
// [x, top] in a 32-unit box; every pipe ends at the same foot line.
const PIPES: [number, number][] = [
  [4.5, 13],
  [10.25, 8],
  [16, 3],
  [21.75, 8],
  [27.5, 13],
]
const HALF = 2.5 // pipe half-width
const MOUTH_Y = 21
const FOOT_Y = 27

/** Cecilia's mark: a flat of five organ pipes. Colours follow `currentColor`. */
export function CeciliaMark({ className, title }: { className?: string; title?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      className={cn("size-6", className)}
      role={title ? "img" : undefined}
      aria-hidden={title ? undefined : true}
      aria-label={title}
    >
      {PIPES.map(([x, top]) => (
        <g key={x}>
          {/* body */}
          <rect x={x - HALF} y={top} width={HALF * 2} height={MOUTH_Y - top} rx={0.8} fill="currentColor" />
          {/* mouth: the upper lip cut-out */}
          <path d={`M${x - 1.4} ${MOUTH_Y} h2.8 l-0.6 -2 h-1.6 z`} fill="var(--mark-cutout, var(--sidebar-accent))" opacity={0.7} />
          {/* conical foot */}
          <path d={`M${x - HALF} ${MOUTH_Y} h${HALF * 2} l-1.7 ${FOOT_Y - MOUTH_Y} h-1.6 z`} fill="currentColor" opacity={0.85} />
        </g>
      ))}
      {/* the pipe shade / impost the pipes stand on */}
      <rect x={2} y={FOOT_Y} width={28} height={2} rx={0.5} fill="currentColor" opacity={0.6} />
    </svg>
  )
}
