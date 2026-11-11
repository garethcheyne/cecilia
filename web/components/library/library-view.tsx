"use client"

import * as React from "react"
import Link from "next/link"
import {
  ArrowDownIcon,
  ArrowUpIcon,
  AudioLinesIcon,
  CopyIcon,
  DownloadIcon,
  EllipsisVerticalIcon,
  FileDownIcon,
  LibraryIcon,
  PencilIcon,
  PianoIcon,
  PlayIcon,
  SearchIcon,
  Trash2Icon,
  UploadIcon,
} from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { ImportDialog } from "@/components/library/import-dialog"
import { TrackDetailsDialog } from "@/components/library/track-details-dialog"
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
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Skeleton } from "@/components/ui/skeleton"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { useTransport } from "@/hooks/use-transport"
import { api, apiUrl, call, type TrackSummary } from "@/lib/api/client"
import { formatClock, formatDate } from "@/lib/time"
import { cn } from "@/lib/utils"

type SortKey = "title" | "duration" | "added"

const SOURCE_LABEL: Record<TrackSummary["source"], string> = {
  recorded: "Recorded",
  sample: "Sample",
  imported: "Imported",
}

function matches(t: TrackSummary, q: string) {
  if (!q) return true
  const i = t.info
  return [i.title, i.composer, i.melody, i.performer, i.publication]
    .filter(Boolean)
    .some((f) => f!.toLowerCase().includes(q))
}

function sortTracks(list: TrackSummary[], key: SortKey, asc: boolean) {
  const dir = asc ? 1 : -1
  return [...list].sort((a, b) => {
    switch (key) {
      case "title":
        return dir * a.info.title.localeCompare(b.info.title)
      case "duration":
        return dir * (a.duration_ms - b.duration_ms)
      case "added":
        return dir * (a.created - b.created) || a.info.title.localeCompare(b.info.title)
    }
  })
}

function SortHeader({
  label,
  k,
  sort,
  onSort,
  className,
}: {
  label: string
  k: SortKey
  sort: { key: SortKey; asc: boolean }
  onSort: (k: SortKey) => void
  className?: string
}) {
  const active = sort.key === k
  return (
    <TableHead className={className} aria-sort={active ? (sort.asc ? "ascending" : "descending") : "none"}>
      <Button variant="ghost" size="sm" className="-ml-2.5" onClick={() => onSort(k)}>
        {label}
        {active && (sort.asc ? <ArrowUpIcon data-icon="inline-end" /> : <ArrowDownIcon data-icon="inline-end" />)}
      </Button>
    </TableHead>
  )
}

function TrackMenu({
  track,
  onEdit,
  onDuplicate,
  onDelete,
}: {
  track: TrackSummary
  onEdit: () => void
  onDuplicate: () => void
  onDelete: () => void
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button variant="ghost" size="icon" aria-label={`More for ${track.info.title}`}>
            <EllipsisVerticalIcon />
          </Button>
        }
      />
      <DropdownMenuContent align="end">
        <DropdownMenuGroup>
          <DropdownMenuItem render={<Link href={`/library/edit/?id=${encodeURIComponent(track.id)}`} />}>
            <PianoIcon />
            Open in editor
          </DropdownMenuItem>
          <DropdownMenuItem onClick={onEdit}>
            <PencilIcon />
            Edit details
          </DropdownMenuItem>
          <DropdownMenuItem onClick={onDuplicate}>
            <CopyIcon />
            Duplicate
          </DropdownMenuItem>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem render={<a href={apiUrl(`/api/v1/library/${encodeURIComponent(track.id)}/midi`)} download />}>
            <FileDownIcon />
            MIDI for this organ
          </DropdownMenuItem>
          <DropdownMenuItem
            render={<a href={apiUrl(`/api/v1/library/${encodeURIComponent(track.id)}/midi?format=general_midi`)} download />}
          >
            <FileDownIcon />
            General MIDI
          </DropdownMenuItem>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem variant="destructive" onClick={onDelete}>
            <Trash2Icon />
            Delete
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function Playing() {
  return <AudioLinesIcon className="size-4 animate-pulse text-primary" aria-label="Playing" />
}

export function LibraryView() {
  const { libraryVersion, connected } = useEngine()
  const t = useTransport()
  const [tracks, setTracks] = React.useState<TrackSummary[] | null>(null)
  const [query, setQuery] = React.useState("")
  const [sort, setSort] = React.useState<{ key: SortKey; asc: boolean }>({ key: "added", asc: false })
  const [editing, setEditing] = React.useState<TrackSummary | null>(null)
  const [deleting, setDeleting] = React.useState<TrackSummary | null>(null)
  const [importing, setImporting] = React.useState(false)

  React.useEffect(() => {
    if (!connected) return
    let cancelled = false
    call(api.GET("/api/v1/library"))
      .then((list) => !cancelled && setTracks(list))
      .catch((e: Error) => toast.error(e.message))
    return () => {
      cancelled = true
    }
  }, [libraryVersion, connected])

  const shown = React.useMemo(() => {
    if (!tracks) return []
    const q = query.trim().toLowerCase()
    return sortTracks(tracks.filter((tr) => matches(tr, q)), sort.key, sort.asc)
  }, [tracks, query, sort])

  const onSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, asc: !s.asc } : { key, asc: key === "title" }))

  const playingId = t.player?.state !== "stopped" ? t.player?.track_id : null

  async function duplicate(tr: TrackSummary) {
    try {
      const copy = await call(api.POST("/api/v1/library/{id}/duplicate", { params: { path: { id: tr.id } } }))
      toast.success(`Made “${copy.info.title}”`)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function remove(tr: TrackSummary) {
    try {
      await call(api.DELETE("/api/v1/library/{id}", { params: { path: { id: tr.id } } }))
      toast.success(`Deleted “${tr.info.title}”`)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  const menu = (tr: TrackSummary) => (
    <TrackMenu
      track={tr}
      onEdit={() => setEditing(tr)}
      onDuplicate={() => duplicate(tr)}
      onDelete={() => setDeleting(tr)}
    />
  )

  const playButton = (tr: TrackSummary) => (
    <Button
      variant={playingId === tr.id ? "secondary" : "ghost"}
      size="icon"
      aria-label={`Play ${tr.info.title}`}
      onClick={() => t.load(tr.id)}
    >
      {playingId === tr.id ? <Playing /> : <PlayIcon />}
    </Button>
  )

  return (
    <div className="mx-auto flex w-full min-w-0 max-w-6xl flex-col gap-4 p-3 sm:p-4">
      <div className="flex flex-wrap items-center gap-3">
        <InputGroup className="w-full sm:max-w-sm">
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            placeholder="Search titles, composers, tunes"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Search the library"
          />
        </InputGroup>
        {tracks && (
          <span className="text-sm text-muted-foreground">
            {shown.length === tracks.length ? `${tracks.length} pieces` : `${shown.length} of ${tracks.length}`}
          </span>
        )}
        <div className="ml-auto flex gap-2">
          <Button variant="outline" onClick={() => setImporting(true)}>
            <UploadIcon data-icon="inline-start" />
            Import
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger
              render={
                <Button variant="outline" aria-label="Export the library">
                  <DownloadIcon data-icon="inline-start" />
                  <span className="hidden sm:inline">Export</span>
                </Button>
              }
            />
            <DropdownMenuContent align="end">
              <DropdownMenuGroup>
                <DropdownMenuItem render={<a href={apiUrl("/api/v1/export/library.csv")} download />}>
                  <FileDownIcon />
                  List as a spreadsheet (CSV)
                </DropdownMenuItem>
                <DropdownMenuItem render={<a href={apiUrl("/api/v1/export/library-midi.zip")} download />}>
                  <FileDownIcon />
                  Back up every piece as MIDI
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {tracks === null ? (
        <div className="flex flex-col gap-2">
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton key={i} className="h-14 w-full" />
          ))}
        </div>
      ) : tracks.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <LibraryIcon />
            </EmptyMedia>
            <EmptyTitle>Your library is empty</EmptyTitle>
            <EmptyDescription>
              Press Record in the bar below and play: each piece is saved here when it ends.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : shown.length === 0 ? (
        <p className="py-10 text-center text-sm text-muted-foreground">Nothing matches “{query}”.</p>
      ) : (
        <>
          {/* Phone: a simple list. */}
          <ul className="flex flex-col divide-y overflow-hidden rounded-xl border bg-card md:hidden">
            {shown.map((tr) => (
              <li key={tr.id} className={cn("flex items-center gap-2 py-2 pr-1 pl-3", playingId === tr.id && "bg-accent/50")}>
                <button
                  type="button"
                  className="flex min-w-0 flex-1 flex-col text-left"
                  onClick={() => t.load(tr.id)}
                  aria-label={`Play ${tr.info.title}`}
                >
                  <span className="truncate font-display font-semibold">{tr.info.title}</span>
                  <span className="truncate text-xs text-muted-foreground">
                    {[tr.info.melody || tr.info.composer, formatClock(tr.duration_ms), tr.verses > 1 && `${tr.verses} verses`]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                </button>
                {playButton(tr)}
                {menu(tr)}
              </li>
            ))}
          </ul>

          {/* Tablet and desktop: a sortable table. */}
          <div className="hidden overflow-hidden rounded-xl border bg-card md:block">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-12" />
                  <SortHeader label="Title" k="title" sort={sort} onSort={onSort} />
                  <TableHead>Tune</TableHead>
                  <SortHeader label="Length" k="duration" sort={sort} onSort={onSort} className="w-24" />
                  <TableHead className="w-20">Verses</TableHead>
                  <SortHeader label="Added" k="added" sort={sort} onSort={onSort} className="w-32" />
                  <TableHead className="w-12" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {shown.map((tr) => (
                  <TableRow
                    key={tr.id}
                    data-state={playingId === tr.id ? "selected" : undefined}
                    onDoubleClick={() => t.load(tr.id)}
                  >
                    <TableCell>{playButton(tr)}</TableCell>
                    <TableCell className="max-w-0 w-full">
                      <div className="flex min-w-0 flex-col">
                        <span className="truncate font-display font-semibold">{tr.info.title}</span>
                        <span className="truncate text-xs text-muted-foreground">
                          {tr.info.composer || tr.organ_name}
                        </span>
                      </div>
                    </TableCell>
                    <TableCell className="text-muted-foreground">{tr.info.melody}</TableCell>
                    <TableCell className="tabular-nums">{formatClock(tr.duration_ms)}</TableCell>
                    <TableCell className="tabular-nums">{tr.verses > 1 ? tr.verses : ""}</TableCell>
                    <TableCell>
                      {tr.source === "recorded" ? (
                        <span className="text-sm text-muted-foreground">{formatDate(tr.created)}</span>
                      ) : (
                        <Badge variant="secondary">{SOURCE_LABEL[tr.source]}</Badge>
                      )}
                    </TableCell>
                    <TableCell>{menu(tr)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </>
      )}

      <ImportDialog open={importing} onOpenChange={setImporting} />
      <TrackDetailsDialog track={editing} open={!!editing} onOpenChange={(o) => !o && setEditing(null)} />

      <AlertDialog open={!!deleting} onOpenChange={(o) => !o && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete “{deleting?.info.title}”?</AlertDialogTitle>
            <AlertDialogDescription>
              It disappears from the library and playlists. A copy is kept in the server&apos;s history folder.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep it</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => deleting && remove(deleting)}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
