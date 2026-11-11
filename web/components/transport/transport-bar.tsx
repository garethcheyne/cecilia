"use client"

import * as React from "react"
import Link from "next/link"
import {
  ChevronUpIcon,
  GaugeIcon,
  LibraryIcon,
  PauseIcon,
  PlayIcon,
  SkipForwardIcon,
  SquareIcon,
  TrendingDownIcon,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { Slider } from "@/components/ui/slider"
import { Toggle } from "@/components/ui/toggle"
import { NowPlayingScore } from "@/components/transport/now-playing-score"
import { useTransport } from "@/hooks/use-transport"
import { api, call } from "@/lib/api/client"
import { toast } from "sonner"
import { formatClock } from "@/lib/time"
import { cn } from "@/lib/utils"

type Transport = ReturnType<typeof useTransport>

/** The record button: one press records the whole service. */
function RecordButton({
  t,
  compact = false,
  large = false,
}: {
  t: Transport
  compact?: boolean
  large?: boolean
}) {
  const state = t.recorder?.state ?? "off"
  const armed = state !== "off"
  const label =
    state === "recording"
      ? `Recording ${formatClock(t.recorder?.elapsed_ms ?? 0)}`
      : state === "waiting"
        ? "Waiting for the first note"
        : "Record"
  return (
    <Button
      variant={armed ? "destructive" : "outline"}
      size={large ? "icon-lg" : compact ? "icon" : "default"}
      onClick={armed ? t.stopRecording : t.record}
      disabled={!t.connected}
      aria-pressed={armed}
      aria-label={armed ? `Stop recording (${label})` : "Record"}
      title={armed ? "Stop recording" : "Record every piece played from now on"}
      className={cn(!compact && "min-w-0", large && "rounded-full")}
    >
      <span
        aria-hidden
        className={cn(
          "size-2.5 shrink-0 rounded-full",
          armed ? "bg-current" : "bg-destructive",
          state === "waiting" && "animate-pulse"
        )}
      />
      {!compact && <span className="truncate">{label}</span>}
    </Button>
  )
}

/** Position slider that only seeks when you let go. */
function SeekBar({ t, className }: { t: Transport; className?: string }) {
  const [scrub, setScrub] = React.useState<number | null>(null)
  const p = t.player
  const loaded = !!p?.track_id
  const value = scrub ?? t.position
  return (
    <div
      className={cn(
        "flex min-w-0 items-center gap-3 text-xs text-muted-foreground tabular-nums",
        className
      )}
    >
      <span className="w-12 text-right">{formatClock(value)}</span>
      <Slider
        aria-label="Position"
        min={0}
        max={Math.max(p?.duration_ms ?? 1, 1)}
        step={100}
        value={value}
        disabled={!loaded}
        onValueChange={(v) => setScrub(v as number)}
        onValueCommitted={(v) => {
          setScrub(null)
          t.seek(v as number)
        }}
      />
      <span className="w-12">{formatClock(p?.duration_ms ?? 0)}</span>
    </div>
  )
}

function TempoControl({ t }: { t: Transport }) {
  const tempo = t.player?.tempo ?? 100
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            variant="ghost"
            size="sm"
            aria-label={`Tempo ${tempo}%`}
            disabled={!t.player?.track_id}
          >
            <GaugeIcon data-icon="inline-start" />
            <span className="tabular-nums">{tempo}%</span>
          </Button>
        }
      />
      <PopoverContent className="w-72">
        <TempoSlider t={t} />
      </PopoverContent>
    </Popover>
  )
}

function TempoSlider({ t }: { t: Transport }) {
  const tempo = t.player?.tempo ?? 100
  const [draft, setDraft] = React.useState<number | null>(null)
  const id = t.player?.track_id
  // The tempo saved with the piece, so "Keep" only shows when it would change it.
  const [saved, setSaved] = React.useState<{
    id: string
    tempo: number
  } | null>(null)
  React.useEffect(() => {
    if (!id) return
    let cancelled = false
    call(api.GET("/api/v1/library/{id}", { params: { path: { id } } }))
      .then((tr) => !cancelled && setSaved({ id, tempo: tr.info.tempo ?? 100 }))
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [id])
  const savedTempo = saved && saved.id === id ? saved.tempo : null
  const canKeep = !!id && savedTempo !== null && savedTempo !== tempo

  async function keep() {
    if (!id) return
    try {
      await call(
        api.PATCH("/api/v1/library/{id}", {
          params: { path: { id } },
          body: { tempo },
        })
      )
      setSaved({ id, tempo })
      toast.success(
        tempo === 100
          ? "Back to the recorded tempo"
          : `This piece will play at ${tempo}% from now on`
      )
    } catch (e) {
      toast.error((e as Error).message)
    }
  }
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between text-sm">
        <span className="font-medium">Tempo</span>
        <span className="text-muted-foreground tabular-nums">
          {draft ?? tempo}%
        </span>
      </div>
      <Slider
        aria-label="Tempo"
        min={50}
        max={150}
        step={1}
        value={draft ?? tempo}
        onValueChange={(v) => setDraft(v as number)}
        onValueCommitted={(v) => {
          setDraft(null)
          t.tempo(v as number)
        }}
      />
      <div className="flex min-h-6 items-center justify-between">
        <span className="text-xs text-muted-foreground">Slower · Faster</span>
        {tempo !== 100 && (
          <Button variant="outline" size="xs" onClick={() => t.tempo(100)}>
            Reset to 100%
          </Button>
        )}
      </div>
      {canKeep && (
        <Button
          variant="secondary"
          size="sm"
          onClick={keep}
          title="Save this tempo with the piece"
        >
          Keep {tempo}% for this piece
        </Button>
      )}
    </div>
  )
}

function VerseControls({
  t,
  stacked = false,
}: {
  t: Transport
  stacked?: boolean
}) {
  const p = t.player
  if (!p?.track_id || p.verses < 2) return null
  const active =
    p.state === "playing" || p.state === "paused" || p.state === "starting"
  return (
    <div
      className={cn(
        "flex items-center gap-1",
        stacked && "flex-col items-stretch gap-2"
      )}
    >
      <Toggle
        variant="outline"
        size="sm"
        pressed={p.stop_after_verse}
        onPressedChange={t.stopAfterVerse}
        disabled={!active}
        title="Stop when this verse ends"
      >
        <SquareIcon />
        End of verse
      </Toggle>
      <Toggle
        variant="outline"
        size="sm"
        pressed={p.skip_to_last}
        onPressedChange={t.skipToLast}
        disabled={!active || p.verse >= p.verses}
        title="When this verse ends, go to the last verse"
      >
        <SkipForwardIcon />
        Last verse
      </Toggle>
    </div>
  )
}

function FadeButton({ t }: { t: Transport }) {
  const p = t.player
  const active = p?.state === "playing" || p?.state === "paused"
  return (
    <Toggle
      variant="outline"
      size="sm"
      pressed={!!p?.fading}
      onPressedChange={t.fade}
      disabled={!active}
      title={p?.fading ? "Cancel the fade" : "Fade out and stop"}
    >
      <TrendingDownIcon />
      Fade
    </Toggle>
  )
}

function PlayPause({
  t,
  size = "icon-lg",
}: {
  t: Transport
  size?: "icon" | "icon-lg"
}) {
  const state = t.player?.state
  const loaded = !!t.player?.track_id
  const busy = state === "playing" || state === "starting"
  return (
    <Button
      size={size}
      className="rounded-full"
      onClick={state === "playing" ? t.pause : t.play}
      disabled={!loaded || state === "starting"}
      aria-label={busy ? "Pause" : "Play"}
    >
      {busy ? <PauseIcon /> : <PlayIcon />}
    </Button>
  )
}

function NowPlaying({ t }: { t: Transport }) {
  const p = t.player
  if (!p?.track_id) {
    return (
      <div className="flex min-w-0 flex-col">
        <span className="truncate text-sm font-medium">Nothing playing</span>
        <Link
          href="/library"
          className="truncate text-xs text-muted-foreground hover:text-foreground"
        >
          Choose music from the Library
        </Link>
      </div>
    )
  }
  const state =
    p.state === "starting"
      ? "Setting stops…"
      : p.state === "paused"
        ? "Paused"
        : p.state === "stopped"
          ? "Ready"
          : null
  return (
    <div className="flex min-w-0 flex-col">
      <span className="truncate font-display text-sm font-semibold">
        {p.title}
      </span>
      <span className="truncate text-xs text-muted-foreground tabular-nums">
        {[
          state,
          p.verses > 1 && `Verse ${p.verse} of ${p.verses}`,
          `${formatClock(t.elapsed)} / ${formatClock(p.total_ms)}`,
        ]
          .filter(Boolean)
          .join(" · ")}
      </span>
    </div>
  )
}

export function TransportBar() {
  const t = useTransport()
  const p = t.player
  const progress = p && p.total_ms > 0 ? (t.elapsed / p.total_ms) * 100 : 0

  return (
    <div
      role="region"
      aria-label="Transport"
      className="sticky bottom-0 z-20 border-t bg-background/90 backdrop-blur-md supports-[backdrop-filter]:bg-background/75"
    >
      {/* Phone: a thin progress line and one row of essentials. */}
      <div className="md:hidden">
        <div className="h-0.5 bg-muted">
          <div
            className="h-full bg-primary transition-[width] duration-100"
            style={{ width: `${progress}%` }}
          />
        </div>
        <div className="flex items-center gap-2 px-3 py-2 pb-[max(0.5rem,env(safe-area-inset-bottom))]">
          <RecordButton t={t} compact />
          <Drawer>
            <DrawerTrigger
              render={
                <button
                  type="button"
                  className="flex min-w-0 flex-1 items-center gap-2 rounded-lg px-1 py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  aria-label="Show playback controls"
                >
                  <NowPlaying t={t} />
                  <ChevronUpIcon
                    className="ml-auto size-4 shrink-0 text-muted-foreground"
                    aria-hidden
                  />
                </button>
              }
            />
            <DrawerContent>
              <DrawerHeader>
                <DrawerTitle className="font-display">
                  {p?.title ?? "Nothing playing"}
                </DrawerTitle>
                <DrawerDescription>
                  {p?.track_id
                    ? p.verses > 1
                      ? `Verse ${p.verse} of ${p.verses}`
                      : "Single piece"
                    : "Choose music from the Library to play it here."}
                </DrawerDescription>
              </DrawerHeader>
              <div className="flex flex-col gap-6 px-4 pb-8">
                <SeekBar t={t} />
                <div className="flex items-center justify-center gap-4">
                  <Button
                    variant="outline"
                    size="icon-lg"
                    className="rounded-full"
                    onClick={t.stop}
                    disabled={!p?.track_id || p.state === "stopped"}
                    aria-label="Stop"
                  >
                    <SquareIcon />
                  </Button>
                  <PlayPause t={t} />
                  <RecordButton t={t} compact large />
                </div>
                <TempoSlider t={t} />
                <div className="flex flex-wrap justify-center gap-2">
                  <VerseControls t={t} />
                  <FadeButton t={t} />
                </div>
                {p?.track_id && <NowPlayingScore label />}
                {!p?.track_id && (
                  <Button
                    variant="outline"
                    render={<Link href="/library" />}
                    nativeButton={false}
                  >
                    <LibraryIcon data-icon="inline-start" />
                    Open the Library
                  </Button>
                )}
              </div>
            </DrawerContent>
          </Drawer>
          <PlayPause t={t} size="icon" />
        </div>
      </div>

      {/* Tablet and desktop: everything in one bar. */}
      <div className="hidden items-center gap-4 px-4 py-2.5 md:flex">
        <RecordButton t={t} />
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={t.stop}
            disabled={!p?.track_id || p.state === "stopped"}
            aria-label="Stop"
          >
            <SquareIcon />
          </Button>
          <PlayPause t={t} />
        </div>
        <div className="flex w-56 min-w-0 shrink-0 lg:w-64">
          <NowPlaying t={t} />
        </div>
        <SeekBar t={t} className="min-w-40 flex-1" />
        <div className="flex items-center gap-1">
          <NowPlayingScore />
          <VerseControls t={t} />
          <FadeButton t={t} />
          <TempoControl t={t} />
        </div>
      </div>
    </div>
  )
}
