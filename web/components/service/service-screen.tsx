"use client"

import * as React from "react"

import { ServicePanel } from "@/components/service/service-panels"
import { useServiceData } from "@/components/service/use-service-data"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useWide } from "@/hooks/use-screens"
import { useTransport } from "@/hooks/use-transport"
import { api, call } from "@/lib/api/client"
import { PANEL_INFO, type PanelId } from "@/lib/displays"
import { cn } from "@/lib/utils"

/** Space plays or pauses; the right arrow (or N) goes to the next piece. */
function useServiceKeys() {
  const t = useTransport()
  const state = t.player?.state
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement
      if (
        target.closest(
          "input, textarea, [contenteditable], [role=slider], [role=dialog]"
        )
      )
        return
      if (e.code === "Space") {
        e.preventDefault()
        if (state === "playing") t.pause()
        else t.play()
      } else if (e.key === "ArrowRight" || e.key.toLowerCase() === "n") {
        e.preventDefault()
        call(api.POST("/api/v1/service/next")).catch(() => {})
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [state, t])
}

const WIDTH: Record<PanelId, string> = {
  order: "max-w-4xl",
  now: "max-w-5xl",
  music: "max-w-6xl",
  console: "max-w-6xl",
}

/**
 * One or two Service panels. Two sit side by side when there's room, each
 * scrolling on its own; on a narrow screen they become tabs.
 *
 * `standalone` is a display window with no sidebar or transport bar.
 */
export function ServiceScreen({
  panels,
  standalone = false,
}: {
  panels: PanelId[]
  standalone?: boolean
}) {
  const data = useServiceData()
  const wide = useWide()
  useServiceKeys()

  if (panels.length === 1) {
    const id = panels[0]
    return (
      <div
        className={cn(
          "mx-auto flex w-full min-w-0 flex-1 flex-col p-3 sm:p-6",
          WIDTH[id]
        )}
      >
        <ServicePanel
          id={id}
          data={data}
          inColumn={standalone}
          display={standalone}
        />
      </div>
    )
  }

  if (!wide) {
    return (
      <Tabs
        defaultValue={panels[0]}
        className="flex w-full min-w-0 flex-1 flex-col gap-3 p-3"
      >
        <TabsList className="self-start">
          {panels.map((p) => (
            <TabsTrigger key={p} value={p}>
              {PANEL_INFO[p].label}
            </TabsTrigger>
          ))}
        </TabsList>
        {panels.map((p) => (
          <TabsContent key={p} value={p} className="flex min-w-0 flex-col">
            <ServicePanel
              id={p}
              data={data}
              inColumn={standalone}
              display={standalone}
            />
          </TabsContent>
        ))}
      </Tabs>
    )
  }

  return (
    <div
      className={cn(
        "grid min-w-0 grid-cols-2 grid-rows-[minmax(0,1fr)] divide-x",
        // Between the page header (3.5rem) and the transport bar (57px), unless standalone.
        standalone ? "h-svh" : "h-[calc(100svh-3.5rem-57px)]"
      )}
    >
      {panels.map((p) => (
        <section
          key={p}
          aria-label={PANEL_INFO[p].label}
          className="flex min-h-0 min-w-0 flex-col overflow-y-auto p-4 xl:p-6"
        >
          <ServicePanel id={p} data={data} inColumn display={standalone} />
        </section>
      ))}
    </div>
  )
}
