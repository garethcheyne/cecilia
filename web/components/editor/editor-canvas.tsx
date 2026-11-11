"use client"

import * as React from "react"

import type { DivisionRole, OrganDefinition } from "@/lib/api/client"
import {
  addNote,
  moveControl,
  moveNotes,
  notesIn,
  resizeNotes,
  snap,
  type EditTrack,
} from "@/lib/edit"
import { noteName } from "@/lib/midi"
import { stopLabel } from "@/lib/stops"
import { formatClock } from "@/lib/time"

export interface Selection {
  notes: Set<number>
  controls: Set<number>
  /** Index into track.verses. */
  verse: number | null
}

export const EMPTY_SELECTION: Selection = { notes: new Set(), controls: new Set(), verse: null }

export type ChangePhase = "start" | "move" | "end"

/** What's under the pointer when the context menu opens. */
export type ContextTarget =
  | { kind: "note"; id: number; ms: number; key: number }
  | { kind: "stop"; id: number; ms: number }
  | { kind: "verse"; index: number; ms: number }
  | { kind: "roll"; ms: number; key: number }
  | { kind: "lane"; ms: number }

interface Props {
  edit: EditTrack
  selection: Selection
  onSelect: (s: Selection) => void
  cursor: number
  onCursor: (ms: number) => void
  range: [number, number] | null
  onRange: (r: [number, number] | null) => void
  tool: "select" | "draw"
  drawDivision: DivisionRole
  pxPerSec: number
  onZoom: (pxPerSec: number) => void
  grid: number
  playhead: number | null
  /** Drags report start/move/end so one drag is one undo step. */
  onChange: (next: EditTrack, phase: ChangePhase) => void
  organ: OrganDefinition | null
  /** Called with what was right-clicked (or long-pressed). */
  onContextTarget?: (t: ContextTarget) => void
}

const GUTTER = 44
const RULER = 22
const STOPS = 58
const EXPR = 34
const VERSE = 22
const BLACK = new Set([1, 3, 6, 8, 10])
const DIVISIONS: DivisionRole[] = ["pedal", "great", "swell", "choir", "solo", "echo"]

type Drag =
  | { kind: "move"; base: EditTrack; ids: Set<number>; ms0: number; key0: number }
  | { kind: "resize"; base: EditTrack; ids: Set<number>; ms0: number }
  | { kind: "band"; x0: number; y0: number; x1: number; y1: number; additive: boolean }
  | { kind: "pan"; x0: number; y0: number; scroll0: number; top0: number }
  | { kind: "draw"; base: EditTrack; id: number; start: number }
  | { kind: "range"; ms0: number }
  | { kind: "control"; base: EditTrack; id: number; ms0: number; t0: number }
  | { kind: "verse"; index: number; v0: number; ms0: number }

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#888"
}

export function EditorCanvas(p: Props) {
  const wrap = React.useRef<HTMLDivElement>(null)
  const canvas = React.useRef<HTMLCanvasElement>(null)
  const [size, setSize] = React.useState({ w: 800, h: 480 })
  const [scrollMs, setScrollMs] = React.useState(0)
  const [topKey, setTopKey] = React.useState(84)
  const [hover, setHover] = React.useState<{ x: number; y: number; text: string } | null>(null)
  const drag = React.useRef<Drag | null>(null)
  const [band, setBand] = React.useState<Drag | null>(null)
  const touch = React.useRef(false)

  const keyH = size.w < 640 ? 13 : 11
  const rollTop = RULER + STOPS + EXPR
  const rollBottom = size.h - VERSE
  const pxPerMs = p.pxPerSec / 1000
  const visibleKeys = Math.floor((rollBottom - rollTop) / keyH)

  // Fit the pitch range to the music the first time.
  const fitted = React.useRef(false)
  React.useEffect(() => {
    if (fitted.current || p.edit.notes.length === 0) return
    fitted.current = true
    const hi = Math.max(...p.edit.notes.map((n) => n.key))
    const lo = Math.min(...p.edit.notes.map((n) => n.key))
    const mid = Math.round((hi + lo) / 2)
    setTopKey(Math.min(127, Math.max(hi + 2, mid + Math.floor(visibleKeys / 2))))
  }, [p.edit.notes, visibleKeys])

  React.useEffect(() => {
    const el = wrap.current
    if (!el) return
    const ro = new ResizeObserver(([e]) => setSize({ w: Math.max(320, e.contentRect.width), h: Math.max(360, e.contentRect.height) }))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // Keep the playhead in view while playing (adjusted during render, as
  // React recommends for state derived from props).
  if (p.playhead != null) {
    const viewMs = (size.w - GUTTER) / pxPerMs
    if (p.playhead < scrollMs || p.playhead > scrollMs + viewMs * 0.85) {
      setScrollMs(Math.max(0, p.playhead - viewMs * 0.15))
    }
  }

  const x = React.useCallback((ms: number) => GUTTER + (ms - scrollMs) * pxPerMs, [scrollMs, pxPerMs])
  const msAt = React.useCallback((px: number) => scrollMs + (px - GUTTER) / pxPerMs, [scrollMs, pxPerMs])
  const y = React.useCallback((key: number) => rollTop + (topKey - key) * keyH, [rollTop, topKey, keyH])
  const keyAt = React.useCallback((py: number) => Math.round(topKey - (py - rollTop) / keyH + 0.5), [topKey, rollTop, keyH])

  const track = p.edit.track
  const meter = track.meter

  // Stop markers, stacked when simultaneous: computed once per change.
  const markers = React.useMemo(() => {
    const stops = p.edit.controls.filter((c) => c.type === "stop").sort((a, b) => a.t - b.t)
    const out: { id: number; t: number; row: number; on: boolean; label: string }[] = []
    let lastT = -1e9
    let row = 0
    for (const c of stops) {
      if (c.type !== "stop") continue
      row = Math.abs(c.t - lastT) < 4 / pxPerMs ? row + 1 : 0
      lastT = c.t
      out.push({ id: c.id, t: c.t, row: Math.min(row, 5), on: c.on, label: stopLabel(c.stop, p.organ) })
    }
    return out
  }, [p.edit.controls, p.organ, pxPerMs])

  // ---------------------------------------------------------------------
  // Drawing
  // ---------------------------------------------------------------------
  React.useEffect(() => {
    const c = canvas.current
    if (!c) return
    const dpr = window.devicePixelRatio || 1
    c.width = size.w * dpr
    c.height = size.h * dpr
    const g = c.getContext("2d")!
    g.setTransform(dpr, 0, 0, dpr, 0, 0)
    const col = {
      bg: cssVar("--background"),
      card: cssVar("--card"),
      muted: cssVar("--muted"),
      text: cssVar("--muted-foreground"),
      fg: cssVar("--foreground"),
      border: cssVar("--border"),
      primary: cssVar("--primary"),
      danger: cssVar("--destructive"),
      verse: cssVar("--division-echo"),
    }
    const div = Object.fromEntries(DIVISIONS.map((d) => [d, cssVar(`--division-${d}`)])) as Record<DivisionRole, string>
    const W = size.w
    const H = size.h
    g.clearRect(0, 0, W, H)
    g.fillStyle = col.card
    g.fillRect(0, 0, W, H)

    // Key rows
    g.save()
    g.beginPath()
    g.rect(GUTTER, rollTop, W - GUTTER, rollBottom - rollTop)
    g.clip()
    for (let k = topKey; k > topKey - visibleKeys - 1; k--) {
      if (BLACK.has(((k % 12) + 12) % 12)) {
        g.fillStyle = col.muted
        g.fillRect(GUTTER, y(k), W - GUTTER, keyH)
      }
      if (k % 12 === 0) {
        g.fillStyle = col.border
        g.fillRect(GUTTER, y(k) + keyH - 1, W - GUTTER, 1)
      }
    }
    g.restore()

    // Time grid: bars and beats if the track has a meter, else seconds.
    const t0 = scrollMs
    const t1 = msAt(W)
    const gridLines: { t: number; strong: boolean; label?: string }[] = []
    if (meter) {
      const beat = (60000 / meter.bpm) * (4 / meter.beat_unit)
      const barLen = beat * meter.beats
      const offset = meter.offset_ms ?? 0
      const firstBar = Math.floor((t0 - offset) / barLen)
      for (let b = firstBar; offset + b * barLen < t1; b++) {
        const bt = offset + b * barLen
        if (bt >= 0) gridLines.push({ t: bt, strong: true, label: String(b + 1) })
        if (beat * pxPerMs > 12) {
          for (let i = 1; i < meter.beats; i++) gridLines.push({ t: bt + i * beat, strong: false })
        }
      }
    } else {
      const step = [0.5, 1, 2, 5, 10, 30, 60].find((s) => s * p.pxPerSec >= 60) ?? 60
      for (let s = Math.floor(t0 / 1000 / step) * step; s * 1000 < t1; s += step) {
        gridLines.push({ t: s * 1000, strong: true, label: formatClock(s * 1000) })
      }
    }
    for (const l of gridLines) {
      const gx = Math.round(x(l.t)) + 0.5
      if (gx < GUTTER) continue
      g.fillStyle = col.border
      g.globalAlpha = l.strong ? 1 : 0.5
      g.fillRect(gx, RULER, 1, H - RULER)
      g.globalAlpha = 1
    }

    // Section (range) selection
    if (p.range) {
      g.fillStyle = col.primary
      g.globalAlpha = 0.1
      g.fillRect(x(p.range[0]), RULER, (p.range[1] - p.range[0]) * pxPerMs, H - RULER)
      g.globalAlpha = 1
    }

    // End of track
    g.fillStyle = col.muted
    g.globalAlpha = 0.6
    const endX = x(track.duration_ms)
    if (endX < W) g.fillRect(endX, RULER, W - endX, H - RULER)
    g.globalAlpha = 1

    // Notes
    g.save()
    g.beginPath()
    g.rect(GUTTER, rollTop, W - GUTTER, rollBottom - rollTop)
    g.clip()
    for (const n of p.edit.notes) {
      if (n.end < t0 || n.start > t1 || n.key > topKey + 1 || n.key < topKey - visibleKeys - 1) continue
      const nx = x(n.start)
      const nw = Math.max(3, (n.end - n.start) * pxPerMs)
      const ny = y(n.key) + 1
      g.fillStyle = div[n.division]
      g.globalAlpha = 0.9
      g.beginPath()
      g.roundRect(nx, ny, nw, keyH - 2, 3)
      g.fill()
      g.globalAlpha = 1
      if (p.selection.notes.has(n.id)) {
        g.strokeStyle = col.fg
        g.lineWidth = 2
        g.stroke()
      }
    }
    g.restore()

    // Lanes background and labels
    g.fillStyle = col.bg
    g.fillRect(0, RULER, W, STOPS + EXPR)
    g.fillRect(0, rollBottom, W, VERSE)
    g.fillStyle = col.border
    g.fillRect(0, RULER + STOPS, W, 1)
    g.fillRect(0, rollTop - 1, W, 1)
    g.fillRect(0, rollBottom, W, 1)

    // Stop markers
    for (const m of markers) {
      const mx = x(m.t)
      if (mx < GUTTER - 6 || mx > W + 6) continue
      const my = RULER + 10 + m.row * 9
      g.strokeStyle = m.on ? col.primary : col.text
      g.fillStyle = col.primary
      g.lineWidth = p.selection.controls.has(m.id) ? 2.5 : 1.5
      if (m.on) {
        g.beginPath()
        g.arc(mx, my, 3.5, 0, Math.PI * 2)
        g.fill()
        if (p.selection.controls.has(m.id)) g.stroke()
      } else {
        g.beginPath()
        g.moveTo(mx - 3.5, my - 3.5)
        g.lineTo(mx + 3.5, my + 3.5)
        g.moveTo(mx + 3.5, my - 3.5)
        g.lineTo(mx - 3.5, my + 3.5)
        g.stroke()
      }
    }

    // Expression lines
    const exprTop = RULER + STOPS + 4
    const exprH = EXPR - 8
    const byDiv = new Map<DivisionRole, { t: number; v: number }[]>()
    for (const [d, v] of Object.entries(track.initial.expression ?? {})) byDiv.set(d as DivisionRole, [{ t: 0, v: v as number }])
    for (const c of [...p.edit.controls].sort((a, b) => a.t - b.t)) {
      if (c.type !== "expression") continue
      const list = byDiv.get(c.division) ?? [{ t: 0, v: 127 }]
      list.push({ t: c.t, v: c.value })
      byDiv.set(c.division, list)
    }
    for (const [d, pts] of byDiv) {
      g.strokeStyle = div[d]
      g.lineWidth = 1.5
      g.beginPath()
      let prev = pts[0]
      g.moveTo(GUTTER, exprTop + exprH * (1 - prev.v / 127))
      for (const pt of pts.slice(1)) {
        g.lineTo(Math.max(GUTTER, x(pt.t)), exprTop + exprH * (1 - prev.v / 127))
        g.lineTo(Math.max(GUTTER, x(pt.t)), exprTop + exprH * (1 - pt.v / 127))
        prev = pt
      }
      g.lineTo(W, exprTop + exprH * (1 - prev.v / 127))
      g.stroke()
    }

    // Verse markers
    ;(track.verses ?? []).forEach((v, i) => {
      const vx = x(v)
      if (vx < GUTTER || vx > W) return
      g.fillStyle = col.verse
      g.globalAlpha = 0.25
      g.fillRect(vx - 0.5, rollTop, 1, rollBottom - rollTop)
      g.globalAlpha = 1
      g.beginPath()
      g.moveTo(vx, rollBottom + 3)
      g.lineTo(vx + 7, rollBottom + 3)
      g.lineTo(vx, rollBottom + 15)
      g.closePath()
      g.fill()
      if (p.selection.verse === i) {
        g.strokeStyle = col.fg
        g.lineWidth = 1.5
        g.stroke()
      }
      g.fillStyle = col.text
      g.font = "10px var(--font-sans), sans-serif"
      g.fillText(`v${i + 2}`, vx + 9, rollBottom + 14)
    })

    // Ruler
    g.fillStyle = col.bg
    g.fillRect(0, 0, W, RULER)
    g.fillStyle = col.border
    g.fillRect(0, RULER - 1, W, 1)
    g.fillStyle = col.text
    g.font = "11px var(--font-sans), sans-serif"
    for (const l of gridLines) {
      if (!l.label) continue
      const lx = x(l.t)
      if (lx >= GUTTER - 2 && lx < W - 20) g.fillText(l.label, lx + 3, 15)
    }

    // Gutter: lane names and key names
    g.fillStyle = col.bg
    g.fillRect(0, RULER, GUTTER, H - RULER)
    g.fillStyle = col.border
    g.fillRect(GUTTER - 1, 0, 1, H)
    g.fillStyle = col.text
    g.font = "10px var(--font-sans), sans-serif"
    g.fillText("Stops", 6, RULER + 16)
    g.fillText("Swell", 6, RULER + STOPS + 14)
    g.fillText("Verse", 6, rollBottom + 15)
    g.save()
    g.beginPath()
    g.rect(0, rollTop, GUTTER, rollBottom - rollTop)
    g.clip()
    for (let k = topKey; k > topKey - visibleKeys - 1; k--) {
      if (k % 12 === 0) g.fillText(noteName(k), 6, y(k) + keyH - 2)
    }
    g.restore()

    // Cursor and playhead
    const cx = x(p.cursor)
    if (cx >= GUTTER) {
      g.fillStyle = col.primary
      g.fillRect(Math.round(cx) - 1, 0, 2, H)
    }
    if (p.playhead != null) {
      const px = x(p.playhead)
      if (px >= GUTTER) {
        g.fillStyle = col.danger
        g.fillRect(Math.round(px), 0, 2, H)
      }
    }

    // Rubber band
    if (band?.kind === "band") {
      g.strokeStyle = col.primary
      g.lineWidth = 1
      g.setLineDash([4, 3])
      g.strokeRect(Math.min(band.x0, band.x1), Math.min(band.y0, band.y1), Math.abs(band.x1 - band.x0), Math.abs(band.y1 - band.y0))
      g.setLineDash([])
    }
  }, [p, size, scrollMs, topKey, keyH, visibleKeys, rollTop, rollBottom, pxPerMs, x, y, msAt, markers, band, track, meter])

  // ---------------------------------------------------------------------
  // Pointer handling
  // ---------------------------------------------------------------------
  const local = (e: React.PointerEvent | React.MouseEvent | React.WheelEvent) => {
    const r = canvas.current!.getBoundingClientRect()
    return { px: e.clientX - r.left, py: e.clientY - r.top }
  }

  const noteAt = (px: number, py: number) => {
    const ms = msAt(px)
    const key = keyAt(py)
    for (let i = p.edit.notes.length - 1; i >= 0; i--) {
      const n = p.edit.notes[i]
      if (n.key === key && ms >= n.start && ms <= n.end + 3 / pxPerMs) return n
    }
    return null
  }

  const markerAt = (px: number, py: number) =>
    markers.find((m) => Math.abs(x(m.t) - px) <= 6 && Math.abs(RULER + 10 + m.row * 9 - py) <= 6)

  const verseAt = (px: number) => (track.verses ?? []).findIndex((v) => Math.abs(x(v) - px) <= 7)

  /** What's at a point, for the context menu. */
  function targetAt(px: number, py: number): ContextTarget {
    const ms = Math.max(0, msAt(px))
    if (py >= RULER && py < RULER + STOPS) {
      const m = markerAt(px, py)
      if (m) return { kind: "stop", id: m.id, ms: m.t }
    }
    if (py >= rollBottom) {
      const i = verseAt(px)
      if (i >= 0) return { kind: "verse", index: i, ms: track.verses![i] }
    }
    if (py >= rollTop && py < rollBottom && px >= GUTTER) {
      const n = noteAt(px, py)
      if (n) return { kind: "note", id: n.id, ms, key: n.key }
      return { kind: "roll", ms: snap(ms, p.grid), key: keyAt(py) }
    }
    return { kind: "lane", ms: snap(ms, p.grid) }
  }

  function onContextMenu(e: React.MouseEvent) {
    const { px, py } = local(e)
    const t = targetAt(px, py)
    // Act on what was clicked: select it if it isn't already.
    if (t.kind === "note" && !p.selection.notes.has(t.id)) p.onSelect({ ...EMPTY_SELECTION, notes: new Set([t.id]) })
    if (t.kind === "stop") p.onSelect({ ...EMPTY_SELECTION, controls: new Set([t.id]) })
    if (t.kind === "verse") p.onSelect({ ...EMPTY_SELECTION, verse: t.index })
    if (t.kind === "roll" || t.kind === "lane") p.onCursor(t.ms)
    p.onContextTarget?.(t)
  }

  function onPointerDown(e: React.PointerEvent) {
    // A long press on a touch screen opens the menu for what was touched.
    if (e.pointerType === "touch") {
      const { px, py } = local(e)
      p.onContextTarget?.(targetAt(px, py))
    }
    if (e.button !== 0) return
    touch.current = e.pointerType === "touch"
    canvas.current?.setPointerCapture(e.pointerId)
    const { px, py } = local(e)
    const ms = Math.max(0, msAt(px))
    if (px < GUTTER) return
    if (py < RULER) {
      p.onCursor(snap(ms, p.grid))
      p.onRange(null)
      drag.current = { kind: "range", ms0: snap(ms, p.grid) }
      return
    }
    if (py < RULER + STOPS) {
      const m = markerAt(px, py)
      if (m) {
        p.onSelect({ ...EMPTY_SELECTION, controls: new Set([m.id]) })
        drag.current = { kind: "control", base: p.edit, id: m.id, ms0: ms, t0: m.t }
        p.onChange(p.edit, "start")
      } else {
        p.onSelect(EMPTY_SELECTION)
        p.onCursor(snap(ms, p.grid))
      }
      return
    }
    if (py < rollTop) {
      p.onCursor(snap(ms, p.grid))
      return
    }
    if (py >= rollBottom) {
      const i = verseAt(px)
      if (i >= 0) {
        p.onSelect({ ...EMPTY_SELECTION, verse: i })
        drag.current = { kind: "verse", index: i, v0: track.verses![i], ms0: ms }
        p.onChange(p.edit, "start")
      } else {
        p.onSelect(EMPTY_SELECTION)
      }
      return
    }
    // Piano roll
    const n = noteAt(px, py)
    if (n) {
      let ids = p.selection.notes
      if (e.shiftKey || e.ctrlKey || e.metaKey) {
        ids = new Set(ids)
        if (ids.has(n.id)) ids.delete(n.id)
        else ids.add(n.id)
      } else if (!ids.has(n.id)) {
        ids = new Set([n.id])
      }
      p.onSelect({ ...EMPTY_SELECTION, notes: ids })
      p.onCursor(snap(n.start, p.grid))
      const nearEnd = x(n.end) - px < 6 && (n.end - n.start) * pxPerMs > 10
      drag.current = nearEnd
        ? { kind: "resize", base: p.edit, ids, ms0: ms }
        : { kind: "move", base: p.edit, ids, ms0: ms, key0: keyAt(py) }
      p.onChange(p.edit, "start")
      return
    }
    if (p.tool === "draw") {
      const start = snap(ms, p.grid)
      const len = p.grid > 0 ? p.grid : 500
      const [next, id] = addNote(p.edit, { division: p.drawDivision, key: keyAt(py), start, end: start + len, velocity: 80 })
      p.onChange(p.edit, "start")
      p.onChange(next, "move")
      p.onSelect({ ...EMPTY_SELECTION, notes: new Set([id]) })
      drag.current = { kind: "draw", base: next, id, start }
      return
    }
    if (touch.current) {
      drag.current = { kind: "pan", x0: px, y0: py, scroll0: scrollMs, top0: topKey }
      return
    }
    const d: Drag = { kind: "band", x0: px, y0: py, x1: px, y1: py, additive: e.shiftKey }
    drag.current = d
    setBand(d)
    if (!e.shiftKey) p.onSelect(EMPTY_SELECTION)
    p.onCursor(snap(ms, p.grid))
  }

  function onPointerMove(e: React.PointerEvent) {
    const { px, py } = local(e)
    const d = drag.current
    if (!d) {
      // Hover help
      if (py >= RULER && py < RULER + STOPS) {
        const m = markerAt(px, py)
        setHover(m ? { x: px, y: py, text: `${m.label} ${m.on ? "on" : "off"} · ${formatClock(m.t)}` } : null)
      } else if (py >= rollTop && py < rollBottom && px > GUTTER) {
        const n = noteAt(px, py)
        setHover(n ? { x: px, y: py, text: `${noteName(n.key)} · ${n.division}` } : null)
      } else {
        setHover(null)
      }
      return
    }
    setHover(null)
    const ms = msAt(px)
    switch (d.kind) {
      case "move": {
        const dt = snap(ms - d.ms0, p.grid)
        const dk = keyAt(py) - d.key0
        p.onChange(moveNotes(d.base, d.ids, dt, dk), "move")
        break
      }
      case "resize":
        p.onChange(resizeNotes(d.base, d.ids, snap(ms - d.ms0, p.grid)), "move")
        break
      case "draw": {
        const end = Math.max(d.start + 20, snap(ms, p.grid) || ms)
        p.onChange({ ...d.base, notes: d.base.notes.map((n) => (n.id === d.id ? { ...n, end } : n)) }, "move")
        break
      }
      case "control":
        p.onChange(moveControl(d.base, d.id, snap(d.t0 + (ms - d.ms0), p.grid)), "move")
        break
      case "verse": {
        const verses = [...(track.verses ?? [])]
        verses[d.index] = Math.max(1, Math.round(d.v0 + (ms - d.ms0)))
        p.onChange({ ...p.edit, track: { ...track, verses } }, "move")
        break
      }
      case "range": {
        const a = d.ms0
        const b = Math.max(0, snap(ms, p.grid))
        p.onRange(Math.abs(b - a) * pxPerMs > 3 ? [Math.min(a, b), Math.max(a, b)] : null)
        break
      }
      case "band": {
        const nd = { ...d, x1: px, y1: py }
        drag.current = nd
        setBand(nd)
        const ids = notesIn(p.edit, msAt(d.x0), msAt(px), keyAt(d.y0), keyAt(py))
        const set = new Set(d.additive ? [...p.selection.notes, ...ids] : ids)
        p.onSelect({ ...EMPTY_SELECTION, notes: set })
        break
      }
      case "pan":
        setScrollMs(Math.max(0, d.scroll0 - (px - d.x0) / pxPerMs))
        setTopKey(Math.max(visibleKeys, Math.min(127, Math.round(d.top0 + (py - d.y0) / keyH))))
        break
    }
  }

  function onPointerUp() {
    const d = drag.current
    drag.current = null
    setBand(null)
    if (d && ["move", "resize", "draw", "control", "verse"].includes(d.kind)) p.onChange(p.edit, "end")
  }

  function onDoubleClick(e: React.MouseEvent) {
    const { px, py } = local(e)
    if (py >= rollBottom && verseAt(px) < 0) {
      const t = Math.round(snap(msAt(px), p.grid))
      const verses = [...(track.verses ?? []), t].sort((a, b) => a - b)
      p.onChange(p.edit, "start")
      p.onChange({ ...p.edit, track: { ...track, verses } }, "move")
      p.onChange(p.edit, "end")
    }
  }

  function onWheel(e: React.WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      const { px } = local(e)
      const at = msAt(px)
      const next = Math.max(5, Math.min(800, p.pxPerSec * (e.deltaY < 0 ? 1.15 : 1 / 1.15)))
      p.onZoom(next)
      setScrollMs(Math.max(0, at - (px - GUTTER) / (next / 1000)))
    } else if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      const dx = e.shiftKey ? e.deltaY : e.deltaX
      setScrollMs((s) => Math.max(0, s + dx / pxPerMs))
    } else {
      setTopKey((k) => Math.max(visibleKeys, Math.min(127, k - Math.sign(e.deltaY) * 3)))
    }
  }

  // Native non-passive wheel listener so ctrl+wheel zooms the roll, not the page.
  React.useEffect(() => {
    const c = canvas.current
    if (!c) return
    const stop = (e: WheelEvent) => e.preventDefault()
    c.addEventListener("wheel", stop, { passive: false })
    return () => c.removeEventListener("wheel", stop)
  }, [])

  const viewMs = (size.w - GUTTER) / pxPerMs
  const maxScroll = Math.max(0, track.duration_ms + 2000 - viewMs)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div ref={wrap} className="relative min-h-[360px] flex-1 overflow-hidden rounded-xl border">
        <canvas
          ref={canvas}
          className="absolute inset-0 size-full touch-none select-none"
          style={{ width: size.w, height: size.h, cursor: p.tool === "draw" ? "crosshair" : "default" }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onPointerLeave={() => setHover(null)}
          onDoubleClick={onDoubleClick}
          onContextMenu={onContextMenu}
          onWheel={onWheel}
          role="application"
          aria-label="Piano roll: notes, stop changes, swell pedal and verse markers"
        />
        {hover && (
          <div
            className="pointer-events-none absolute z-10 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground shadow-md ring-1 ring-foreground/10"
            style={{ left: Math.min(hover.x + 12, size.w - 220), top: hover.y + 14 }}
          >
            {hover.text}
          </div>
        )}
      </div>
      <input
        type="range"
        aria-label="Scroll through the track"
        min={0}
        max={Math.round(maxScroll)}
        value={Math.min(Math.round(scrollMs), Math.round(maxScroll))}
        onChange={(e) => setScrollMs(Number(e.target.value))}
        className="mt-2 w-full accent-primary"
      />
    </div>
  )
}
