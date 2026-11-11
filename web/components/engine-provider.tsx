"use client"

import * as React from "react"

import { toast } from "sonner"

import {
  eventsUrl,
  type EngineEvent,
  type MidiEvent,
  type MidiStatus,
  type PlayerStatus,
  type RecorderStatus,
  type SimSnapshot,
  type TrackSummary,
} from "@/lib/api/client"

const MAX_EVENTS = 500

export type MonitorEvent = MidiEvent & { id: number }

interface EngineContextValue {
  /** WebSocket to organ-server is open. */
  connected: boolean
  status: MidiStatus | null
  /** Simulator organ and console state. */
  simulator: SimSnapshot | null
  player: PlayerStatus | null
  /** performance.now() when `player` arrived, for a smooth position. */
  playerAt: number
  recorder: RecorderStatus | null
  /** Bumped whenever the library changes; refetch when it does. */
  libraryVersion: number
  /** performance.now() when `status` arrived, for interpolating timers. */
  statusAt: number
}

interface MonitorContextValue {
  /** Most recent MIDI traffic, newest first. */
  events: MonitorEvent[]
  /** Messages dropped because this client fell behind. */
  skipped: number
  paused: boolean
  setPaused: (paused: boolean) => void
  clearEvents: () => void
}

const EngineContext = React.createContext<EngineContextValue | null>(null)
// Separate, because MIDI traffic changes many times a second and only the
// monitor needs it.
const MonitorContext = React.createContext<MonitorContextValue | null>(null)

/**
 * Keeps one WebSocket to organ-server open for the whole app (reconnecting
 * with backoff) and exposes live status and MIDI traffic.
 */
export function EngineProvider({ children }: { children: React.ReactNode }) {
  const [connected, setConnected] = React.useState(false)
  const [status, setStatus] = React.useState<MidiStatus | null>(null)
  const [simulator, setSimulator] = React.useState<SimSnapshot | null>(null)
  const [player, setPlayer] = React.useState<PlayerStatus | null>(null)
  const [playerAt, setPlayerAt] = React.useState(0)
  const [recorder, setRecorder] = React.useState<RecorderStatus | null>(null)
  const [libraryVersion, setLibraryVersion] = React.useState(0)
  const [statusAt, setStatusAt] = React.useState(0)
  const [events, setEvents] = React.useState<MonitorEvent[]>([])
  const [skipped, setSkipped] = React.useState(0)
  const [paused, setPaused] = React.useState(false)

  const pausedRef = React.useRef(paused)
  React.useEffect(() => {
    pausedRef.current = paused
  }, [paused])

  React.useEffect(() => {
    let socket: WebSocket | null = null
    let retry: ReturnType<typeof setTimeout> | undefined
    let frame = 0
    let attempt = 0
    let nextId = 0
    let stopped = false
    // Batch MIDI messages and flush once per animation frame, so a dense
    // passage doesn't re-render once per note.
    let pending: MonitorEvent[] = []

    const flush = () => {
      frame = 0
      if (pending.length === 0) return
      const batch = pending.reverse()
      pending = []
      setEvents((prev) => batch.concat(prev).slice(0, MAX_EVENTS))
    }

    const connect = () => {
      socket = new WebSocket(eventsUrl())
      socket.onopen = () => {
        attempt = 0
        setConnected(true)
      }
      socket.onmessage = (message) => {
        const event = JSON.parse(message.data) as
          | EngineEvent
          | (SimSnapshot & { type: "simulator" })
          | (PlayerStatus & { type: "player" })
          | (RecorderStatus & { type: "recorder" })
          | { type: "library"; saved?: TrackSummary | null }
          | { type: "player_finished"; track_id: string }
          | { type: "lagged"; skipped: number }
        if (event.type === "status") {
          setStatus(event)
          setStatusAt(performance.now())
        } else if (event.type === "midi") {
          if (pausedRef.current) return
          pending.push({ ...event, id: nextId++ })
          if (!frame) frame = requestAnimationFrame(flush)
        } else if (event.type === "simulator") {
          setSimulator(event)
        } else if (event.type === "player") {
          setPlayer(event)
          setPlayerAt(performance.now())
        } else if (event.type === "recorder") {
          setRecorder(event)
        } else if (event.type === "library") {
          setLibraryVersion((v) => v + 1)
          if (event.saved) toast.success(`Saved “${event.saved.info.title}”`, { description: "In your library" })
        } else if (event.type === "lagged") {
          setSkipped((n) => n + event.skipped)
        }
      }
      socket.onclose = () => {
        setConnected(false)
        if (stopped) return
        const delay = Math.min(1000 * 2 ** attempt, 5000)
        attempt += 1
        retry = setTimeout(connect, delay)
      }
    }

    connect()
    return () => {
      stopped = true
      clearTimeout(retry)
      cancelAnimationFrame(frame)
      socket?.close()
    }
  }, [])

  const clearEvents = React.useCallback(() => {
    setEvents([])
    setSkipped(0)
  }, [])

  const value = React.useMemo(
    () => ({ connected, status, simulator, player, playerAt, recorder, libraryVersion, statusAt }),
    [connected, status, simulator, player, playerAt, recorder, libraryVersion, statusAt]
  )
  const monitor = React.useMemo(
    () => ({ events, skipped, paused, setPaused, clearEvents }),
    [events, skipped, paused, clearEvents]
  )

  return (
    <EngineContext.Provider value={value}>
      <MonitorContext.Provider value={monitor}>{children}</MonitorContext.Provider>
    </EngineContext.Provider>
  )
}

export function useMonitor(): MonitorContextValue {
  const ctx = React.useContext(MonitorContext)
  if (!ctx) throw new Error("useMonitor must be used inside <EngineProvider>")
  return ctx
}

export function useEngine(): EngineContextValue {
  const ctx = React.useContext(EngineContext)
  if (!ctx) throw new Error("useEngine must be used inside <EngineProvider>")
  return ctx
}

/** Re-renders every `intervalMs` while `active`, returning performance.now(). */
export function useTicker(active: boolean, intervalMs = 100): number {
  const [now, setNow] = React.useState(0)
  React.useEffect(() => {
    if (!active) return
    const id = setInterval(() => setNow(performance.now()), intervalMs)
    return () => clearInterval(id)
  }, [active, intervalMs])
  return now
}
