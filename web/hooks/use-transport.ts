"use client"

import * as React from "react"
import { toast } from "sonner"

import { useEngine, useTicker } from "@/components/engine-provider"
import { api, call, type PlayOptions } from "@/lib/api/client"

function run(request: Promise<unknown>) {
  request.catch((e: Error) => toast.error(e.message))
}

/** Player and recorder state with a smoothly moving position, plus actions. */
export function useTransport() {
  const { player, playerAt, recorder, connected } = useEngine()
  const playing = player?.state === "playing"
  const now = useTicker(playing, 100)

  // Between status updates, advance the position ourselves.
  const drift =
    playing && now > playerAt
      ? ((now - playerAt) * (player?.tempo ?? 100)) / 100
      : 0
  const position = Math.min(
    (player?.position_ms ?? 0) + drift,
    player?.duration_ms ?? 0
  )
  const elapsed = Math.min(
    (player?.elapsed_ms ?? 0) + drift,
    player?.total_ms ?? 0
  )

  const actions = React.useMemo(
    () => ({
      load: (trackId: string, options: PlayOptions = {}) =>
        run(
          call(
            api.POST("/api/v1/player/load", {
              body: { track_id: trackId, options, play: true },
            })
          )
        ),
      play: () => run(call(api.POST("/api/v1/player/play"))),
      pause: () => run(call(api.POST("/api/v1/player/pause"))),
      stop: () => run(call(api.POST("/api/v1/player/stop"))),
      seek: (ms: number) =>
        run(
          call(
            api.POST("/api/v1/player/seek", { body: { ms: Math.round(ms) } })
          )
        ),
      tempo: (percent: number) =>
        run(call(api.POST("/api/v1/player/tempo", { body: { percent } }))),
      stopAfterVerse: (on: boolean) =>
        run(
          call(
            api.POST("/api/v1/player/verses", {
              body: { stop_after_verse: on },
            })
          )
        ),
      skipToLast: (on: boolean) =>
        run(
          call(
            api.POST("/api/v1/player/verses", { body: { skip_to_last: on } })
          )
        ),
      fade: (on: boolean) =>
        run(call(api.POST("/api/v1/player/fade", { body: { on } }))),
      record: () => run(call(api.POST("/api/v1/recorder/start"))),
      stopRecording: () => run(call(api.POST("/api/v1/recorder/stop"))),
    }),
    []
  )

  return { player, recorder, connected, playing, position, elapsed, ...actions }
}
