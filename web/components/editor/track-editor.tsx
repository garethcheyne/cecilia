"use client"

import * as React from "react"
import Link from "next/link"
import { useSearchParams } from "next/navigation"
import {
  ArrowLeftIcon,
  ClipboardPasteIcon,
  CopyIcon,
  MinusIcon,
  MousePointer2Icon,
  PencilIcon,
  PlayIcon,
  PlusIcon,
  Redo2Icon,
  SaveIcon,
  ScissorsIcon,
  SquareDashedIcon,
  Trash2Icon,
  Undo2Icon,
} from "lucide-react"
import { toast } from "sonner"

import {
  EditorCanvas,
  EMPTY_SELECTION,
  type ChangePhase,
  type ContextTarget,
  type Selection,
} from "@/components/editor/editor-canvas"
import { ScoreView } from "@/components/editor/score-view"
import { useEngine } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu"
import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { Field, FieldLabel } from "@/components/ui/field"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useTransport } from "@/hooks/use-transport"
import {
  api,
  call,
  type DivisionRole,
  type OrganDefinition,
} from "@/lib/api/client"
import {
  addControl,
  addNote,
  copyNotes,
  copyRange,
  flipStop,
  overlayClip,
  deleteControls,
  deleteNotes,
  deleteRange,
  fromEdit,
  History,
  insertTime,
  moveNotes,
  pasteClip,
  setDivision,
  toEdit,
  transpose,
  type Clip,
  type EditTrack,
} from "@/lib/edit"
import { noteName } from "@/lib/midi"
import { divisionName, stopLabel, stopRefOf } from "@/lib/stops"
import { formatClock } from "@/lib/time"

const GRIDS = [
  { value: "0", label: "No snap" },
  { value: "10", label: "10 ms" },
  { value: "50", label: "50 ms" },
  { value: "100", label: "0.1 s" },
  { value: "250", label: "¼ s" },
  { value: "beat", label: "Beat" },
  { value: "half", label: "½ beat" },
  { value: "quarter", label: "¼ beat" },
]

function IconButton({
  label,
  shortcut,
  ...props
}: React.ComponentProps<typeof Button> & { label: string; shortcut?: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button variant="ghost" size="icon" aria-label={label} {...props} />
        }
      />
      <TooltipContent>
        {label}
        {shortcut && (
          <span className="ml-2 text-muted-foreground">{shortcut}</span>
        )}
      </TooltipContent>
    </Tooltip>
  )
}

/** A heading in the right-click menu (outside any group). */
function MenuHeading({ children }: { children: React.ReactNode }) {
  return (
    <div className="truncate px-1.5 py-1 text-xs font-medium text-muted-foreground">
      {children}
    </div>
  )
}

// The clipboard survives switching tracks, so sections can be spliced
// between recordings.
let clipboard: Clip | null = null

export function TrackEditor() {
  const id = useSearchParams().get("id") ?? ""
  const { connected, simulator } = useEngine()
  const t = useTransport()
  const [edit, setEdit] = React.useState<EditTrack | null>(null)
  const [missing, setMissing] = React.useState(false)
  const [organ, setOrgan] = React.useState<OrganDefinition | null>(null)
  const [selection, setSelection] = React.useState<Selection>(EMPTY_SELECTION)
  const [cursor, setCursor] = React.useState(0)
  const [range, setRange] = React.useState<[number, number] | null>(null)
  const [tool, setTool] = React.useState<"select" | "draw">("select")
  const [drawDivision, setDrawDivision] = React.useState<DivisionRole>("great")
  const [pxPerSec, setPxPerSec] = React.useState(80)
  const [gridChoice, setGridChoice] = React.useState("0")
  const [dirty, setDirty] = React.useState(false)
  const [saving, setSaving] = React.useState(false)
  const [view, setView] = React.useState<"roll" | "score">("roll")
  const [silence, setSilence] = React.useState(2)
  const [canUndo, setCanUndo] = React.useState(false)
  const [canRedo, setCanRedo] = React.useState(false)
  const history = React.useRef(new History<EditTrack>())
  const dragBase = React.useRef<EditTrack | null>(null)
  const force = () => {
    setCanUndo(history.current.canUndo)
    setCanRedo(history.current.canRedo)
  }

  React.useEffect(() => {
    if (!connected || !id) return
    call(api.GET("/api/v1/library/{id}", { params: { path: { id } } }))
      .then((tr) => {
        setEdit(toEdit(tr))
        if (tr.meter) setGridChoice("half")
      })
      .catch(() => setMissing(true))
  }, [connected, id])

  React.useEffect(() => {
    const organId = simulator?.organ_id
    if (!organId) return
    call(api.GET("/api/v1/organs/{id}", { params: { path: { id: organId } } }))
      .then(setOrgan)
      .catch(() => {})
  }, [simulator?.organ_id])

  // Warn before leaving with unsaved changes.
  React.useEffect(() => {
    if (!dirty) return
    const warn = (e: BeforeUnloadEvent) => e.preventDefault()
    window.addEventListener("beforeunload", warn)
    return () => window.removeEventListener("beforeunload", warn)
  }, [dirty])

  const meter = edit?.track.meter
  const beat = meter ? (60000 / meter.bpm) * (4 / meter.beat_unit) : 500
  const grid =
    gridChoice === "beat"
      ? beat
      : gridChoice === "half"
        ? beat / 2
        : gridChoice === "quarter"
          ? beat / 4
          : Number(gridChoice)

  const apply = React.useCallback(
    (fn: (e: EditTrack) => EditTrack) => {
      if (!edit) return
      history.current.push(edit)
      setEdit(fn(edit))
      setDirty(true)
      force()
    },
    [edit]
  )

  const onChange = (next: EditTrack, phase: ChangePhase) => {
    if (phase === "start") dragBase.current = edit
    else if (phase === "move") setEdit(next)
    else if (phase === "end") {
      if (dragBase.current && dragBase.current !== next) {
        history.current.push(dragBase.current)
        setDirty(true)
        force()
      }
      dragBase.current = null
    }
  }

  const undo = React.useCallback(() => {
    if (!edit) return
    const prev = history.current.undo(edit)
    if (prev) {
      setEdit(prev)
      setDirty(true)
      force()
    }
  }, [edit])
  const redo = React.useCallback(() => {
    if (!edit) return
    const next = history.current.redo(edit)
    if (next) {
      setEdit(next)
      setDirty(true)
      force()
    }
  }, [edit])

  const save = React.useCallback(async () => {
    if (!edit) return false
    setSaving(true)
    try {
      const track = fromEdit(edit)
      await call(
        api.PUT("/api/v1/library/{id}", {
          params: { path: { id: track.id } },
          body: track,
        })
      )
      setDirty(false)
      toast.success("Saved")
      return true
    } catch (e) {
      toast.error((e as Error).message)
      return false
    } finally {
      setSaving(false)
    }
  }, [edit])

  const playFrom = React.useCallback(
    async (ms: number) => {
      if (!edit) return
      if (dirty && !(await save())) return
      t.load(edit.track.id, { start_ms: Math.round(ms) })
    },
    [edit, dirty, save, t]
  )
  const playFromCursor = React.useCallback(
    () => playFrom(cursor),
    [playFrom, cursor]
  )
  const [ctx, setCtx] = React.useState<ContextTarget | null>(null)

  const deleteSelection = React.useCallback(() => {
    if (selection.notes.size) apply((e) => deleteNotes(e, selection.notes))
    else if (selection.controls.size)
      apply((e) => deleteControls(e, selection.controls))
    else if (selection.verse != null) {
      const i = selection.verse
      apply((e) => ({
        ...e,
        track: {
          ...e.track,
          verses: (e.track.verses ?? []).filter((_, j) => j !== i),
        },
      }))
    }
    setSelection(EMPTY_SELECTION)
  }, [selection, apply])

  // Keyboard shortcuts
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (
        (e.target as HTMLElement).closest(
          "input, textarea, select, [contenteditable], [role=dialog]"
        )
      )
        return
      const mod = e.ctrlKey || e.metaKey
      if (mod && e.key.toLowerCase() === "z") {
        e.preventDefault()
        if (e.shiftKey) redo()
        else undo()
      } else if (mod && e.key.toLowerCase() === "y") {
        e.preventDefault()
        redo()
      } else if (mod && e.key.toLowerCase() === "s") {
        e.preventDefault()
        void save()
      } else if (mod && e.key.toLowerCase() === "a" && edit) {
        e.preventDefault()
        setSelection({
          ...EMPTY_SELECTION,
          notes: new Set(edit.notes.map((n) => n.id)),
        })
      } else if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault()
        deleteSelection()
      } else if (
        (e.key === "ArrowUp" || e.key === "ArrowDown") &&
        selection.notes.size
      ) {
        e.preventDefault()
        const by = (e.key === "ArrowUp" ? 1 : -1) * (e.shiftKey ? 12 : 1)
        apply((x) => transpose(x, selection.notes, by))
      } else if (
        (e.key === "ArrowLeft" || e.key === "ArrowRight") &&
        selection.notes.size
      ) {
        e.preventDefault()
        const by = (e.key === "ArrowRight" ? 1 : -1) * (grid || 50)
        apply((x) => moveNotes(x, selection.notes, by, 0))
      } else if (e.code === "Space") {
        e.preventDefault()
        if (t.player?.state === "playing" && t.player.track_id === id) t.pause()
        else void playFromCursor()
      } else if (e.key.toLowerCase() === "d" && !mod) {
        setTool((x) => (x === "draw" ? "select" : "draw"))
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [
    undo,
    redo,
    save,
    deleteSelection,
    selection,
    apply,
    grid,
    edit,
    t,
    id,
    playFromCursor,
  ])

  if (!id || missing) {
    return (
      <div className="p-4">
        <Empty className="border">
          <EmptyHeader>
            <EmptyTitle>Piece not found</EmptyTitle>
            <EmptyDescription>
              It may have been deleted.{" "}
              <Link href="/library">Back to the library</Link>
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      </div>
    )
  }
  if (!edit) {
    return (
      <div className="flex flex-col gap-3 p-4">
        <Skeleton className="h-10 w-80" />
        <Skeleton className="h-[60svh]" />
      </div>
    )
  }

  const track = edit.track
  const playhead =
    t.player?.track_id === id && t.player.state !== "stopped"
      ? t.position
      : null
  const selNotes = edit.notes.filter((n) => selection.notes.has(n.id))
  const selControl = edit.controls.find((c) => selection.controls.has(c.id))
  const divisions: DivisionRole[] = organ?.divisions.map((d) => d.role) ?? [
    "great",
    "swell",
    "pedal",
  ]

  const at = (ms: number) => formatClock(ms)
  const sectionItems = range && (
    <>
      <ContextMenuSeparator />
      <ContextMenuGroup>
        <ContextMenuLabel>
          Section {at(range[0])} to {at(range[1])}
        </ContextMenuLabel>
        <ContextMenuItem
          onClick={() => {
            clipboard = copyRange(edit, range[0], range[1])
            toast("Section copied")
          }}
        >
          <CopyIcon />
          Copy section
        </ContextMenuItem>
        <ContextMenuItem
          onClick={() => {
            clipboard = copyRange(edit, range[0], range[1])
            apply((e) => deleteRange(e, range[0], range[1]))
            setRange(null)
          }}
        >
          <ScissorsIcon />
          Cut out section
        </ContextMenuItem>
        <ContextMenuItem
          onClick={() =>
            apply((e) => deleteRange(e, range[0], range[1], ["notes"], false))
          }
        >
          <SquareDashedIcon />
          Clear notes in section
        </ContextMenuItem>
      </ContextMenuGroup>
    </>
  )
  const menuItems = (() => {
    if (!ctx) return null
    const play = (
      <ContextMenuItem onClick={() => void playFrom(ctx.ms)}>
        <PlayIcon />
        Play from here
        <ContextMenuShortcut>Space</ContextMenuShortcut>
      </ContextMenuItem>
    )
    switch (ctx.kind) {
      case "note": {
        const n = selection.notes.size
        return (
          <>
            <MenuHeading>
              {n > 1 ? `${n} notes` : noteName(ctx.key)}
            </MenuHeading>
            <ContextMenuGroup>
              <ContextMenuSub>
                <ContextMenuSubTrigger>Move to keyboard</ContextMenuSubTrigger>
                <ContextMenuSubContent>
                  {divisions.map((d) => (
                    <ContextMenuItem
                      key={d}
                      onClick={() =>
                        apply((e) => setDivision(e, selection.notes, d))
                      }
                    >
                      {divisionName(d, organ)}
                    </ContextMenuItem>
                  ))}
                </ContextMenuSubContent>
              </ContextMenuSub>
              <ContextMenuSub>
                <ContextMenuSubTrigger>Transpose</ContextMenuSubTrigger>
                <ContextMenuSubContent>
                  <ContextMenuItem
                    onClick={() =>
                      apply((e) => transpose(e, selection.notes, 1))
                    }
                  >
                    Up a semitone
                    <ContextMenuShortcut>↑</ContextMenuShortcut>
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      apply((e) => transpose(e, selection.notes, -1))
                    }
                  >
                    Down a semitone
                    <ContextMenuShortcut>↓</ContextMenuShortcut>
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      apply((e) => transpose(e, selection.notes, 12))
                    }
                  >
                    Up an octave
                    <ContextMenuShortcut>Shift ↑</ContextMenuShortcut>
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      apply((e) => transpose(e, selection.notes, -12))
                    }
                  >
                    Down an octave
                    <ContextMenuShortcut>Shift ↓</ContextMenuShortcut>
                  </ContextMenuItem>
                </ContextMenuSubContent>
              </ContextMenuSub>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuGroup>
              <ContextMenuItem
                onClick={() => {
                  clipboard = copyNotes(edit, selection.notes)
                  toast(n > 1 ? `${n} notes copied` : "Note copied")
                }}
              >
                <CopyIcon />
                Copy
              </ContextMenuItem>
              <ContextMenuItem
                onClick={() => {
                  const clip = copyNotes(edit, selection.notes)
                  const start = Math.min(
                    ...edit.notes
                      .filter((x) => selection.notes.has(x.id))
                      .map((x) => x.start)
                  )
                  const [next, added] = overlayClip(
                    edit,
                    clip,
                    start + clip.length
                  )
                  apply(() => next)
                  setSelection({ ...EMPTY_SELECTION, notes: new Set(added) })
                }}
              >
                <ClipboardPasteIcon />
                Duplicate after
              </ContextMenuItem>
              {play}
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuItem variant="destructive" onClick={deleteSelection}>
              <Trash2Icon />
              Delete
              <ContextMenuShortcut>Del</ContextMenuShortcut>
            </ContextMenuItem>
          </>
        )
      }
      case "stop": {
        const c = edit.controls.find((x) => x.id === ctx.id)
        if (!c || c.type !== "stop") return null
        return (
          <>
            <MenuHeading>
              {stopLabel(c.stop, organ)} {c.on ? "drawn" : "pushed in"} at{" "}
              {at(c.t)}
            </MenuHeading>
            <ContextMenuGroup>
              <ContextMenuItem
                onClick={() => apply((e) => flipStop(e, ctx.id))}
              >
                {c.on ? "Push it in instead" : "Draw it instead"}
              </ContextMenuItem>
              {play}
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuItem variant="destructive" onClick={deleteSelection}>
              <Trash2Icon />
              Delete this stop change
            </ContextMenuItem>
          </>
        )
      }
      case "verse":
        return (
          <>
            <MenuHeading>
              Start of verse {ctx.index + 2}, at {at(ctx.ms)}
            </MenuHeading>
            <ContextMenuGroup>{play}</ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuItem variant="destructive" onClick={deleteSelection}>
              <Trash2Icon />
              Remove verse marker
            </ContextMenuItem>
          </>
        )
      default: {
        const here = ctx.ms
        const clip = clipboard
        return (
          <>
            <MenuHeading>At {at(here)}</MenuHeading>
            <ContextMenuGroup>
              {ctx.kind === "roll" && (
                <ContextMenuItem
                  onClick={() => {
                    const len = grid > 0 ? grid : 500
                    const [next, id] = addNote(edit, {
                      division: drawDivision,
                      key: ctx.key,
                      start: here,
                      end: here + len,
                      velocity: 80,
                    })
                    apply(() => next)
                    setSelection({ ...EMPTY_SELECTION, notes: new Set([id]) })
                  }}
                >
                  <PlusIcon />
                  Add {noteName(ctx.key)} on the{" "}
                  {divisionName(drawDivision, organ)}
                </ContextMenuItem>
              )}
              {clip && clip.notes.length > 0 && (
                <ContextMenuItem
                  onClick={() => {
                    const [next, added] = overlayClip(edit, clip, here)
                    apply(() => next)
                    setSelection({ ...EMPTY_SELECTION, notes: new Set(added) })
                  }}
                >
                  <ClipboardPasteIcon />
                  Paste here
                </ContextMenuItem>
              )}
              {clip && (
                <ContextMenuItem
                  onClick={() => apply((e) => pasteClip(e, clip, here))}
                >
                  <ClipboardPasteIcon />
                  Paste here, making room
                </ContextMenuItem>
              )}
              {organ && (
                <ContextMenuSub>
                  <ContextMenuSubTrigger>
                    Add a stop change here
                  </ContextMenuSubTrigger>
                  <ContextMenuSubContent className="max-h-80 overflow-y-auto">
                    {organ.stops.map((st) => (
                      <ContextMenuItem
                        key={st.id}
                        onClick={() => {
                          const ref = stopRefOf(organ, st.id)
                          if (!ref) return
                          const on = !isOnAt(fromEdit(edit), ref, here)
                          apply(
                            (e) =>
                              addControl(e, {
                                t: Math.round(here),
                                type: "stop",
                                stop: ref,
                                on,
                              })[0]
                          )
                        }}
                      >
                        {stopLabel(
                          { division: st.division, equivalent: st.equivalent },
                          organ
                        )}
                      </ContextMenuItem>
                    ))}
                  </ContextMenuSubContent>
                </ContextMenuSub>
              )}
              <ContextMenuItem
                onClick={() =>
                  apply((e) => ({
                    ...e,
                    track: {
                      ...e.track,
                      verses: [
                        ...(e.track.verses ?? []),
                        Math.round(here),
                      ].sort((a, b) => a - b),
                    },
                  }))
                }
              >
                Start a new verse here
              </ContextMenuItem>
              <ContextMenuItem
                onClick={() =>
                  apply((e) => insertTime(e, here, Math.round(silence * 1000)))
                }
              >
                Insert {silence} s of silence here
              </ContextMenuItem>
              {play}
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuItem
              onClick={() =>
                setSelection({
                  ...EMPTY_SELECTION,
                  notes: new Set(edit.notes.map((x) => x.id)),
                })
              }
            >
              Select all notes
              <ContextMenuShortcut>Ctrl+A</ContextMenuShortcut>
            </ContextMenuItem>
          </>
        )
      }
    }
  })()

  const inspector = (
    <div className="flex flex-col gap-4 text-sm">
      {selNotes.length > 0 ? (
        <>
          <p className="font-medium">
            {selNotes.length === 1
              ? `${noteName(selNotes[0].key)} at ${formatClock(selNotes[0].start)}`
              : `${selNotes.length} notes selected`}
          </p>
          {selNotes.length === 1 && (
            <p className="text-muted-foreground">
              Length {((selNotes[0].end - selNotes[0].start) / 1000).toFixed(2)}{" "}
              s
            </p>
          )}
          <Field>
            <FieldLabel>Keyboard</FieldLabel>
            <Select
              value={
                selNotes.every((n) => n.division === selNotes[0].division)
                  ? selNotes[0].division
                  : ""
              }
              items={divisions.map((d) => ({
                value: d,
                label: divisionName(d, organ),
              }))}
              onValueChange={(v) =>
                v &&
                apply((e) => setDivision(e, selection.notes, v as DivisionRole))
              }
            >
              <SelectTrigger className="w-full" aria-label="Move to keyboard">
                <SelectValue placeholder="Mixed" />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {divisions.map((d) => (
                    <SelectItem key={d} value={d}>
                      {divisionName(d, organ)}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => apply((e) => transpose(e, selection.notes, -1))}
            >
              <MinusIcon data-icon="inline-start" />
              Semitone
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => apply((e) => transpose(e, selection.notes, 1))}
            >
              <PlusIcon data-icon="inline-start" />
              Semitone
            </Button>
            <Button variant="outline" size="sm" onClick={deleteSelection}>
              <Trash2Icon data-icon="inline-start" />
              Delete
            </Button>
          </div>
        </>
      ) : selControl ? (
        <>
          <p className="font-medium">
            {selControl.type === "stop"
              ? `${stopLabel(selControl.stop, organ)} ${selControl.on ? "on" : "off"}`
              : selControl.type === "expression"
                ? `${divisionName(selControl.division, organ)} pedal ${Math.round((selControl.value / 127) * 100)}%`
                : `Volume ${selControl.value}`}
          </p>
          <p className="text-muted-foreground">
            At {formatClock(selControl.t)}. Drag the marker to move it.
          </p>
          <Button
            variant="outline"
            size="sm"
            className="self-start"
            onClick={deleteSelection}
          >
            <Trash2Icon data-icon="inline-start" />
            Delete
          </Button>
        </>
      ) : selection.verse != null ? (
        <>
          <p className="font-medium">Start of verse {selection.verse + 2}</p>
          <p className="text-muted-foreground">
            At {formatClock(track.verses?.[selection.verse] ?? 0)}. Drag to move
            it into the silence between verses.
          </p>
          <Button
            variant="outline"
            size="sm"
            className="self-start"
            onClick={deleteSelection}
          >
            <Trash2Icon data-icon="inline-start" />
            Remove marker
          </Button>
        </>
      ) : (
        <>
          <p className="text-muted-foreground">
            Click a note, stop marker or verse marker to change it. Drag along
            the time ruler to select a section.
          </p>
          <p className="text-xs font-medium text-muted-foreground">
            Transpose the whole piece
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => apply((e) => transpose(e, new Set(), -1))}
            >
              <MinusIcon data-icon="inline-start" />
              Down a semitone
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => apply((e) => transpose(e, new Set(), 1))}
            >
              <PlusIcon data-icon="inline-start" />
              Up a semitone
            </Button>
          </div>
        </>
      )}

      <Separator />
      <div className="flex flex-col gap-2">
        <p className="font-medium">
          {range
            ? `Section ${formatClock(range[0])} – ${formatClock(range[1])}`
            : `At the cursor (${formatClock(cursor)})`}
        </p>
        {range ? (
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                clipboard = copyRange(edit, range[0], range[1])
                toast("Section copied")
              }}
            >
              <CopyIcon data-icon="inline-start" />
              Copy
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                clipboard = copyRange(edit, range[0], range[1])
                apply((e) => deleteRange(e, range[0], range[1]))
                setRange(null)
              }}
            >
              <ScissorsIcon data-icon="inline-start" />
              Cut out
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                apply((e) =>
                  deleteRange(e, range[0], range[1], ["notes"], false)
                )
              }
            >
              <SquareDashedIcon data-icon="inline-start" />
              Clear notes
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                apply((e) =>
                  deleteRange(
                    e,
                    range[0],
                    range[1],
                    ["stops", "expression"],
                    false
                  )
                )
              }
            >
              Clear stops and pedal
            </Button>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={!clipboard}
                onClick={() =>
                  clipboard && apply((e) => pasteClip(e, clipboard!, cursor))
                }
              >
                <ClipboardPasteIcon data-icon="inline-start" />
                Paste section
              </Button>
              {!clipboard && (
                <span className="self-center text-xs text-muted-foreground">
                  Select and copy a section first.
                </span>
              )}
            </div>
            <div className="flex items-center gap-2">
              <InputGroup className="w-32">
                <InputGroupInput
                  type="number"
                  min={0.1}
                  max={60}
                  step={0.5}
                  value={silence}
                  onChange={(e) => setSilence(Number(e.target.value) || 1)}
                  aria-label="Seconds of silence"
                />
                <InputGroupAddon align="inline-end">seconds</InputGroupAddon>
              </InputGroup>
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  apply((e) =>
                    insertTime(e, cursor, Math.round(silence * 1000))
                  )
                }
              >
                Insert silence
              </Button>
            </div>
            {organ && (
              <Field>
                <FieldLabel>Add a stop change here</FieldLabel>
                <Select
                  value=""
                  items={organ.stops.map((s) => ({
                    value: s.id,
                    label: stopLabel(
                      { division: s.division, equivalent: s.equivalent },
                      organ
                    ),
                  }))}
                  onValueChange={(v) => {
                    const ref = v ? stopRefOf(organ, String(v)) : null
                    if (!ref) return
                    // Toggle relative to the state at the cursor.
                    const before = fromEdit(edit)
                    const on = !isOnAt(before, ref, cursor)
                    apply(
                      (e) =>
                        addControl(e, {
                          t: Math.round(cursor),
                          type: "stop",
                          stop: ref,
                          on,
                        })[0]
                    )
                  }}
                >
                  <SelectTrigger
                    className="w-full"
                    aria-label="Add a stop change"
                  >
                    <SelectValue placeholder="Choose a stop…" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {organ.stops.map((s) => (
                        <SelectItem key={s.id} value={s.id}>
                          {stopLabel(
                            { division: s.division, equivalent: s.equivalent },
                            organ
                          )}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
            )}
          </div>
        )}
      </div>
    </div>
  )

  return (
    <div className="flex min-h-0 w-full min-w-0 flex-1 flex-col gap-3 p-3 sm:p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="ghost"
          size="icon"
          render={<Link href="/library" />}
          nativeButton={false}
          aria-label="Back to the library"
        >
          <ArrowLeftIcon />
        </Button>
        <div className="flex min-w-0 flex-col">
          <h2 className="truncate font-display text-lg font-semibold">
            {track.info.title}
          </h2>
          <span className="text-xs text-muted-foreground">
            {formatClock(track.duration_ms)} · {edit.notes.length} notes ·{" "}
            {(track.verses?.length ?? 0) + 1} verse
            {(track.verses?.length ?? 0) > 0 ? "s" : ""}
          </span>
        </div>
        {dirty && <Badge variant="outline">Unsaved</Badge>}
        <div className="ml-auto flex flex-wrap items-center gap-1">
          <Tabs
            value={view}
            onValueChange={(v) => setView(v as "roll" | "score")}
          >
            <TabsList>
              <TabsTrigger value="roll">Piano roll</TabsTrigger>
              <TabsTrigger value="score">Score</TabsTrigger>
            </TabsList>
          </Tabs>
          <IconButton
            label="Undo"
            shortcut="Ctrl+Z"
            onClick={undo}
            disabled={!canUndo}
          >
            <Undo2Icon />
          </IconButton>
          <IconButton
            label="Redo"
            shortcut="Ctrl+Y"
            onClick={redo}
            disabled={!canRedo}
          >
            <Redo2Icon />
          </IconButton>
          <IconButton
            label="Play from the cursor"
            shortcut="Space"
            onClick={() => void playFromCursor()}
          >
            <PlayIcon />
          </IconButton>
          <Button onClick={() => void save()} disabled={!dirty || saving}>
            <SaveIcon data-icon="inline-start" />
            Save
          </Button>
        </div>
      </div>

      {view === "roll" ? (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <ToggleGroup
              value={[tool]}
              onValueChange={(v) => v[0] && setTool(v[0] as "select" | "draw")}
              variant="outline"
              size="sm"
              aria-label="Tool"
            >
              <ToggleGroupItem value="select" aria-label="Select and move">
                <MousePointer2Icon />
                Select
              </ToggleGroupItem>
              <ToggleGroupItem value="draw" aria-label="Draw notes">
                <PencilIcon />
                Draw
              </ToggleGroupItem>
            </ToggleGroup>
            {tool === "draw" && (
              <Select
                value={drawDivision}
                items={divisions.map((d) => ({
                  value: d,
                  label: divisionName(d, organ),
                }))}
                onValueChange={(v) => v && setDrawDivision(v as DivisionRole)}
              >
                <SelectTrigger
                  size="sm"
                  className="w-28"
                  aria-label="Keyboard for new notes"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {divisions.map((d) => (
                      <SelectItem key={d} value={d}>
                        {divisionName(d, organ)}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            )}
            <Select
              value={gridChoice}
              items={GRIDS}
              onValueChange={(v) => v && setGridChoice(String(v))}
            >
              <SelectTrigger size="sm" className="w-28" aria-label="Snap">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {GRIDS.filter(
                    (g) =>
                      meter || !["beat", "half", "quarter"].includes(g.value)
                  ).map((g) => (
                    <SelectItem key={g.value} value={g.value}>
                      {g.label}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
            <div className="ml-auto flex items-center gap-1">
              <IconButton
                label="Zoom out"
                shortcut="Ctrl+wheel"
                onClick={() => setPxPerSec((z) => Math.max(5, z / 1.4))}
              >
                <MinusIcon />
              </IconButton>
              <IconButton
                label="Zoom in"
                shortcut="Ctrl+wheel"
                onClick={() => setPxPerSec((z) => Math.min(800, z * 1.4))}
              >
                <PlusIcon />
              </IconButton>
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col gap-3 lg:flex-row">
            <ContextMenu>
              <ContextMenuTrigger
                render={
                  <div className="flex min-h-[420px] min-w-0 flex-1 flex-col" />
                }
              >
                <EditorCanvas
                  edit={edit}
                  selection={selection}
                  onSelect={setSelection}
                  cursor={cursor}
                  onCursor={setCursor}
                  range={range}
                  onRange={setRange}
                  tool={tool}
                  drawDivision={drawDivision}
                  pxPerSec={pxPerSec}
                  onZoom={setPxPerSec}
                  grid={grid}
                  playhead={playhead}
                  onChange={onChange}
                  organ={organ}
                  onContextTarget={setCtx}
                />
              </ContextMenuTrigger>
              <ContextMenuContent className="min-w-56">
                {menuItems}
                {sectionItems}
              </ContextMenuContent>
            </ContextMenu>
            <aside
              className="w-full shrink-0 rounded-xl border bg-card p-4 lg:w-72"
              aria-label="Inspector"
            >
              {inspector}
            </aside>
          </div>
        </>
      ) : (
        <ScoreView track={fromEdit(edit)} position={playhead} />
      )}
    </div>
  )
}

/** Is `ref` drawn at time `t` in `track`? */
function isOnAt(
  track: ReturnType<typeof fromEdit>,
  ref: { division: string; equivalent: unknown },
  t: number
): boolean {
  const key = JSON.stringify(ref)
  let on = (track.initial.stops ?? []).some((s) => JSON.stringify(s) === key)
  for (const e of track.events) {
    if (e.t > t) break
    if (e.type === "stop" && JSON.stringify(e.stop) === key) on = e.on
  }
  return on
}
