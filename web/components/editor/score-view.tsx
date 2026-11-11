"use client"

import * as React from "react"
import { InfoIcon } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { Track } from "@/lib/api/client"
import { durationParts, gridFor, keyName, layout, spell, type Grid, type Measure, type StaffId } from "@/lib/notation"

type VF = typeof import("vexflow/bravura")

const TIMES = ["2/4", "3/4", "4/4", "3/2", "6/8", "9/8", "12/8"]
const STAFF_GAP = 90
const SYSTEM_GAP = 60
const CLEF: Record<StaffId, string> = { treble: "treble", bass: "bass", pedal: "bass" }
const REST_KEY: Record<StaffId, string> = { treble: "b/4", bass: "d/3", pedal: "d/3" }

interface Box {
  /** Where the notes of the bar start and how wide they run. */
  cx: number
  cw: number
  x: number
  y: number
  w: number
  h: number
  startMs: number
  endMs: number
}

/** Draw `measures` into `el`, returning where each bar landed. */
function render(vf: VF, el: HTMLDivElement, measures: Measure[], staves: StaffId[], g: Grid, width: number, dark: boolean): Box[] {
  const { Renderer, Stave, StaveNote, Voice, Formatter, Accidental, Dot, StaveTie, StaveConnector } = vf
  el.innerHTML = ""
  const perLine = Math.max(1, Math.min(6, Math.floor((width - 40) / 210)))
  const lines = Math.ceil(measures.length / perLine)
  const systemH = staves.length * STAFF_GAP + SYSTEM_GAP
  const height = lines * systemH + 40
  const renderer = new Renderer(el, Renderer.Backends.SVG)
  renderer.resize(width, height)
  const ctx = renderer.getContext()
  const ink = dark ? "#e5e7eb" : "#1f2330"
  ctx.setFillStyle(ink)
  ctx.setStrokeStyle(ink)
  const key = keyName(g.fifths)
  const boxes: Box[] = []
  // Last note per staff, to tie across bar lines.
  const lastNote: Partial<Record<StaffId, { note: InstanceType<VF["StaveNote"]>; keys: number[] }>> = {}

  for (let line = 0; line < lines; line++) {
    const row = measures.slice(line * perLine, (line + 1) * perLine)
    const top = 20 + line * systemH
    const lead = 70 + (Math.abs(g.fifths) * 10) + (line === 0 ? 30 : 0)
    const barW = (width - 30 - lead) / row.length
    let x = 10
    const firstStaves: InstanceType<VF["Stave"]>[] = []
    row.forEach((m, i) => {
      const w = barW + (i === 0 ? lead : 0)
      const created: InstanceType<VF["Stave"]>[] = []
      staves.forEach((staff, si) => {
        const stave = new Stave(x, top + si * STAFF_GAP, w)
        if (i === 0) {
          stave.addClef(CLEF[staff]).addKeySignature(key)
          if (line === 0) stave.addTimeSignature(`${g.beats}/${g.beatUnit}`)
        }
        stave.setContext(ctx).draw()
        created.push(stave)

        const notes: InstanceType<VF["StaveNote"]>[] = []
        const ties: { a: InstanceType<VF["StaveNote"]>; b: InstanceType<VF["StaveNote"]>; ia: number[]; ib: number[] }[] = []
        let prev = lastNote[staff]
        for (const slice of m.staves[staff] ?? []) {
          const rest = slice.keys.length === 0
          const keys = rest ? [REST_KEY[staff]] : slice.keys.map((k) => spell(k, g.fifths))
          const parts = durationParts(slice.len)
          parts.forEach((part, pi) => {
            const n = new StaveNote({ keys, duration: part.duration + (rest ? "r" : ""), clef: CLEF[staff], dots: part.dots, autoStem: true })
            if (part.dots) Dot.buildAndAttach([n], { all: true })
            notes.push(n)
            if (!rest) {
              const all = slice.keys.map((_, k) => k)
              if (pi > 0) {
                ties.push({ a: notes[notes.length - 2], b: n, ia: all, ib: all })
              } else if (prev && slice.tiedIn.length) {
                const ia = slice.tiedIn.map((k) => prev!.keys.indexOf(k)).filter((v) => v >= 0)
                const ib = slice.tiedIn.map((k) => slice.keys.indexOf(k)).filter((v) => v >= 0)
                if (ia.length && ia.length === ib.length) ties.push({ a: prev.note, b: n, ia, ib })
              }
            }
          })
          if (!rest) prev = { note: notes[notes.length - 1], keys: slice.keys }
          else prev = undefined
        }
        lastNote[staff] = prev
        if (notes.length === 0) return
        const voice = new Voice({ numBeats: g.beats, beatValue: g.beatUnit }).setStrict(false)
        voice.addTickables(notes)
        Accidental.applyAccidentals([voice], key)
        new Formatter().joinVoices([voice]).format([voice], Math.max(40, w - (i === 0 ? lead : 0) - 20))
        voice.draw(ctx, stave)
        for (const t of ties) {
          new StaveTie({ firstNote: t.a, lastNote: t.b, firstIndexes: t.ia, lastIndexes: t.ib }).setContext(ctx).draw()
        }
      })
      if (i === 0) firstStaves.push(...created)
      // Bar line through the whole system.
      if (created.length > 1) {
        new StaveConnector(created[0], created[created.length - 1]).setType("singleRight").setContext(ctx).draw()
      }
      const cx = x + (i === 0 ? lead : 10)
      boxes.push({ cx, cw: x + w - 10 - cx, x, y: top - 10, w, h: staves.length * STAFF_GAP - 10, startMs: m.startMs, endMs: m.endMs })
      x += w
    })
    if (firstStaves.length > 1) {
      new StaveConnector(firstStaves[0], firstStaves[firstStaves.length - 1]).setType("singleLeft").setContext(ctx).draw()
      new StaveConnector(firstStaves[0], firstStaves[1]).setType("brace").setContext(ctx).draw()
    }
  }
  return boxes
}

/** Staff notation for a track, following the playback position. */
export function ScoreView({ track, position }: { track: Track; position: number | null }) {
  const host = React.useRef<HTMLDivElement>(null)
  const scoreEl = React.useRef<HTMLDivElement>(null)
  const [vf, setVf] = React.useState<VF | null>(null)
  const [width, setWidth] = React.useState(900)
  const [boxes, setBoxes] = React.useState<Box[]>([])
  const [override, setOverride] = React.useState<Partial<Grid>>({})
  const verses = (track.verses ?? []).length + 1
  const [chosenVerse, setChosenVerse] = React.useState(1)
  // While it plays, the score follows the verse being played, unless
  // someone picks a verse to look at.
  const [follow, setFollow] = React.useState(true)
  const bounds = React.useMemo(() => [0, ...(track.verses ?? []), track.duration_ms], [track])
  const playingVerse = position == null ? null : Math.max(1, bounds.findIndex((b) => b > position))
  const verse = follow && playingVerse ? Math.min(playingVerse, verses) : chosenVerse
  const [dark, setDark] = React.useState(false)

  React.useEffect(() => {
    let alive = true
    import("vexflow/bravura").then((m) => alive && setVf(m))
    return () => {
      alive = false
    }
  }, [])

  React.useEffect(() => {
    const el = host.current
    if (!el) return
    const ro = new ResizeObserver(([e]) => setWidth(Math.max(340, Math.floor(e.contentRect.width))))
    ro.observe(el)
    setDark(document.documentElement.classList.contains("dark"))
    return () => ro.disconnect()
  }, [])

  const grid = React.useMemo(() => gridFor(track, override), [track, override])
  const span = React.useMemo(
    () => ({ start: bounds[verse - 1] ?? 0, end: bounds[verse] ?? track.duration_ms }),
    [bounds, verse, track.duration_ms]
  )
  const { measures, staves } = React.useMemo(() => layout(track, grid, span), [track, grid, span])

  React.useEffect(() => {
    if (!vf || !scoreEl.current) return
    try {
      setBoxes(render(vf, scoreEl.current, measures, staves, grid, width, dark))
    } catch (e) {
      console.error(e)
      scoreEl.current.textContent = "This passage couldn't be drawn."
    }
  }, [vf, measures, staves, grid, width, dark])

  const current = position == null ? -1 : boxes.findIndex((b) => position >= b.startMs && position < b.endMs)
  const box = current >= 0 ? boxes[current] : null
  // Keep the bar being played in view (in the page or in a drawer).
  const highlight = React.useRef<HTMLDivElement>(null)
  React.useEffect(() => {
    if (current >= 0) highlight.current?.scrollIntoView({ block: "center", behavior: "smooth" })
  }, [current])
  const progress = box && position != null ? Math.min(1, Math.max(0, (position - box.startMs) / (box.endMs - box.startMs))) : 0

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-end gap-3">
        {verses > 1 && (
          <Tabs
            value={String(verse)}
            onValueChange={(v) => {
              setChosenVerse(Number(v))
              setFollow(false)
            }}
          >
            <TabsList>
              {Array.from({ length: verses }, (_, i) => (
                <TabsTrigger key={i} value={String(i + 1)}>
                  Verse {i + 1}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        )}
        {position != null && !follow && (
          <Button variant="outline" size="sm" onClick={() => setFollow(true)}>
            Follow the music
          </Button>
        )}
        {grid.approximate && (
          <>
            <Field className="w-28">
              <FieldLabel className="text-xs">Beats/min</FieldLabel>
              <Input
                type="number"
                min={30}
                max={240}
                value={Math.round(grid.bpm)}
                onChange={(e) => setOverride({ ...override, bpm: Math.max(30, Math.min(240, Number(e.target.value) || 80)) })}
              />
            </Field>
            <Field className="w-28">
              <FieldLabel className="text-xs">Time</FieldLabel>
              <Select
                value={`${grid.beats}/${grid.beatUnit}`}
                items={TIMES.map((t) => ({ value: t, label: t }))}
                onValueChange={(v) => {
                  if (!v) return
                  const [beats, beatUnit] = String(v).split("/").map(Number)
                  setOverride({ ...override, beats, beatUnit })
                }}
              >
                <SelectTrigger className="w-full" aria-label="Time signature">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {TIMES.map((t) => (
                      <SelectItem key={t} value={t}>
                        {t}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </>
        )}
      </div>
      {grid.approximate && (
        <Alert>
          <InfoIcon />
          <AlertDescription>
            This is a live recording, so the notation is an estimate. Set the speed and time signature until the bars
            line up.
          </AlertDescription>
        </Alert>
      )}
      <div ref={host} className="relative w-full overflow-x-auto rounded-xl border bg-card">
        {box && (
          <>
            <div
              ref={highlight}
              className="pointer-events-none absolute rounded-md bg-primary/10 ring-1 ring-primary/40 transition-all duration-150"
              style={{ left: box.x, top: box.y, width: box.w, height: box.h }}
              aria-hidden
            />
            {/* Where the music is now, moving through the bar. */}
            <div
              className="pointer-events-none absolute z-10 w-0.5 rounded-full bg-primary"
              style={{ left: box.cx + progress * box.cw, top: box.y, height: box.h }}
              aria-hidden
            />
          </>
        )}
        <div ref={scoreEl} className="relative" />
        {!vf && (
          <div className="flex items-center justify-center gap-2 p-10 text-sm text-muted-foreground">
            <Spinner /> Loading notation…
          </div>
        )}
        {vf && measures.length === 0 && <p className="p-10 text-center text-sm text-muted-foreground">No notes to show.</p>}
      </div>
    </div>
  )
}
