"use client"

import { RotateCcwIcon } from "lucide-react"

import { LiveConsole, liveAct } from "@/components/console/live-console"
import { useEngine } from "@/components/engine-provider"
import { PageHeader } from "@/components/page-header"
import { Button } from "@/components/ui/button"

/**
 * The real organ, live: what's drawn and played on it right now, and a
 * remote control for it. Changes made at the console show up here too.
 */
export function ConsoleView() {
  const { status } = useEngine()
  return (
    <>
      <PageHeader title="Console">
        {status?.output && (
          <Button variant="outline" size="sm" onClick={() => liveAct({ type: "all_stops_off" })} title="Push in every stop">
            <RotateCcwIcon data-icon="inline-start" />
            <span className="hidden sm:inline">General cancel</span>
            <span className="sm:hidden">Cancel</span>
          </Button>
        )}
      </PageHeader>
      <div className="mx-auto flex w-full min-w-0 max-w-6xl flex-col gap-4 p-3 sm:p-4">
        <LiveConsole />
      </div>
    </>
  )
}
