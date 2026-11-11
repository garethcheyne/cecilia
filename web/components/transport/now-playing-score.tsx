"use client"

import * as React from "react"
import { MusicIcon } from "lucide-react"

import { ScoreView } from "@/components/editor/score-view"
import { Button } from "@/components/ui/button"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet"
import { Spinner } from "@/components/ui/spinner"
import { useTransport } from "@/hooks/use-transport"
import { api, call, type Track } from "@/lib/api/client"

/** The music of whatever is playing, scrolling along with it. */
export function NowPlayingScore({ label = false }: { label?: boolean }) {
  const t = useTransport()
  const id = t.player?.track_id
  const [open, setOpen] = React.useState(false)
  const [track, setTrack] = React.useState<Track | null>(null)

  React.useEffect(() => {
    if (!open || !id || track?.id === id) return
    call(api.GET("/api/v1/library/{id}", { params: { path: { id } } }))
      .then(setTrack)
      .catch(() => setTrack(null))
  }, [open, id, track?.id])

  const playing = t.player && t.player.state !== "stopped" ? t.position : null

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger
        render={
          <Button
            variant={label ? "outline" : "ghost"}
            size={label ? "default" : "sm"}
            disabled={!id}
            aria-label="Show the music"
          >
            <MusicIcon data-icon="inline-start" />
            {label ? "Show the music" : "Music"}
          </Button>
        }
      />
      <SheetContent
        side="bottom"
        className="gap-0 data-[side=bottom]:h-[92svh] sm:data-[side=bottom]:h-[85svh]"
      >
        <SheetHeader>
          <SheetTitle className="font-display">
            {t.player?.title ?? "Music"}
          </SheetTitle>
          <SheetDescription>Follows along as it plays.</SheetDescription>
        </SheetHeader>
        <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-6">
          {track && track.id === id ? (
            <ScoreView track={track} position={playing} />
          ) : (
            <div className="flex items-center justify-center gap-2 p-10 text-sm text-muted-foreground">
              <Spinner /> Loading the music…
            </div>
          )}
        </div>
      </SheetContent>
    </Sheet>
  )
}
