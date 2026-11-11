"use client"

import * as React from "react"
import Link from "next/link"
import { ListMusicIcon, MusicIcon, PauseIcon, PlayIcon, SkipBackIcon, SkipForwardIcon, SquareIcon } from "lucide-react"
import { toast } from "sonner"

import { LiveConsole } from "@/components/console/live-console"
import { ScoreView } from "@/components/editor/score-view"
import type { ServiceData } from "@/components/service/use-service-data"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Kbd } from "@/components/ui/kbd"
import { Progress } from "@/components/ui/progress"
import { Spinner } from "@/components/ui/spinner"
import { useTransport } from "@/hooks/use-transport"
import { api, call, type Track } from "@/lib/api/client"
import type { PanelId } from "@/lib/displays"
import { formatClock } from "@/lib/time"
import { cn } from "@/lib/utils"
import { formatVerses } from "@/lib/verses"

function act(request: Promise<unknown>) {
  request.catch((e: Error) => toast.error(e.message))
}

function NoService() {
  return (
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
  )
}

/**
 * The pieces in order, large and easy to tap, with big buttons. One tap gets
 * a piece ready; a second tap (or a double-click) plays it now.
 */
function OrderPanel({ data, inColumn }: { data: ServiceData; inColumn: boolean }) {
  const { service, playlist, library } = data
  const t = useTransport()
  const currentRef = React.useRef<HTMLLIElement>(null)

  React.useEffect(() => {
    currentRef.current?.scrollIntoView({ block: "center", behavior: "smooth" })
  }, [service?.row])

  const lastTap = React.useRef<{ row: number; at: number } | null>(null)
  const tapRow = (row: number, now: number) => {
    const again = lastTap.current?.row === row && now - lastTap.current.at < 450
    lastTap.current = again ? null : { row, at: now }
    if (again) {
      act(call(api.POST("/api/v1/service/select", { body: { row } })).then(() => call(api.POST("/api/v1/player/play"))))
    } else {
      act(call(api.POST("/api/v1/service/select", { body: { row } })))
    }
  }

  if (!service || !playlist) return <NoService />

  const state = t.player?.state
  const playing = state === "playing" || state === "starting"
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
    <div className="flex min-h-full flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <h2 className="font-display text-2xl font-semibold sm:text-3xl">{playlist.name}</h2>
        <Badge variant={service.state === "waiting" ? "default" : "secondary"} className="text-sm" aria-live="polite">
          {status}
        </Badge>
        <Button variant="ghost" size="sm" className="ml-auto" onClick={() => act(call(api.POST("/api/v1/service/end")))}>
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
                onClick={(e) => tapRow(i, e.timeStamp)}
                title="Tap to get it ready; double-tap to play it now"
                className={cn(
                  "flex w-full touch-manipulation items-center gap-4 rounded-2xl border bg-card px-4 py-4 text-left transition-colors outline-none hover:bg-accent/40 focus-visible:ring-3 focus-visible:ring-ring/50 sm:px-5",
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

      <div
        className={cn(
          "sticky mt-auto flex items-center justify-center gap-3 pt-4",
          inColumn ? "bottom-0 bg-linear-to-t from-background from-60% to-transparent pb-3" : "bottom-24 md:bottom-20"
        )}
      >
        <Button
          variant="outline"
          size="icon-lg"
          className="size-14 rounded-full"
          onClick={() => act(call(api.POST("/api/v1/service/previous")))}
          aria-label="Previous piece"
        >
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
        <Button
          size="icon-lg"
          className="size-20 rounded-full shadow-lg [&_svg]:size-8"
          onClick={() => (state === "playing" ? t.pause() : t.play())}
          aria-label={playing ? "Pause" : "Play"}
        >
          {playing ? <PauseIcon /> : <PlayIcon />}
        </Button>
        <Button
          variant="outline"
          size="icon-lg"
          className="size-14 rounded-full"
          onClick={() => act(call(api.POST("/api/v1/service/next")))}
          aria-label="Next piece"
        >
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
        <span>Double-click a piece to play it now</span>
      </p>
      <p className="text-center text-sm text-muted-foreground md:hidden">Double-tap a piece to play it now</p>
    </div>
  )
}

/** What's playing, large enough to read across the church. */
function NowPanel({ data }: { data: ServiceData }) {
  const t = useTransport()
  const { library, currentTrackId, nextTrackId, service } = data
  const p = t.player
  const active = !!p?.track_id && p.state !== "stopped"
  const title = active ? p?.title : currentTrackId ? library.get(currentTrackId)?.info.title : null
  const next = nextTrackId && nextTrackId !== currentTrackId ? library.get(nextTrackId)?.info.title : null
  const left = active && p ? Math.max(0, p.total_ms - t.elapsed) : 0
  const progress = active && p && p.total_ms > 0 ? (t.elapsed / p.total_ms) * 100 : 0
  const state = !active
    ? service?.state === "waiting"
      ? `Starts in ${service.countdown ?? 0} s`
      : title
        ? "Ready"
        : "Nothing playing"
    : p?.state === "paused"
      ? "Paused"
      : "Playing"

  return (
    <div className="flex min-h-[60svh] flex-1 flex-col items-center justify-center gap-6 px-4 py-10 text-center">
      <Badge variant={active && p?.state !== "paused" ? "default" : "secondary"} className="text-base">
        {state}
      </Badge>
      <p className="font-display text-4xl leading-tight font-semibold text-balance sm:text-6xl">{title ?? "—"}</p>
      {active && p && (
        <>
          {p.verses > 1 && (
            <p className="text-2xl text-muted-foreground sm:text-3xl">
              Verse {p.verse} of {p.verses}
            </p>
          )}
          <Progress value={progress} className="w-full max-w-xl" aria-label="Progress through the piece" />
          <p className="font-mono text-3xl tabular-nums sm:text-4xl">{formatClock(left)} left</p>
        </>
      )}
      {next && <p className="text-lg text-muted-foreground sm:text-xl">Next: {next}</p>}
    </div>
  )
}

/** The score of the piece that's playing (or ready), following along. */
function MusicPanel({ data }: { data: ServiceData }) {
  const t = useTransport()
  const playingId = t.player && t.player.state !== "stopped" ? t.player.track_id : null
  const id = playingId ?? data.currentTrackId
  const [track, setTrack] = React.useState<Track | null>(null)

  React.useEffect(() => {
    if (!id || track?.id === id) return
    call(api.GET("/api/v1/library/{id}", { params: { path: { id } } }))
      .then(setTrack)
      .catch(() => setTrack(null))
  }, [id, track?.id])

  if (!id) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <MusicIcon />
          </EmptyMedia>
          <EmptyTitle>No music yet</EmptyTitle>
          <EmptyDescription>The score appears here when a piece is ready or playing.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }
  if (!track || track.id !== id) {
    return (
      <div className="flex items-center justify-center gap-2 p-10 text-sm text-muted-foreground">
        <Spinner /> Loading the music…
      </div>
    )
  }
  return (
    <div className="flex min-w-0 flex-col gap-3">
      <h2 className="font-display text-2xl font-semibold sm:text-3xl">{track.info.title}</h2>
      <ScoreView track={track} position={playingId === id ? t.position : null} />
    </div>
  )
}

export function ServicePanel({ id, data, inColumn }: { id: PanelId; data: ServiceData; inColumn: boolean }) {
  switch (id) {
    case "order":
      return <OrderPanel data={data} inColumn={inColumn} />
    case "now":
      return <NowPanel data={data} />
    case "music":
      return <MusicPanel data={data} />
    case "console":
      return <LiveConsole compact />
  }
}
