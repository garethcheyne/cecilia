"use client"

import * as React from "react"
import { CheckCircle2Icon, RotateCcwIcon, SkipForwardIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Progress } from "@/components/ui/progress"
import { Spinner } from "@/components/ui/spinner"
import { useEngine } from "@/components/engine-provider"
import type { OrganDefinition } from "@/lib/api/client"
import {
  describeBinding,
  firstControl,
  stopBinding,
  type StopBinding,
} from "@/lib/learn"
import { stopLabel } from "@/lib/stops"

type Step = "on" | "off" | "done"

/**
 * Learn stops one after another: "pull out X", "push it in", next. With a
 * single stop it learns just that one.
 */
export function LearnDialog({
  organ,
  stopIds,
  open,
  onOpenChange,
  onLearned,
}: {
  organ: OrganDefinition
  stopIds: string[]
  open: boolean
  onOpenChange: (o: boolean) => void
  onLearned: (stopId: string, binding: StopBinding) => void
}) {
  const [index, setIndex] = React.useState(0)
  const [step, setStep] = React.useState<Step>("on")
  const [onMsgs, setOnMsgs] = React.useState<number[][]>([])
  const [result, setResult] = React.useState<StopBinding | null>(null)
  const [learned, setLearned] = React.useState(0)
  const [wasOpen, setWasOpen] = React.useState(false)
  if (open && !wasOpen) {
    setWasOpen(true)
    setIndex(0)
    setStep("on")
    setResult(null)
    setLearned(0)
  }
  if (!open && wasOpen) setWasOpen(false)

  const { onMidi } = useEngine()
  const stop = organ.stops.find((s) => s.id === stopIds[index])
  const label = stop
    ? stopLabel({ division: stop.division, equivalent: stop.equivalent }, organ)
    : ""
  const many = stopIds.length > 1

  // Listen for the organ: the first stop message moves to the next step.
  React.useEffect(() => {
    if (!open || step === "done" || !stop) return
    const heard: number[][] = []
    return onMidi((e) => {
      if (e.direction !== "in") return
      heard.push(e.bytes)
      if (!firstControl(heard)) return
      if (step === "on") {
        setOnMsgs([...heard])
        setStep("off")
      } else {
        const b = stopBinding(onMsgs, heard)
        setResult(b)
        setStep("done")
        if (b) {
          onLearned(stop.id, b)
          setLearned((n) => n + 1)
        }
      }
    })
  }, [open, step, stop, onMidi, onMsgs, onLearned])

  const next = React.useCallback(() => {
    if (index + 1 >= stopIds.length) {
      onOpenChange(false)
      return
    }
    setIndex(index + 1)
    setStep("on")
    setResult(null)
  }, [index, stopIds.length, onOpenChange])

  // In the wizard, move on by itself shortly after a stop is learned.
  React.useEffect(() => {
    if (!many || step !== "done" || !result) return
    const t = setTimeout(next, 900)
    return () => clearTimeout(t)
  }, [many, step, result, next])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {many ? "Learn every stop" : "Learn a stop"}
          </DialogTitle>
          <DialogDescription>
            {many
              ? `Stop ${index + 1} of ${stopIds.length}. Work along the stop jambs at the console.`
              : "Cecilia listens to the organ's Organ in port."}
          </DialogDescription>
        </DialogHeader>
        {many && (
          <Progress
            value={(index / stopIds.length) * 100}
            aria-label="Progress"
          />
        )}
        <div
          className="flex flex-col items-center gap-4 py-6 text-center"
          aria-live="polite"
        >
          <p className="font-display text-3xl font-semibold text-balance">
            {label}
          </p>
          {step === "on" && (
            <p className="flex items-center gap-2 text-lg">
              <Spinner /> Pull it out now
            </p>
          )}
          {step === "off" && (
            <p className="flex items-center gap-2 text-lg">
              <Spinner /> Now push it in
            </p>
          )}
          {step === "done" &&
            (result ? (
              <p className="flex items-center gap-2 text-lg text-primary">
                <CheckCircle2Icon /> Learned:{" "}
                <span className="font-mono text-base">
                  {describeBinding(result)}
                </span>
              </p>
            ) : (
              <p className="text-lg text-destructive">
                That didn&apos;t look like a stop. Try again.
              </p>
            ))}
          <p className="max-w-sm text-sm text-muted-foreground">
            No organ to hand? Choose the simulator as Organ in and press its
            stops on the Simulator page.
          </p>
        </div>
        <DialogFooter className="sm:justify-between">
          <span className="text-sm text-muted-foreground">
            {many && `${learned} learned`}
          </span>
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={() => {
                setStep("on")
                setResult(null)
              }}
            >
              <RotateCcwIcon data-icon="inline-start" />
              Again
            </Button>
            {many ? (
              <Button variant="outline" onClick={next}>
                <SkipForwardIcon data-icon="inline-start" />
                {index + 1 >= stopIds.length ? "Finish" : "Skip"}
              </Button>
            ) : (
              <Button onClick={() => onOpenChange(false)}>
                {step === "done" && result ? "Done" : "Cancel"}
              </Button>
            )}
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
