"use client"

import * as React from "react"
import { MaximizeIcon, MinimizeIcon } from "lucide-react"

import { PageHeader } from "@/components/page-header"
import { DisplaysDialog } from "@/components/service/displays-dialog"
import { ServiceScreen } from "@/components/service/service-screen"
import { Button } from "@/components/ui/button"
import { useFullscreen, useScreenCount, useServiceChannel } from "@/hooks/use-screens"
import {
  type Displays,
  getDisplays,
  getServerDisplays,
  saveDisplays,
  shown,
  subscribeDisplays,
} from "@/lib/displays"

/**
 * The screen for whoever runs the music during a service: large text, the
 * current piece highlighted, and big buttons. It can show two panels side by
 * side, and send other panels to a second screen.
 */
export function ServiceView() {
  const displays = React.useSyncExternalStore(subscribeDisplays, getDisplays, getServerDisplays)
  const screens = useScreenCount()
  const fullscreen = useFullscreen()
  const [secondOpen, setSecondOpen] = React.useState(false)
  const secondPanels = shown(displays.secondLayout, displays.secondPanels)

  const post = useServiceChannel((m) => {
    if (m.type === "open") setSecondOpen(true)
    else if (m.type === "closed") setSecondOpen(false)
  })

  // Ask whether a display window is already open (say, after a reload).
  React.useEffect(() => {
    post({ type: "hello" })
  }, [post])

  function change(d: Displays) {
    saveDisplays(d)
    // An open display window follows the new choice straight away.
    post({ type: "panels", panels: shown(d.secondLayout, d.secondPanels) })
  }

  // Keep a display window that answers "hello" on the current choice.
  React.useEffect(() => {
    if (secondOpen) post({ type: "panels", panels: secondPanels })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [secondOpen, secondPanels.join(",")])

  return (
    <>
      <PageHeader title="Service">
        <DisplaysDialog displays={displays} onChange={change} screens={screens} secondOpen={secondOpen} />
        {fullscreen.supported && (
          <Button
            variant="outline"
            size="sm"
            onClick={fullscreen.toggle}
            aria-label={fullscreen.on ? "Leave full screen" : "Full screen"}
          >
            {fullscreen.on ? <MinimizeIcon data-icon="inline-start" /> : <MaximizeIcon data-icon="inline-start" />}
            <span className="hidden sm:inline">{fullscreen.on ? "Leave full screen" : "Full screen"}</span>
          </Button>
        )}
      </PageHeader>
      <ServiceScreen panels={shown(displays.layout, displays.panels)} />
    </>
  )
}
