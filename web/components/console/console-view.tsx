"use client"

import * as React from "react"
import Link from "next/link"
import { CableIcon, RotateCcwIcon } from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { OrganConsole } from "@/components/organ/organ-console"
import { PageHeader } from "@/components/page-header"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { api, call, type LiveSnapshot, type OrganDefinition, type OrganEvent } from "@/lib/api/client"

function act(event: OrganEvent) {
  call(api.POST("/api/v1/live/act", { body: event })).catch((e: Error) => toast.error(e.message))
}

/**
 * The real organ, live: what's drawn and played on it right now, and a
 * remote control for it. Changes made at the console show up here too.
 */
export function ConsoleView() {
  const { connected, status, live } = useEngine()
  const [snapshot, setSnapshot] = React.useState<LiveSnapshot | null>(null)
  const [organ, setOrgan] = React.useState<OrganDefinition | null>(null)

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/live")).then(setSnapshot).catch(() => {})
  }, [connected, status?.output])

  const organId = live?.organ_id ?? snapshot?.organ_id
  React.useEffect(() => {
    if (!organId) return
    call(api.GET("/api/v1/organs/{id}", { params: { path: { id: organId } } }))
      .then(setOrgan)
      .catch(() => {})
  }, [organId])

  const console_ = live?.organ_id === organId && live ? live.console : snapshot?.console
  const output = status?.output

  return (
    <>
      <PageHeader title="Console">
        {output && (
          <Button variant="outline" size="sm" onClick={() => act({ type: "all_stops_off" })} title="Push in every stop">
            <RotateCcwIcon data-icon="inline-start" />
            <span className="hidden sm:inline">General cancel</span>
            <span className="sm:hidden">Cancel</span>
          </Button>
        )}
      </PageHeader>
      <div className="mx-auto flex w-full min-w-0 max-w-6xl flex-col gap-4 p-3 sm:p-4">
        {status && !output ? (
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <CableIcon />
              </EmptyMedia>
              <EmptyTitle>No organ connected</EmptyTitle>
              <EmptyDescription>Choose the organ as Organ out on MIDI Setup to see and control it here.</EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button render={<Link href="/setup/midi" />} nativeButton={false}>
                MIDI Setup
              </Button>
            </EmptyContent>
          </Empty>
        ) : (
          <>
            <div className="flex flex-wrap items-center gap-3">
              <p className="font-display text-2xl font-semibold sm:text-3xl">{organ?.name ?? "Connecting…"}</p>
              {output && <Badge variant="secondary">{output}</Badge>}
            </div>
            <p className="text-sm text-muted-foreground">
              Stops and keys follow the organ as it&apos;s played. Touch a stop to draw or push it in on the organ itself.
            </p>
            {organ && console_ ? (
              <OrganConsole organ={organ} state={console_} onAct={act} />
            ) : (
              <Skeleton className="h-[36rem] w-full rounded-xl" />
            )}
          </>
        )}
      </div>
    </>
  )
}
