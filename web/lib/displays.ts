/**
 * How the Service page is spread over this device's screens: what this
 * screen shows (one panel, or two side by side) and what a second screen
 * shows. Kept per device, since each church's setup differs.
 */

export const PANELS = ["order", "music", "now", "console"] as const
export type PanelId = (typeof PANELS)[number]

export const PANEL_INFO: Record<PanelId, { label: string; hint: string }> = {
  order: {
    label: "Order of service",
    hint: "The pieces in order, with Play, Next and Stop",
  },
  music: {
    label: "Music",
    hint: "The score of the piece, following along as it plays",
  },
  now: {
    label: "Now playing",
    hint: "Large title, verse and time left, readable from a distance",
  },
  console: {
    label: "Organ console",
    hint: "The stops and keys of the organ, live",
  },
}

export type Layout = "single" | "split"

export type Displays = {
  /** This screen. */
  layout: Layout
  panels: [PanelId, PanelId]
  /** A second screen or window. */
  secondLayout: Layout
  secondPanels: [PanelId, PanelId]
}

export const DEFAULT_DISPLAYS: Displays = {
  layout: "single",
  panels: ["order", "music"],
  secondLayout: "single",
  secondPanels: ["console", "music"],
}

const KEY = "cecilia.displays"

export function isPanel(v: unknown): v is PanelId {
  return typeof v === "string" && (PANELS as readonly string[]).includes(v)
}

function pair(v: unknown, fallback: [PanelId, PanelId]): [PanelId, PanelId] {
  return Array.isArray(v) && isPanel(v[0]) && isPanel(v[1])
    ? [v[0], v[1]]
    : fallback
}

function layout(v: unknown, fallback: Layout): Layout {
  return v === "single" || v === "split" ? v : fallback
}

/** Settings from storage, falling back to the defaults for anything missing or broken. */
export function parseDisplays(text: string | null): Displays {
  if (!text) return DEFAULT_DISPLAYS
  try {
    const d = JSON.parse(text) as Partial<Record<keyof Displays, unknown>>
    return {
      layout: layout(d.layout, DEFAULT_DISPLAYS.layout),
      panels: pair(d.panels, DEFAULT_DISPLAYS.panels),
      secondLayout: layout(d.secondLayout, DEFAULT_DISPLAYS.secondLayout),
      secondPanels: pair(d.secondPanels, DEFAULT_DISPLAYS.secondPanels),
    }
  } catch {
    return DEFAULT_DISPLAYS
  }
}

// A tiny store over localStorage, for useSyncExternalStore: the snapshot
// only changes when the stored text does, and other windows on this device
// hear about changes through the "storage" event.
let raw: string | null = null
let cached: Displays = DEFAULT_DISPLAYS
let memory: string | null = null // when storage is unavailable
const listeners = new Set<() => void>()

function read(): string | null {
  try {
    return window.localStorage.getItem(KEY)
  } catch {
    return memory
  }
}

export function getDisplays(): Displays {
  const text = read()
  if (text !== raw) {
    raw = text
    cached = parseDisplays(text)
  }
  return cached
}

export function getServerDisplays(): Displays {
  return DEFAULT_DISPLAYS
}

export function subscribeDisplays(onChange: () => void) {
  listeners.add(onChange)
  const onStorage = (e: StorageEvent) => {
    if (e.key === KEY) onChange()
  }
  window.addEventListener("storage", onStorage)
  return () => {
    listeners.delete(onChange)
    window.removeEventListener("storage", onStorage)
  }
}

export function saveDisplays(d: Displays) {
  const text = JSON.stringify(d)
  try {
    window.localStorage.setItem(KEY, text)
  } catch {
    // Private browsing or storage turned off: the choice lasts this visit only.
    memory = text
  }
  listeners.forEach((l) => l())
}

/** The panels a screen actually shows. */
export function shown(layout: Layout, panels: [PanelId, PanelId]): PanelId[] {
  return layout === "split" && panels[0] !== panels[1]
    ? [...panels]
    : [panels[0]]
}

/** Panels for a display window, from its address (`?panels=console,music`). */
export function panelsFromQuery(value: string | null): PanelId[] {
  const list = (value ?? "").split(",").filter(isPanel)
  const unique = [...new Set(list)].slice(0, 2)
  return unique.length ? unique : ["order"]
}

export function displayUrl(panels: PanelId[]): string {
  return `/service/display/?panels=${panels.join(",")}`
}

/** Messages between the Service page and its display windows. */
export type ChannelMessage =
  | { type: "panels"; panels: PanelId[] }
  | { type: "hello" }
  | { type: "open"; panels: PanelId[] }
  | { type: "closed" }

export const CHANNEL = "cecilia-service"
