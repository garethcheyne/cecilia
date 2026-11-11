"use client"

import * as React from "react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { api, call } from "@/lib/api/client"

/**
 * After Cecilia updates and restarts, screens that were already open still
 * run the old page. Notice the new version when the connection comes back,
 * and offer to reload (on a display screen, just reload).
 */
export function UpdateWatcher() {
  const { connected } = useEngine()
  const first = React.useRef<string | null>(null)

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/health"))
      .then((h) => {
        if (first.current === null) {
          first.current = h.version
          return
        }
        if (h.version === first.current) return
        if (window.location.pathname.startsWith("/service/display")) {
          window.location.reload()
          return
        }
        toast(`Cecilia has been updated to ${h.version}`, {
          id: "cecilia-updated",
          description: "Reload to use the new version.",
          duration: Infinity,
          action: { label: "Reload", onClick: () => window.location.reload() },
        })
      })
      .catch(() => {})
  }, [connected])

  return null
}
