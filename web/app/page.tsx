"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import {
  ArrowRightIcon,
  CheckCircle2Icon,
  CircleIcon,
  LibraryIcon,
  ListMusicIcon,
  MonitorPlayIcon,
  PianoIcon,
  PlayIcon,
} from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { PageBody } from "@/components/page-body"
import { PageHeader } from "@/components/page-header"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { useTransport } from "@/hooks/use-transport"
import {
  api,
  call,
  type PlaylistSummary,
  type TrackSummary,
} from "@/lib/api/client"
import { formatClock } from "@/lib/time"
import { cn } from "@/lib/utils"

export default function OverviewPage() {
  const { connected, status, service, libraryVersion } = useEngine()
  const router = useRouter()
  const t = useTransport()
  const ready = !!status && status.inputs.length > 0 && !!status.output
  const [playlists, setPlaylists] = React.useState<PlaylistSummary[] | null>(
    null
  )
  const [recent, setRecent] = React.useState<TrackSummary[] | null>(null)

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/playlists"))
      .then(setPlaylists)
      .catch(() => setPlaylists([]))
  }, [connected])

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/library"))
      .then((list) =>
        setRecent([...list].sort((a, b) => b.created - a.created).slice(0, 5))
      )
      .catch(() => setRecent([]))
  }, [connected, libraryVersion])

  const running = !!service && service.state !== "idle"
  const latest = playlists
    ?.filter((p) => p.tracks > 0)
    .sort((a, b) => b.modified - a.modified)[0]

  async function runService(p: PlaylistSummary) {
    try {
      await call(
        api.POST("/api/v1/service/start", { body: { playlist_id: p.id } })
      )
      router.push("/service/")
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  return (
    <>
      <PageHeader title="Overview" />
      <PageBody>
        {!connected && (
          <Alert variant="destructive">
            <AlertTitle>Can&apos;t reach Cecilia</AlertTitle>
            <AlertDescription>
              Check the computer running Cecilia is switched on. This page
              reconnects by itself.
            </AlertDescription>
          </Alert>
        )}

        <div className="flex flex-wrap items-center gap-x-5 gap-y-1 rounded-xl border bg-card px-4 py-2.5 text-sm">
          <Status ok={connected} label={connected ? "Connected" : "Offline"} />
          <Status
            ok={!!status?.inputs.length}
            label={
              status?.inputs.length
                ? `Organ in: ${status.inputs.join(", ")}`
                : "Organ in: not connected"
            }
          />
          <Status
            ok={!!status?.output}
            label={
              status?.output
                ? `Organ out: ${status.output}`
                : "Organ out: not connected"
            }
          />
          <Button
            variant="link"
            size="sm"
            className="ml-auto h-auto px-0"
            render={<Link href="/setup/midi" />}
            nativeButton={false}
          >
            MIDI Setup
          </Button>
        </div>
        {status && status.missing.length > 0 && (
          <Alert>
            <AlertTitle>Some saved ports are unplugged</AlertTitle>
            <AlertDescription>
              {status.missing.join(", ")}. Cecilia reconnects as soon as
              they&apos;re plugged in.
            </AlertDescription>
          </Alert>
        )}

        {connected && !ready && (
          <Card>
            <CardHeader>
              <CardTitle>Connect your organ</CardTitle>
              <CardDescription>
                Two steps, then Cecilia can record and play your organ.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ol className="flex flex-col gap-2 text-sm">
                <Step done={connected}>Cecilia is running</Step>
                <Step done={ready}>
                  Choose the organ&apos;s MIDI connections in MIDI Setup
                </Step>
              </ol>
            </CardContent>
            <CardFooter className="flex-wrap gap-2">
              <Button render={<Link href="/setup/midi" />} nativeButton={false}>
                Open MIDI Setup
                <ArrowRightIcon data-icon="inline-end" />
              </Button>
            </CardFooter>
          </Card>
        )}

        <div className="grid gap-4 lg:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle>
                {running ? "Service in progress" : "Next service"}
              </CardTitle>
              <CardDescription>
                {running
                  ? (service?.name ?? "A service is running.")
                  : latest
                    ? `${latest.name} · ${latest.tracks} ${latest.tracks === 1 ? "piece" : "pieces"}`
                    : "Plan the music for a service as a playlist, then run it from here."}
              </CardDescription>
            </CardHeader>
            <CardFooter className="flex-wrap gap-2">
              {running ? (
                <Button render={<Link href="/service/" />} nativeButton={false}>
                  <MonitorPlayIcon data-icon="inline-start" />
                  Go to the service
                </Button>
              ) : latest ? (
                <Button onClick={() => runService(latest)}>
                  <MonitorPlayIcon data-icon="inline-start" />
                  Run service
                </Button>
              ) : null}
              <Button
                variant="outline"
                render={<Link href="/playlists/" />}
                nativeButton={false}
              >
                <ListMusicIcon data-icon="inline-start" />
                {latest || running ? "All playlists" : "Plan a service"}
              </Button>
            </CardFooter>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Recently added</CardTitle>
              <CardAction>
                <Button
                  variant="ghost"
                  size="sm"
                  render={<Link href="/library/" />}
                  nativeButton={false}
                >
                  <LibraryIcon data-icon="inline-start" />
                  Library
                </Button>
              </CardAction>
            </CardHeader>
            <CardContent>
              {recent && recent.length === 0 ? (
                <Empty className="p-4">
                  <EmptyHeader>
                    <EmptyTitle>No music yet</EmptyTitle>
                    <EmptyDescription>
                      Record from the organ, or add hymns from the shared
                      library.
                    </EmptyDescription>
                  </EmptyHeader>
                </Empty>
              ) : (
                <ul className="-mx-2 flex flex-col">
                  {(recent ?? []).map((tr) => (
                    <li
                      key={tr.id}
                      className="flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-muted/50"
                    >
                      <div className="flex min-w-0 flex-1 flex-col">
                        <span className="truncate font-medium">
                          {tr.info.title}
                        </span>
                        <span className="truncate text-xs text-muted-foreground">
                          {[
                            tr.info.melody || tr.info.composer,
                            formatClock(tr.duration_ms),
                          ]
                            .filter(Boolean)
                            .join(" · ")}
                        </span>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`Play ${tr.info.title}`}
                        onClick={() => t.load(tr.id)}
                      >
                        <PlayIcon />
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            render={<Link href="/simulator/" />}
            nativeButton={false}
          >
            <PianoIcon data-icon="inline-start" />
            Try the simulator
          </Button>
          <Button
            variant="outline"
            render={<Link href="/library/" />}
            nativeButton={false}
          >
            <LibraryIcon data-icon="inline-start" />
            Find hymns in the shared library
          </Button>
        </div>
      </PageBody>
    </>
  )
}

function Status({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      <span
        className={cn(
          "size-2 shrink-0 rounded-full",
          ok
            ? "bg-success shadow-[0_0_0_3px_color-mix(in_oklch,var(--success),transparent_80%)]"
            : "bg-muted-foreground/40"
        )}
        aria-hidden
      />
      <span className="truncate">{label}</span>
    </span>
  )
}

function Step({
  done,
  children,
}: {
  done: boolean
  children: React.ReactNode
}) {
  return (
    <li
      className={cn("flex items-center gap-2", done && "text-muted-foreground")}
    >
      {done ? (
        <CheckCircle2Icon className="size-4 text-success" aria-label="Done" />
      ) : (
        <CircleIcon
          className="size-4 text-muted-foreground"
          aria-label="To do"
        />
      )}
      {children}
    </li>
  )
}
