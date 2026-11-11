"use client"

import * as React from "react"

import { useEngine } from "@/components/engine-provider"
import { api, call, type Playlist, type TrackSummary } from "@/lib/api/client"

export type ServiceData = ReturnType<typeof useServiceData>

/** The running service, its playlist and the library it draws on. */
export function useServiceData() {
  const { service, connected, libraryVersion } = useEngine()
  const [playlist, setPlaylist] = React.useState<Playlist | null>(null)
  const [library, setLibrary] = React.useState<Map<string, TrackSummary>>(new Map())

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

  const running = !!service && service.state !== "idle" && !!playlist && playlist.id === playlistId
  const rows = running ? (playlist?.rows ?? []) : []
  const trackAt = (i: number | null | undefined) => {
    const row = i == null ? undefined : rows[i]
    return row && row.type !== "text" ? row.track_id : null
  }

  return {
    service,
    playlist: running ? playlist : null,
    library,
    /** Track of the row that's ready or playing. */
    currentTrackId: trackAt(service?.row),
    nextTrackId: trackAt(service?.next_row),
  }
}
