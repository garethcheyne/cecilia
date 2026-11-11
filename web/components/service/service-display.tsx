"use client"

import * as React from "react"
import { useSearchParams } from "next/navigation"
import { MaximizeIcon, MinimizeIcon } from "lucide-react"

import { ServiceScreen } from "@/components/service/service-screen"
import { Button } from "@/components/ui/button"
import { useFullscreen, useServiceChannel } from "@/hooks/use-screens"
import { type PanelId, panelsFromQuery } from "@/lib/displays"

/**
 * A second screen for the service: only the chosen panels, no menus. It
 * follows changes made in Displays on the main Service page.
 */
export function ServiceDisplay() {
  const fromAddress = panelsFromQuery(useSearchParams().get("panels"))
  const [chosen, setChosen] = React.useState<PanelId[] | null>(null)
  const panels = chosen ?? fromAddress
  const fullscreen = useFullscreen()

  const post = useServiceChannel((m, reply) => {
    if (m.type === "panels") {
      setChosen(m.panels)
      // Keep the address in step, so a reload shows the same thing.
      window.history.replaceState(null, "", `?panels=${m.panels.join(",")}`)
    } else if (m.type === "hello") {
      reply({ type: "open", panels })
    }
  })

  React.useEffect(() => {
    post({ type: "open", panels })
    const bye = () => post({ type: "closed" })
    window.addEventListener("pagehide", bye)
    return () => window.removeEventListener("pagehide", bye)
    // Announce once; later changes come from the Service page itself.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [post])

  return (
    <div className="relative flex min-h-svh flex-col bg-background">
      <ServiceScreen panels={panels} standalone />
      {fullscreen.supported && (
        <Button
          variant={fullscreen.on ? "ghost" : "secondary"}
          size="sm"
          onClick={fullscreen.toggle}
          className="fixed top-3 right-3 z-30 opacity-80 shadow-sm hover:opacity-100 data-[on=true]:opacity-25 data-[on=true]:hover:opacity-100"
          data-on={fullscreen.on}
          aria-label={fullscreen.on ? "Leave full screen" : "Full screen"}
        >
          {fullscreen.on ? (
            <MinimizeIcon data-icon="inline-start" />
          ) : (
            <MaximizeIcon data-icon="inline-start" />
          )}
          {fullscreen.on ? "Leave full screen" : "Full screen"}
        </Button>
      )}
    </div>
  )
}
