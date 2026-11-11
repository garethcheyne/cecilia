"use client"

import * as React from "react"

import {
  CHANNEL,
  type ChannelMessage,
  displayUrl,
  type PanelId,
} from "@/lib/displays"

/** Window Management API (Chromium), not yet in the DOM typings. */
type ScreenDetailed = {
  availLeft: number
  availTop: number
  availWidth: number
  availHeight: number
  isPrimary: boolean
  label?: string
}
type ScreenDetails = {
  screens: ScreenDetailed[]
  currentScreen: ScreenDetailed
}
type ExtendedScreen = Screen & { isExtended?: boolean } & EventTarget
type WindowWithScreens = Window & {
  getScreenDetails?: () => Promise<ScreenDetails>
}

/** "many": more than one screen; "one": just this one; "unknown": the browser can't say. */
export type ScreenCount = "many" | "one" | "unknown"

function subscribeScreens(onChange: () => void) {
  const s = window.screen as ExtendedScreen
  s.addEventListener?.("change", onChange)
  window.addEventListener("resize", onChange)
  return () => {
    s.removeEventListener?.("change", onChange)
    window.removeEventListener("resize", onChange)
  }
}

function screenCount(): ScreenCount {
  const extended = (window.screen as ExtendedScreen).isExtended
  return extended === undefined ? "unknown" : extended ? "many" : "one"
}

/** Whether this computer has more than one screen, kept up to date. */
export function useScreenCount(): ScreenCount {
  return React.useSyncExternalStore(
    subscribeScreens,
    screenCount,
    () => "unknown"
  )
}

function subscribeFullscreen(onChange: () => void) {
  document.addEventListener("fullscreenchange", onChange)
  return () => document.removeEventListener("fullscreenchange", onChange)
}

/** Whether this window is full screen, and a toggle for it. */
export function useFullscreen() {
  const on = React.useSyncExternalStore(
    subscribeFullscreen,
    () => !!document.fullscreenElement,
    () => false
  )
  const toggle = React.useCallback(() => {
    const done = on
      ? document.exitFullscreen()
      : document.documentElement.requestFullscreen()
    done.catch(() => {})
  }, [on])
  const supported =
    typeof document !== "undefined" && !!document.fullscreenEnabled
  return { on, toggle, supported }
}

function subscribeWide(onChange: () => void) {
  const mq = window.matchMedia("(min-width: 1024px)")
  mq.addEventListener("change", onChange)
  return () => mq.removeEventListener("change", onChange)
}

/** Wide enough to show two panels side by side. */
export function useWide(): boolean {
  return React.useSyncExternalStore(
    subscribeWide,
    () => window.matchMedia("(min-width: 1024px)").matches,
    () => true
  )
}

export type OpenResult = "other-screen" | "window" | "blocked"

/**
 * Open a display window. With permission to manage windows it opens on the
 * other screen, filling it; otherwise it's an ordinary window to drag there.
 * Call it straight from a click.
 */
export async function openDisplay(panels: PanelId[]): Promise<OpenResult> {
  const w = window as WindowWithScreens
  let features = "popup=yes,width=1280,height=800"
  let placed = false
  if (w.getScreenDetails && screenCount() === "many") {
    try {
      const details = await w.getScreenDetails()
      const other =
        details.screens.find(
          (s) => s !== details.currentScreen && !s.isPrimary
        ) ?? details.screens.find((s) => s !== details.currentScreen)
      if (other) {
        features = `popup=yes,left=${other.availLeft},top=${other.availTop},width=${other.availWidth},height=${other.availHeight}`
        placed = true
      }
    } catch {
      // Permission refused: fall back to an ordinary window.
    }
  }
  const opened = window.open(displayUrl(panels), "cecilia-display", features)
  if (!opened) return "blocked"
  return placed ? "other-screen" : "window"
}

/** Talk to the Service page and its display windows on this device. */
export function useServiceChannel(
  onMessage: (m: ChannelMessage, reply: (m: ChannelMessage) => void) => void
) {
  const handler = React.useRef(onMessage)
  React.useEffect(() => {
    handler.current = onMessage
  })
  const channel = React.useRef<BroadcastChannel | null>(null)
  React.useEffect(() => {
    if (typeof BroadcastChannel === "undefined") return
    const c = new BroadcastChannel(CHANNEL)
    c.onmessage = (e: MessageEvent<ChannelMessage>) =>
      handler.current(e.data, (m) => c.postMessage(m))
    channel.current = c
    return () => {
      c.close()
      channel.current = null
    }
  }, [])
  return React.useCallback(
    (m: ChannelMessage) => channel.current?.postMessage(m),
    []
  )
}
