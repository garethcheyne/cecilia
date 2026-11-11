"use client"

import * as React from "react"

import { Slider } from "@/components/ui/slider"
import type { DivisionRole, OrganDefinition, OrganEvent, SimSnapshot } from "@/lib/api/client"
import { cn } from "@/lib/utils"

import { CONSOLE_ORDER, DIVISION_BG, DIVISION_RING } from "./divisions"
import { Drawstop } from "./drawstop"
import { Keyboard } from "./keyboard"
import { coupledKeys } from "@/lib/sounding"

type ConsoleState = SimSnapshot["console"]

const EMPTY = new Set<number>()

/**
 * A playable organ console: for each division, its stop jamb, expression
 * pedal and keyboard. Rendering only; every action is reported via `onAct`.
 */
export function OrganConsole({
  organ,
  state,
  onAct,
  activeDivision,
}: {
  organ: OrganDefinition
  state: ConsoleState
  onAct: (event: OrganEvent) => void
  /** Division the computer keyboard plays, highlighted. */
  activeDivision?: DivisionRole
}) {
  const divisions = React.useMemo(
    () =>
      [...organ.divisions].sort(
        (a, b) => CONSOLE_ORDER.indexOf(a.role) - CONSOLE_ORDER.indexOf(b.role)
      ),
    [organ]
  )
  const drawn = React.useMemo(() => new Set(state.stops), [state.stops])
  const coupled = React.useMemo(() => coupledKeys(organ, state.stops, state.notes), [organ, state.stops, state.notes])

  return (
    <div className="console-panel flex w-full min-w-0 flex-col gap-8 rounded-2xl p-3 text-panel-foreground sm:p-6">
      {divisions.map((division) => {
        const stops = organ.stops.filter((s) => s.division === division.role)
        const held = new Set(state.notes[division.role] ?? EMPTY)
        const expression = state.expression[division.role]
        const isPedal = division.role === "pedal"
        return (
          <section key={division.role} aria-label={division.name} className="flex min-w-0 flex-col gap-3">
            <header className="flex flex-wrap items-center gap-x-4 gap-y-2">
              <h2 className="flex items-center gap-2.5 font-display text-lg font-semibold">
                <span className={cn("h-4 w-1 rounded-full", DIVISION_BG[division.role])} aria-hidden />
                {division.name}
              </h2>
              {activeDivision === division.role && (
                <span className="rounded-full bg-white/8 px-2.5 py-0.5 text-xs text-panel-muted ring-1 ring-white/10">
                  Computer keys play here
                </span>
              )}
              {division.expression && expression !== undefined && (
                <label className="ml-auto flex w-full max-w-56 items-center gap-3 text-xs text-panel-muted sm:w-56">
                  <span className="shrink-0">Expression</span>
                  <Slider
                    value={expression}
                    min={0}
                    max={127}
                    onValueChange={(v) =>
                      onAct({ type: "expression", division: division.role, value: v as number })
                    }
                    aria-label={`${division.name} expression`}
                  />
                  <span className="w-8 text-right tabular-nums">{Math.round((expression / 127) * 100)}%</span>
                </label>
              )}
            </header>
            {stops.length > 0 && (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(4.75rem,1fr))] justify-items-center gap-2.5 sm:flex sm:flex-wrap sm:gap-4">
                {stops.map((stop) => (
                  <Drawstop
                    key={stop.id}
                    stop={stop}
                    on={drawn.has(stop.id)}
                    ringClass={DIVISION_RING[division.role]}
                    onToggle={(on) => onAct({ type: "stop", stop: stop.id, on })}
                  />
                ))}
              </div>
            )}
            <div className={cn("min-w-0", isPedal && "mx-auto w-full max-w-3xl")}>
              <Keyboard
                low={division.low_key}
                high={division.high_key}
                held={held}
                coupled={coupled.get(division.role)}
                pedal={isPedal}
                label={`${division.name} keys`}
                colorClass={DIVISION_BG[division.role]}
                onNote={(key, on) =>
                  onAct({ type: "note", division: division.role, key, velocity: on ? 96 : 0 })
                }
              />
            </div>
          </section>
        )
      })}
    </div>
  )
}
