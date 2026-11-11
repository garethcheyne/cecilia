"use client"

import * as React from "react"
import Link from "next/link"
import { ListMusicIcon, PauseIcon, PlayIcon, SkipBackIcon, SkipForwardIcon, SquareIcon } from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Kbd } from "@/components/ui/kbd"
import { useTransport } from "@/hooks/use-transport"
import { api, call, type Playlist, type TrackSummary } from "@/lib/api/client"
import { formatClock } from "@/lib/time"
import { cn } from "@/lib/utils"
import { formatVerses } from "@/lib/verses"

function act(request: Promise<unknown>) {
  request.catch((e: Error) => toast.error(e.message))
}

/**
 * The screen for whoever runs the music during a service: large text, the
 * current piece highlighted, and big buttons. Space plays or pauses; the
 * right arrow skips to the next piece.
 */
export function ServiceView() {
  const { service, connected, libraryVersion } = useEngine()
  const t = useTransport()
  const [playlist, setPlaylist] = React.useState<Playlist | null>(null)
  const [library, setLibrary] = React.useState<Map<string, TrackSummary>>(new Map())
  const currentRef = React.useRef<HTMLLIElement>(null)

  const playlistId = service?.playlist_id
  React.useEffect(() => {
    if (!playlistId) return
    call(api.GET("/api/v1/playlists/{id}", { params: { path: { id: playlistId } } }))
      .then(setPlaylist)
      .catch(() => setPlaylist(null))
  }, [playlistId])

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/library"))
      .then((l) => setLibrary(new Map(l.map((x) => [x.id, x]))))
      .catch(() => {})
  }, [connected, libraryVersion])

  React.useEffect(() => {
    currentRef.current?.scrollIntoView({ block: "center", behavior: "smooth" })
  }, [service?.row])

  const state = t.player?.state
  const playing = state === "playing" || state === "starting"
  const next = () => act(call(api.POST("/api/v1/service/next")))
  const previous = () => act(call(api.POST("/api/v1/service/previous")))
  const select = (row: number) => act(call(api.POST("/api/v1/service/select", { body: { row } })))
  const end = () => act(call(api.POST("/api/v1/service/end")))
  const playPause = React.useCallback(() => (state === "playing" ? t.pause() : t.play()), [state, t])

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement
      if (target.closest("input, textarea, [contenteditable], [role=slider]")) return
      if (e.code === "Space") {
        e.preventDefault()
        playPause()
      } else if (e.key === "ArrowRight" || e.key.toLowerCase() === "n") {
        e.preventDefault()
        act(call(api.POST("/api/v1/service/next")))
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [playPause])

  if (!service || service.state === "idle" || !playlist) {
    return (
      <div className="p-4">
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ListMusicIcon />
            </EmptyMedia>
            <EmptyTitle>No service running</EmptyTitle>
            <EmptyDescription>Open a playlist and press Run service.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button render={<Link href="/playlists" />} nativeButton={false}>
              Choose a playlist
            </Button>
          </EmptyContent>
        </Empty>
      </div>
    )
  }

  const status =
    service.state === "waiting"
      ? `Starts in ${service.countdown ?? 0} s`
      : service.state === "finished"
        ? "Finished"
        : service.state === "playing"
          ? state === "paused"
            ? "Paused"
            : "Playing"
          : "Ready: press Play"

  return (
    <div className="mx-auto flex w-full min-w-0 max-w-4xl flex-1 flex-col gap-4 p-3 sm:p-6">
      <div className="flex flex-wrap items-center gap-3">
        <h2 className="font-display text-2xl font-semibold sm:text-3xl">{playlist.name}</h2>
        <Badge variant={service.state === "waiting" ? "default" : "secondary"} className="text-sm" aria-live="polite">
          {status}
        </Badge>
        <Button variant="ghost" size="sm" className="ml-auto" onClick={end}>
          End service
        </Button>
      </div>

      <ol className="flex flex-col gap-2">
        {(playlist.rows ?? []).map((row, i) => {
          const current = service.row === i
          const isNext = service.next_row === i
          if (row.type === "text") {
            return (
              <li key={i} className="px-4 py-2 font-display text-lg text-muted-foreground italic sm:text-xl">
                {row.text}
              </li>
            )
          }
          const tr = library.get(row.track_id)
          return (
            <li key={i} ref={current ? currentRef : undefined}>
              <button
                type="button"
                onClick={() => select(i)}
                className={cn(
                  "flex w-full items-center gap-4 rounded-2xl border bg-card px-4 py-4 text-left transition-colors outline-none hover:bg-accent/40 focus-visible:ring-3 focus-visible:ring-ring/50 sm:px-5",
                  current && "border-primary bg-primary/5 ring-2 ring-primary/30"
                )}
                aria-current={current ? "step" : undefined}
              >
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <span className={cn("font-display text-xl font-semibold sm:text-2xl", !current && "text-foreground/80")}>
                    {tr?.info.title ?? "Missing track"}
                  </span>
                  <span className="text-sm text-muted-foreground sm:text-base">
                    {[
                      row.note,
                      row.verses != null ? `Verses ${formatVerses(row.verses)}` : tr && tr.verses > 1 && `${tr.verses} verses`,
                      tr && formatClock(tr.duration_ms),
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                </div>
                {current && <Badge className="shrink-0">Now</Badge>}
                {isNext && !current && (
                  <Badge variant="outline" className="shrink-0">
                    Next
                  </Badge>
                )}
              </button>
            </li>
          )
        })}
      </ol>

      <div className="sticky bottom-24 mt-auto flex items-center justify-center gap-3 pt-4 md:bottom-20">
        <Button variant="outline" size="icon-lg" className="size-14 rounded-full" onClick={previous} aria-label="Previous piece">
          <SkipBackIcon />
        </Button>
        <Button
          variant="outline"
          size="icon-lg"
          className="size-14 rounded-full"
          onClick={t.stop}
          disabled={!t.player?.track_id || state === "stopped"}
          aria-label="Stop"
        >
          <SquareIcon />
        </Button>
        <Button size="icon-lg" className="size-20 rounded-full shadow-lg [&_svg]:size-8" onClick={playPause} aria-label={playing ? "Pause" : "Play"}>
          {playing ? <PauseIcon /> : <PlayIcon />}
        </Button>
        <Button variant="outline" size="icon-lg" className="size-14 rounded-full" onClick={next} aria-label="Next piece">
          <SkipForwardIcon />
        </Button>
      </div>
      <p className="hidden justify-center gap-4 text-sm text-muted-foreground md:flex">
        <span>
          <Kbd>Space</Kbd> play or pause
        </span>
        <span>
          <Kbd>→</Kbd> next piece
        </span>
      </p>
    </div>
  )
}
