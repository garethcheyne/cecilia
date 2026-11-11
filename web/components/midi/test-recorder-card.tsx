"use client"

import * as React from "react"
import { CircleIcon, OctagonXIcon, PlayIcon, SquareIcon } from "lucide-react"
import { toast } from "sonner"

import { useEngine, useTicker } from "@/components/engine-provider"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
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
import { api, call } from "@/lib/api/client"
import { formatDuration } from "@/lib/midi"

const STATE_LABEL = {
  idle: "Idle",
  recording: "Recording",
  playing: "Playing",
} as const

export function TestRecorderCard() {
  const { connected, status, statusAt } = useEngine()
  const [busy, setBusy] = React.useState(false)
  const test = status?.test
  const state = test?.state ?? "idle"
  const now = useTicker(state !== "idle")

  // The server only reports times on state changes; interpolate between them.
  const sinceStatus = state !== "idle" && now > statusAt ? now - statusAt : 0
  const duration =
    (test?.duration_ms ?? 0) + (state === "recording" ? sinceStatus : 0)
  const position =
    state === "playing" ? (test?.position_ms ?? 0) + sinceStatus : 0

  async function run(action: "record" | "stop" | "play") {
    setBusy(true)
    try {
      await call(api.POST(`/api/v1/midi/test/${action}`))
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  async function panic() {
    try {
      await call(api.POST("/api/v1/midi/panic"))
      toast.success("All notes off sent")
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  const noOutput = status !== null && !status.output
  const noInputs = status !== null && status.inputs.length === 0

  return (
    <Card>
      <CardHeader>
        <CardTitle>Test recorder</CardTitle>
        <CardDescription>
          Play a few notes, then play them straight back exactly as they came
          in. If that doesn&apos;t sound right, check the cables and the choices
          under Ports.
        </CardDescription>
        <CardAction>
          <Badge
            variant={
              state === "recording"
                ? "destructive"
                : state === "playing"
                  ? "default"
                  : "secondary"
            }
          >
            {STATE_LABEL[state]}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <dl className="grid grid-cols-3 gap-4">
          <div className="flex flex-col gap-1">
            <dt className="text-xs text-muted-foreground">Length</dt>
            <dd className="font-mono text-2xl tabular-nums">
              {formatDuration(duration)}
            </dd>
          </div>
          <div className="flex flex-col gap-1">
            <dt className="text-xs text-muted-foreground">Position</dt>
            <dd className="font-mono text-2xl tabular-nums">
              {formatDuration(position)}
            </dd>
          </div>
          <div className="flex flex-col gap-1">
            <dt className="text-xs text-muted-foreground">Messages</dt>
            <dd className="font-mono text-2xl tabular-nums">
              {test?.events ?? 0}
            </dd>
          </div>
        </dl>
        {(noInputs || noOutput) && (
          <Alert>
            <AlertTitle>
              {noInputs ? "No input connected" : "No output connected"}
            </AlertTitle>
            <AlertDescription>
              {noInputs
                ? "Choose at least one connection under Organ in to record."
                : "Choose a connection under Organ out to play back."}
            </AlertDescription>
          </Alert>
        )}
      </CardContent>
      <CardFooter className="flex-wrap gap-2">
        <Button
          variant="destructive"
          onClick={() => run("record")}
          disabled={!connected || busy || state !== "idle" || noInputs}
        >
          <CircleIcon data-icon="inline-start" />
          Record
        </Button>
        <Button
          variant="outline"
          onClick={() => run("stop")}
          disabled={!connected || busy || state === "idle"}
        >
          <SquareIcon data-icon="inline-start" />
          Stop
        </Button>
        <Button
          onClick={() => run("play")}
          disabled={
            !connected || busy || state !== "idle" || noOutput || !test?.events
          }
        >
          <PlayIcon data-icon="inline-start" />
          Play
        </Button>
        <Button
          variant="ghost"
          className="ml-auto"
          onClick={panic}
          disabled={!connected || noOutput}
        >
          <OctagonXIcon data-icon="inline-start" />
          All notes off
        </Button>
      </CardFooter>
    </Card>
  )
}
