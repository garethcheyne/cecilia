"use client"

import * as React from "react"
import Link from "next/link"
import {
  ArrowLeftRightIcon,
  RotateCcwIcon,
  Volume2Icon,
  VolumeXIcon,
} from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { ConnectionCard } from "@/components/organ/connection-card"
import { OrganConsole } from "@/components/organ/organ-console"
import { PageHeader } from "@/components/page-header"
import {
  Alert,
  AlertAction,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Kbd } from "@/components/ui/kbd"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { useComputerKeyboard } from "@/hooks/use-computer-keyboard"
import {
  api,
  call,
  type DivisionRole,
  type OrganDefinition,
  type OrganEvent,
  type OrganSummary,
} from "@/lib/api/client"
import { noteName } from "@/lib/midi"
import { formatFootage } from "@/lib/organ"
import { keyboardSpeaks, starterStop } from "@/lib/sounding"

const SIM_PORT = "Cecilia Simulator"

function act(event: OrganEvent) {
  return call(api.POST("/api/v1/simulator/act", { body: event })).catch(
    (e: Error) => {
      toast.error(e.message)
    }
  )
}

export function SimulatorView() {
  const { connected, simulator, status } = useEngine()
  const [organ, setOrgan] = React.useState<OrganDefinition | null>(null)
  const [organs, setOrgans] = React.useState<OrganSummary[]>([])
  const [playOn, setPlayOn] = React.useState<DivisionRole>("great")

  const organId = simulator?.organ_id
  React.useEffect(() => {
    if (!organId) return
    let cancelled = false
    Promise.all([
      call(
        api.GET("/api/v1/organs/{id}", { params: { path: { id: organId } } })
      ),
      call(api.GET("/api/v1/organs")),
    ])
      .then(([o, list]) => {
        if (cancelled) return
        setOrgan(o)
        setOrgans(list)
      })
      .catch((e: Error) => toast.error(e.message))
    return () => {
      cancelled = true
    }
  }, [organId])

  // Keep the computer-keyboard target valid for this organ.
  const playDivision = organ?.divisions.some((d) => d.role === playOn)
    ? playOn
    : (organ?.divisions.find((d) => d.role !== "pedal")?.role ?? "great")

  // Stops that are on, including ones just drawn whose update hasn't come back yet.
  const stopsOn = React.useRef<string[]>([])
  const consoleStops = simulator?.console.stops
  React.useEffect(() => {
    stopsOn.current = consoleStops ?? []
  }, [consoleStops])

  /**
   * Like a real organ, keys make no sound until a stop is drawn. Rather than
   * leave a silent simulator, draw a soft stop first and say so.
   */
  async function press(event: OrganEvent) {
    if (
      organ &&
      event.type === "note" &&
      event.velocity > 0 &&
      !keyboardSpeaks(organ, stopsOn.current, event.division)
    ) {
      const stop = starterStop(organ, event.division)
      if (stop) {
        stopsOn.current = [...stopsOn.current, stop.id]
        await act({ type: "stop", stop: stop.id, on: true })
        const feet =
          stop.equivalent.type === "voice"
            ? ` ${formatFootage(stop.equivalent.footage)}`
            : ""
        const name = stop.full_name ?? stop.names.join(" ")
        toast(`Drew ${name}${feet} so you can hear the keys`, {
          id: "starter-stop",
          description:
            "An organ is silent until a stop is drawn. Choose other stops above to change the sound.",
        })
      }
    }
    void act(event)
  }

  const { base } = useComputerKeyboard(!!organ, (key, on) =>
    press({ type: "note", division: playDivision, key, velocity: on ? 96 : 0 })
  )

  async function loadOrgan(id: string) {
    try {
      await call(api.PUT("/api/v1/simulator/organ", { body: { organ_id: id } }))
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function reset() {
    try {
      await call(api.POST("/api/v1/simulator/reset"))
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  const routed =
    !!status && status.inputs.includes(SIM_PORT) && status.output === SIM_PORT

  return (
    <>
      <PageHeader title="Simulator">
        {simulator && (
          <Badge
            variant="secondary"
            className="hidden sm:inline-flex"
            title={simulator.audio ?? undefined}
          >
            {simulator.audio ? (
              <Volume2Icon data-icon="inline-start" />
            ) : (
              <VolumeXIcon data-icon="inline-start" />
            )}
            {simulator.audio ? "Sound on" : "Silent"}
          </Badge>
        )}
        <Button
          variant="outline"
          size="sm"
          onClick={reset}
          disabled={!connected}
          title="Push in every stop and release all keys"
        >
          <RotateCcwIcon data-icon="inline-start" />
          General cancel
        </Button>
      </PageHeader>

      <div className="mx-auto flex w-full max-w-6xl min-w-0 flex-col gap-4 p-3 sm:p-4">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div className="flex flex-col gap-1">
            <p className="font-display text-2xl font-semibold sm:text-3xl">
              {organ?.name ?? "Loading organ…"}
            </p>
            <p className="text-sm text-muted-foreground">
              A virtual organ for practising, testing and listening without an
              instrument.
            </p>
          </div>
          {organs.length > 1 && organId && (
            <DropdownMenu>
              <DropdownMenuTrigger
                render={<Button variant="outline" size="sm" />}
              >
                <ArrowLeftRightIcon data-icon="inline-start" />
                Change organ
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-64">
                <DropdownMenuGroup>
                  <DropdownMenuLabel>Organ to simulate</DropdownMenuLabel>
                  <DropdownMenuRadioGroup
                    value={organId}
                    onValueChange={(v) => v && void loadOrgan(String(v))}
                  >
                    {organs.map((o) => (
                      <DropdownMenuRadioItem key={o.id} value={o.id}>
                        {o.name}
                      </DropdownMenuRadioItem>
                    ))}
                  </DropdownMenuRadioGroup>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>

        {status && !routed && (
          <Alert>
            <AlertTitle>Playing here isn&apos;t being recorded</AlertTitle>
            <AlertDescription>
              To record what you play, and to hear playback on this organ,
              choose {SIM_PORT} as both Organ in and Organ out.
            </AlertDescription>
            <AlertAction>
              <Button
                size="sm"
                variant="outline"
                render={<Link href="/setup/midi" />}
                nativeButton={false}
              >
                MIDI Setup
              </Button>
            </AlertAction>
          </Alert>
        )}

        {organ && simulator ? (
          <OrganConsole
            organ={organ}
            state={simulator.console}
            onAct={press}
            activeDivision={playDivision}
          />
        ) : (
          <Skeleton className="h-[36rem] w-full rounded-xl" />
        )}

        {organ && (
          <div className="hidden flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground md:flex">
            <span>Play with your computer keyboard:</span>
            <span className="flex items-center gap-1">
              <Kbd>A</Kbd>–<Kbd>;</Kbd> plays {noteName(base)} to{" "}
              {noteName(base + 16)}
            </span>
            <span className="flex items-center gap-1">
              <Kbd>Z</Kbd> <Kbd>X</Kbd> change octave
            </span>
            <span className="ml-auto flex items-center gap-2">
              Keys play on
              <Select
                value={playDivision}
                onValueChange={(v) => v && setPlayOn(v as DivisionRole)}
                items={organ.divisions.map((d) => ({
                  value: d.role,
                  label: d.name,
                }))}
              >
                <SelectTrigger
                  size="sm"
                  className="w-32"
                  aria-label="Division for computer keys"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {organ.divisions.map((d) => (
                      <SelectItem key={d.role} value={d.role}>
                        {d.name}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </span>
          </div>
        )}

        {organ && <ConnectionCard organ={organ} />}
      </div>
    </>
  )
}
