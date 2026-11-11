"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter, useSearchParams } from "next/navigation"
import {
  AlertTriangleIcon,
  ArrowLeftIcon,
  ArrowDownIcon,
  ArrowUpIcon,
  CopyIcon,
  MonitorPlayIcon,
  PlayIcon,
  PlusIcon,
  TextIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { PageBody } from "@/components/page-body"
import { AddTracksDialog } from "@/components/playlists/add-tracks-dialog"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
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
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import {
  api,
  call,
  type Delay,
  type Playlist,
  type PlaylistRow,
  type TrackSummary,
} from "@/lib/api/client"
import { formatClock } from "@/lib/time"
import { cn } from "@/lib/utils"
import { formatVerses, parseVerses } from "@/lib/verses"

type TrackRow = Extract<PlaylistRow, { type: "track" }>

const DELAYS = [
  {
    value: "wait",
    label: "When you press Play",
    hint: "Loads it and waits for Play.",
  },
  {
    value: "after",
    label: "By itself",
    hint: "Plays after the pause you set.",
  },
  {
    value: "cue",
    label: "Ready at the first note",
    hint: "Sets the stops and waits, so Play starts the music at once.",
  },
]

function VersesInput({
  row,
  recorded,
  onChange,
}: {
  row: TrackRow
  recorded: number
  onChange: (r: TrackRow) => void
}) {
  // Follow the row when it changes from elsewhere (reordering, a save).
  const external = formatVerses(row.verses)
  const [text, setText] = React.useState(external)
  const [synced, setSynced] = React.useState(external)
  const parsed = parseVerses(text)
  if (external !== synced) {
    setSynced(external)
    if (!parsed.ok || formatVerses(parsed.plan) !== external) setText(external)
  }
  return (
    <Field data-invalid={!parsed.ok || undefined} className="w-28">
      <FieldLabel className="text-xs">Verses</FieldLabel>
      <Input
        value={text}
        inputMode="numeric"
        placeholder={recorded > 1 ? `All ${recorded}` : "1"}
        aria-invalid={!parsed.ok || undefined}
        onChange={(e) => {
          setText(e.target.value)
          const p = parseVerses(e.target.value)
          if (p.ok) onChange({ ...row, verses: p.plan })
        }}
        title='A number plays that many verses; "12357" plays exactly those'
      />
    </Field>
  )
}

function DelayInput({
  row,
  onChange,
}: {
  row: TrackRow
  onChange: (r: TrackRow) => void
}) {
  const delay = row.delay ?? { type: "wait" }
  const seconds = delay.type === "after" ? delay.seconds : 5
  const set = (d: Delay) => onChange({ ...row, delay: d })
  return (
    <div className="flex items-start gap-2">
      <Field className="w-52">
        <FieldLabel className="text-xs">Starts</FieldLabel>
        <Select
          value={delay.type}
          items={DELAYS}
          onValueChange={(v) =>
            set(
              v === "after"
                ? { type: "after", seconds }
                : v === "cue"
                  ? { type: "cue" }
                  : { type: "wait" }
            )
          }
        >
          <SelectTrigger className="w-full" aria-label="How it starts">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {DELAYS.map((d) => (
                <SelectItem key={d.value} value={d.value}>
                  {d.label}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
        <FieldDescription className="text-xs">
          {DELAYS.find((d) => d.value === delay.type)?.hint}
        </FieldDescription>
      </Field>
      {delay.type === "after" && (
        <Field className="w-24">
          <FieldLabel className="text-xs">After</FieldLabel>
          <InputGroup>
            <InputGroupInput
              type="number"
              min={0}
              max={600}
              value={seconds}
              onChange={(e) =>
                set({
                  type: "after",
                  seconds: Math.max(
                    0,
                    Math.min(600, Number(e.target.value) || 0)
                  ),
                })
              }
              aria-label="Seconds before it starts"
            />
            <InputGroupAddon align="inline-end">s</InputGroupAddon>
          </InputGroup>
        </Field>
      )}
    </div>
  )
}

function RowActions({
  index,
  count,
  onMove,
  onRemove,
  label,
}: {
  index: number
  count: number
  onMove: (to: number) => void
  onRemove: () => void
  label: string
}) {
  return (
    <div className="flex items-center">
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={() => onMove(index - 1)}
        disabled={index === 0}
        aria-label={`Move ${label} up`}
      >
        <ArrowUpIcon />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={() => onMove(index + 1)}
        disabled={index === count - 1}
        aria-label={`Move ${label} down`}
      >
        <ArrowDownIcon />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onRemove}
        aria-label={`Remove ${label}`}
      >
        <XIcon />
      </Button>
    </div>
  )
}

export function PlaylistEditor() {
  const id = useSearchParams().get("id") ?? ""
  const router = useRouter()
  const { connected, libraryVersion, service } = useEngine()
  const [playlist, setPlaylist] = React.useState<Playlist | null>(null)
  const [library, setLibrary] = React.useState<TrackSummary[]>([])
  const [missing, setMissing] = React.useState(false)
  const [saving, setSaving] = React.useState<"idle" | "saving" | "saved">(
    "idle"
  )
  const [adding, setAdding] = React.useState(false)
  const [deleting, setDeleting] = React.useState(false)
  const edits = React.useRef(0)

  React.useEffect(() => {
    if (!connected || !id) return
    call(api.GET("/api/v1/playlists/{id}", { params: { path: { id } } }))
      .then(setPlaylist)
      .catch(() => setMissing(true))
  }, [connected, id])

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/library"))
      .then(setLibrary)
      .catch(() => {})
  }, [connected, libraryVersion])

  // Save a moment after the last change.
  React.useEffect(() => {
    if (!playlist || edits.current === 0) return
    const version = edits.current
    setSaving("saving")
    const timer = setTimeout(async () => {
      try {
        const saved = await call(
          api.PUT("/api/v1/playlists/{id}", {
            params: { path: { id: playlist.id } },
            body: playlist,
          })
        )
        // Keep newer local edits; take the server's checked-at times otherwise.
        if (edits.current === version) setPlaylist(saved)
        setSaving("saved")
      } catch (e) {
        toast.error((e as Error).message)
        setSaving("idle")
      }
    }, 600)
    return () => clearTimeout(timer)
  }, [playlist])

  const update = (p: Playlist) => {
    edits.current += 1
    setPlaylist(p)
  }
  const setRows = (rows: PlaylistRow[]) =>
    playlist && update({ ...playlist, rows })
  const byId = React.useMemo(
    () => new Map(library.map((t) => [t.id, t])),
    [library]
  )

  if (!id || missing) {
    return (
      <PageBody>
        <Empty className="border">
          <EmptyHeader>
            <EmptyTitle>Playlist not found</EmptyTitle>
            <EmptyDescription>
              It may have been deleted.{" "}
              <Link href="/playlists">Back to playlists</Link>
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      </PageBody>
    )
  }
  if (!playlist) {
    return (
      <PageBody>
        <Skeleton className="h-10 w-72" />
        <Skeleton className="h-28" />
        <Skeleton className="h-28" />
      </PageBody>
    )
  }

  const rows = playlist.rows ?? []
  const move = (from: number, to: number) => {
    const next = [...rows]
    const [r] = next.splice(from, 1)
    next.splice(to, 0, r)
    setRows(next)
  }
  const remove = (i: number) => setRows(rows.filter((_, j) => j !== i))
  const replace = (i: number, r: PlaylistRow) =>
    setRows(rows.map((x, j) => (j === i ? r : x)))
  const running = service?.playlist_id === playlist.id

  async function run(row?: number, play = false) {
    if (!playlist) return
    try {
      await call(
        api.POST("/api/v1/service/start", {
          body: { playlist_id: playlist.id, row, play },
        })
      )
      router.push("/service")
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function duplicate() {
    if (!playlist) return
    try {
      const copy = await call(
        api.POST("/api/v1/playlists/{id}/duplicate", {
          params: { path: { id: playlist.id } },
        })
      )
      router.push(`/playlists/edit/?id=${encodeURIComponent(copy.id)}`)
      toast.success(`Made “${copy.name}”`)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function remove_playlist() {
    if (!playlist) return
    try {
      await call(
        api.DELETE("/api/v1/playlists/{id}", {
          params: { path: { id: playlist.id } },
        })
      )
      router.push("/playlists")
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  const totalMs = rows.reduce(
    (sum, r) =>
      sum + (r.type === "track" ? (byId.get(r.track_id)?.duration_ms ?? 0) : 0),
    0
  )

  return (
    <PageBody>
      <div className="flex items-end gap-2">
        <Button
          variant="ghost"
          size="icon"
          className="mb-0.5 shrink-0"
          render={<Link href="/playlists" />}
          nativeButton={false}
          aria-label="Back to playlists"
        >
          <ArrowLeftIcon />
        </Button>
        <Field className="min-w-0 flex-1">
          <div className="flex items-baseline justify-between gap-2">
            <FieldLabel htmlFor="playlist-name">Name</FieldLabel>
            <span className="text-xs text-muted-foreground" aria-live="polite">
              {saving === "saving"
                ? "Saving…"
                : saving === "saved"
                  ? "Saved"
                  : ""}
            </span>
          </div>
          <Input
            id="playlist-name"
            value={playlist.name}
            onChange={(e) => update({ ...playlist, name: e.target.value })}
            className="font-display text-lg md:text-lg"
          />
        </Field>
        <Button
          className="mb-0.5 shrink-0"
          onClick={() => run()}
          disabled={!rows.some((r) => r.type === "track")}
        >
          <MonitorPlayIcon data-icon="inline-start" />
          {running ? "Open service" : "Run service"}
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-x-6 gap-y-3 rounded-xl border bg-card px-4 py-3 text-sm">
        {(
          [
            ["repeat", "Repeat", "Start again after the last piece"],
            ["shuffle", "Shuffle", "Play the pieces in a random order"],
            [
              "silent",
              "Silent mode",
              "Leave the stops drawn when a piece ends",
            ],
          ] as const
        ).map(([key, label, hint]) => (
          <label key={key} className="flex items-center gap-2" title={hint}>
            <Switch
              checked={playlist[key]}
              onCheckedChange={(on) => update({ ...playlist, [key]: on })}
            />
            {label}
          </label>
        ))}
        <span className="ml-auto text-muted-foreground tabular-nums">
          {rows.filter((r) => r.type === "track").length} pieces · about{" "}
          {formatClock(totalMs)}
        </span>
      </div>

      {rows.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyTitle>Nothing in this playlist yet</EmptyTitle>
            <EmptyDescription>
              Add music from the library, and text lines for things like the
              sermon.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ol className="flex flex-col gap-2">
          {rows.map((row, i) => {
            const current = running && service?.row === i
            if (row.type === "text") {
              return (
                <li
                  key={i}
                  className="flex items-center gap-2 rounded-xl border border-dashed px-3 py-2"
                >
                  <TextIcon
                    className="size-4 shrink-0 text-muted-foreground"
                    aria-hidden
                  />
                  <Input
                    value={row.text}
                    onChange={(e) =>
                      replace(i, { ...row, text: e.target.value })
                    }
                    className="border-0 bg-transparent font-display text-base shadow-none focus-visible:ring-0 md:text-base"
                    aria-label="Text line"
                  />
                  <RowActions
                    index={i}
                    count={rows.length}
                    onMove={(to) => move(i, to)}
                    onRemove={() => remove(i)}
                    label="text line"
                  />
                </li>
              )
            }
            const t = byId.get(row.track_id)
            const stale =
              !!t && !!row.track_modified && t.modified > row.track_modified
            return (
              <li
                key={i}
                className={cn(
                  "flex flex-col gap-3 rounded-xl border bg-card p-3",
                  current && "border-primary ring-2 ring-primary/20"
                )}
              >
                <div className="flex items-start gap-2">
                  <Button
                    variant="secondary"
                    size="icon"
                    className="shrink-0 rounded-full"
                    onClick={() => run(i, true)}
                    aria-label={`Play from ${t?.info.title ?? "here"}`}
                    title="Run the service from here"
                  >
                    <PlayIcon />
                  </Button>
                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate font-display font-semibold">
                      {t?.info.title ?? "Missing piece"}
                    </span>
                    <span className="truncate text-xs text-muted-foreground">
                      {t
                        ? [
                            t.info.melody || t.info.composer,
                            formatClock(t.duration_ms),
                            t.verses > 1 && `${t.verses} verses recorded`,
                          ]
                            .filter(Boolean)
                            .join(" · ")
                        : "This piece is no longer in the library."}
                    </span>
                  </div>
                  {current && <Badge>Now</Badge>}
                  {stale && (
                    <Badge
                      variant="outline"
                      title="Edited after it was added. Saving the playlist clears this."
                    >
                      <AlertTriangleIcon data-icon="inline-start" />
                      Edited
                    </Badge>
                  )}
                  <RowActions
                    index={i}
                    count={rows.length}
                    onMove={(to) => move(i, to)}
                    onRemove={() => remove(i)}
                    label={t?.info.title ?? "piece"}
                  />
                </div>
                <div className="flex flex-wrap items-end gap-3 pl-0 sm:pl-10">
                  <DelayInput row={row} onChange={(r) => replace(i, r)} />
                  <VersesInput
                    row={row}
                    recorded={t?.verses ?? 1}
                    onChange={(r) => replace(i, r)}
                  />
                  <Field className="w-24">
                    <FieldLabel className="text-xs">Tempo</FieldLabel>
                    <InputGroup>
                      <InputGroupInput
                        type="number"
                        min={50}
                        max={150}
                        step={5}
                        placeholder="100"
                        value={row.tempo ?? ""}
                        onChange={(e) => {
                          const v =
                            e.target.value === ""
                              ? null
                              : Math.max(
                                  50,
                                  Math.min(150, Number(e.target.value) || 100)
                                )
                          replace(i, { ...row, tempo: v })
                        }}
                        aria-label="Tempo for this piece, percent"
                        title="Play this piece faster or slower than recorded"
                      />
                      <InputGroupAddon align="inline-end">%</InputGroupAddon>
                    </InputGroup>
                  </Field>
                  <Field className="min-w-40 flex-1">
                    <FieldLabel className="text-xs">Note</FieldLabel>
                    <Input
                      value={row.note ?? ""}
                      placeholder="e.g. Hymn 477"
                      onChange={(e) =>
                        replace(i, { ...row, note: e.target.value })
                      }
                    />
                  </Field>
                </div>
              </li>
            )
          })}
        </ol>
      )}

      <div className="flex flex-wrap gap-2">
        <Button variant="outline" onClick={() => setAdding(true)}>
          <PlusIcon data-icon="inline-start" />
          Add music
        </Button>
        <Button
          variant="outline"
          onClick={() => setRows([...rows, { type: "text", text: "" }])}
        >
          <TextIcon data-icon="inline-start" />
          Add text line
        </Button>
        <div className="ml-auto flex gap-2">
          <Button variant="ghost" onClick={duplicate}>
            <CopyIcon data-icon="inline-start" />
            Duplicate
          </Button>
          <Button variant="destructive" onClick={() => setDeleting(true)}>
            <Trash2Icon data-icon="inline-start" />
            Delete playlist
          </Button>
        </div>
      </div>

      <AddTracksDialog
        open={adding}
        onOpenChange={setAdding}
        library={library}
        onAdd={(ids) =>
          setRows([
            ...rows,
            ...ids.map((track_id): PlaylistRow => ({
              type: "track",
              track_id,
              delay: { type: "wait" },
            })),
          ])
        }
      />
      <AlertDialog open={deleting} onOpenChange={setDeleting}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete “{playlist.name}”?</AlertDialogTitle>
            <AlertDialogDescription>
              The music stays in the library.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep it</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={remove_playlist}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PageBody>
  )
}
