"use client"

import Link from "next/link"
import { ArrowRightIcon, PianoIcon } from "lucide-react"

import { useEngine } from "@/components/engine-provider"
import { PageHeader } from "@/components/page-header"
import { cn } from "@/lib/utils"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"

export default function OverviewPage() {
  const { connected, status } = useEngine()
  const ready = !!status && status.inputs.length > 0 && !!status.output

  return (
    <>
      <PageHeader title="Overview" />
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-4 p-3 sm:p-4">
        {!connected && (
          <Alert variant="destructive">
            <AlertTitle>Can&apos;t reach the server</AlertTitle>
            <AlertDescription>
              Start organ-server, then this page will connect automatically.
            </AlertDescription>
          </Alert>
        )}
        <div className="grid gap-3 md:grid-cols-3">
          <StatusCard label="Server" value={connected ? "Connected" : "Offline"} ok={connected} />
          <StatusCard
            label="Organ in"
            value={status?.inputs.length ? status.inputs.join(", ") : "Not connected"}
            ok={!!status?.inputs.length}
          />
          <StatusCard label="Organ out" value={status?.output ?? "Not connected"} ok={!!status?.output} />
        </div>
        {status && status.missing.length > 0 && (
          <Alert>
            <AlertTitle>Some saved ports are unavailable</AlertTitle>
            <AlertDescription>{status.missing.join(", ")}</AlertDescription>
          </Alert>
        )}
        <Card>
          <CardHeader>
            <CardTitle>{ready ? "MIDI is set up" : "Get started"}</CardTitle>
            <CardDescription>
              {ready
                ? "Use the test recorder to confirm round-trip playback before recording music."
                : "Choose your organ's MIDI ports and run a test recording."}
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            <Badge variant={connected ? "secondary" : "outline"}>1 · Start server</Badge>
            <Badge variant={ready ? "secondary" : "outline"}>2 · Connect ports</Badge>
            <Badge variant="outline">3 · Test record &amp; play</Badge>
          </CardContent>
          <CardFooter className="flex-wrap gap-2">
            <Button render={<Link href="/setup/midi" />} nativeButton={false}>
              MIDI Setup
              <ArrowRightIcon data-icon="inline-end" />
            </Button>
            <Button variant="outline" render={<Link href="/simulator" />} nativeButton={false}>
              <PianoIcon data-icon="inline-start" />
              Try the simulator
            </Button>
          </CardFooter>
        </Card>
      </div>
    </>
  )
}

function StatusCard({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <Card>
      <CardHeader>
        <CardDescription className="flex items-center gap-2">
          <span
            className={cn("size-2 rounded-full", ok ? "bg-success shadow-[0_0_0_3px_color-mix(in_oklch,var(--success),transparent_80%)]" : "bg-muted-foreground/40")}
            aria-hidden
          />
          {label}
        </CardDescription>
        <CardTitle className="truncate">{value}</CardTitle>
      </CardHeader>
    </Card>
  )
}
